//! Latest-request media preparation. Neither the editor nor the audio callback waits here.
use crate::{Result, Session, error};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Request {
    revision: u64,
    session: Session,
}
type Completion = (u64, Result<Session>);
type Decoded = std::collections::HashMap<daw_core::Id, daw_media::AudioData>;
struct Shared {
    pending: Mutex<Option<Request>>,
    ready: Mutex<Option<Completion>>,
    decoded: Mutex<Option<(u64, Decoded)>>,
    wake: Condvar,
    revision: AtomicU64,
    stopped: AtomicBool,
    progress: AtomicU64,
    total: AtomicU64,
    waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
/// One running request and one replaceable pending request. Dropping never joins on the UI.
/// Each instance is a project-open generation; its results cannot escape into another instance.
pub struct BackgroundAudio {
    shared: Arc<Shared>,
}
impl BackgroundAudio {
    pub fn new() -> Result<Self> {
        Self::with_prepare(|session, cancelled, progress, decoded| {
            session.prepare_project_audio_observed(
                &|| cancelled(),
                &|done, total| progress(done, total),
                &|audio| decoded(audio),
            )
        })
    }
    fn with_prepare(
        prepare: impl Fn(
            &mut Session,
            &(dyn Fn() -> bool + Sync),
            &dyn Fn(u64, u64),
            &dyn Fn(&Decoded),
        ) -> Result<()>
        + Send
        + 'static,
    ) -> Result<Self> {
        let shared = Arc::new(Shared {
            pending: Mutex::new(None),
            ready: Mutex::new(None),
            decoded: Mutex::new(None),
            wake: Condvar::new(),
            revision: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            progress: AtomicU64::new(0),
            total: AtomicU64::new(0),
            waker: Mutex::new(None),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("project-audio".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut pending = worker.pending.lock().unwrap();
                        while pending.is_none() && !worker.stopped.load(Ordering::Acquire) {
                            pending = worker.wake.wait(pending).unwrap();
                        }
                        if worker.stopped.load(Ordering::Acquire) {
                            break;
                        }
                        pending.take().unwrap()
                    };
                    let cancelled = || {
                        worker.stopped.load(Ordering::Acquire)
                            || worker.revision.load(Ordering::Acquire) != request.revision
                    };
                    let mut session = request.session;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        prepare(
                            &mut session,
                            &cancelled,
                            &|done, total| {
                                worker.progress.store(done, Ordering::Relaxed);
                                worker.total.store(total, Ordering::Relaxed);
                                let wake = worker.waker.lock().unwrap().clone();
                                if let Some(wake) = wake {
                                    wake();
                                }
                            },
                            &|audio| {
                                if !cancelled() {
                                    *worker.decoded.lock().unwrap() =
                                        Some((request.revision, audio.clone()));
                                    let wake = worker.waker.lock().unwrap().clone();
                                    if let Some(wake) = wake {
                                        wake();
                                    }
                                }
                            },
                        )
                    }))
                    .unwrap_or_else(|_| Err(error("Audio preparation worker stopped unexpectedly")))
                    .map(|()| session);
                    if !cancelled() {
                        *worker.ready.lock().unwrap() = Some((request.revision, result));
                        let wake = worker.waker.lock().unwrap().clone();
                        if let Some(wake) = wake {
                            wake();
                        }
                    }
                }
            })
            .map_err(error)?;
        Ok(Self { shared })
    }
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.shared.waker.lock().unwrap() = Some(Arc::new(waker));
    }
    pub fn submit(&self, session: Session) -> u64 {
        let revision = self.shared.revision.fetch_add(1, Ordering::AcqRel) + 1;
        *self.shared.pending.lock().unwrap() = Some(Request { revision, session });
        self.shared.progress.store(0, Ordering::Relaxed);
        self.shared.total.store(0, Ordering::Relaxed);
        self.shared.wake.notify_one();
        revision
    }
    /// Cancellation pauses preparation; it does not roll back an editor revision.
    pub fn cancel(&self) {
        self.shared.revision.fetch_add(1, Ordering::AcqRel);
        self.shared.pending.lock().unwrap().take();
    }
    pub fn poll(&self) -> Option<Completion> {
        self.shared
            .ready
            .lock()
            .unwrap()
            .take()
            .filter(|(revision, _)| *revision == self.shared.revision.load(Ordering::Acquire))
    }
    /// Verified original PCM/peaks may be shown while transforms remain pending.
    /// These maps are never a complete render plan.
    pub fn poll_decoded(&self) -> Option<Decoded> {
        self.shared
            .decoded
            .lock()
            .unwrap()
            .take()
            .filter(|(revision, _)| *revision == self.shared.revision.load(Ordering::Acquire))
            .map(|(_, audio)| audio)
    }
    pub fn progress(&self) -> (u64, u64) {
        (
            self.shared.progress.load(Ordering::Relaxed),
            self.shared.total.load(Ordering::Relaxed),
        )
    }
}
impl Drop for BackgroundAudio {
    fn drop(&mut self) {
        let _pending = self.shared.pending.lock().unwrap();
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, mpsc};
    fn completion(worker: &BackgroundAudio) -> Completion {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            assert!(std::time::Instant::now() < deadline, "worker timed out");
            std::thread::yield_now();
        }
    }
    #[test]
    fn latest_request_replaces_waiting_work_and_rejects_obsolete_completion() {
        let (started_tx, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let worker = BackgroundAudio::with_prepare(move |_, _, _, _| {
            if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                started_tx.send(()).unwrap();
                gate.recv().unwrap();
            }
            Ok(())
        })
        .unwrap();
        worker.submit(Session::default());
        started.recv().unwrap();
        let mut next = Session::default();
        next.project.name = "Discard".into();
        worker.submit(next.clone());
        next.project.name = "Latest".into();
        let revision = worker.submit(next);
        release.send(()).unwrap();
        let (actual, result) = completion(&worker);
        assert_eq!(actual, revision);
        assert_eq!(result.unwrap().project.name, "Latest");
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
    #[test]
    fn cancel_retains_no_late_result_and_retry_and_failure_are_explicit() {
        let (started_tx, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let count = AtomicUsize::new(0);
        let worker = BackgroundAudio::with_prepare(move |_, _, _, _| {
            if count.fetch_add(1, Ordering::Relaxed) == 0 {
                started_tx.send(()).unwrap();
                gate.recv().unwrap();
            }
            Err(error("injected failure"))
        })
        .unwrap();
        worker.submit(Session::default());
        started.recv().unwrap();
        worker.cancel();
        release.send(()).unwrap();
        let revision = worker.submit(Session::default());
        let (actual, result) = completion(&worker);
        assert_eq!(actual, revision);
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("injected failure")
        );
    }
    #[test]
    fn dropping_a_generation_does_not_join_its_running_worker() {
        let (started_tx, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (finished_tx, finished) = mpsc::channel();
        let worker = BackgroundAudio::with_prepare(move |_, cancelled, _, _| {
            started_tx.send(()).unwrap();
            gate.recv().unwrap();
            assert!(cancelled());
            finished_tx.send(()).unwrap();
            Ok(())
        })
        .unwrap();
        worker.submit(Session::default());
        started.recv().unwrap();
        drop(worker);
        release.send(()).unwrap();
        finished.recv().unwrap();
    }
}
