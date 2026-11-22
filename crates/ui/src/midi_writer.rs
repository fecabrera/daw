//! Storage has its own worker so slow snapshots and disks cannot stall live note release.
use super::midi_service::{Inbox, Message};
use daw_core::Id;
use daw_output::{MidiAction, MidiReceipt};
use daw_project::{
    SaveSnapshot, Session,
    midi_recording::{CaptureAction, CaptureEvent, TakeJournal},
    recovery::{RecoveryEntry, RecoveryStore},
};
use rtrb::{Producer, RingBuffer};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(super) enum Request {
    Begin {
        session: Session,
        targets: Vec<Id>,
        revision: u64,
        count_in: u64,
    },
    Snapshot(SaveSnapshot, u64, Option<PathBuf>),
    Acknowledge,
    Recover(RecoveryEntry),
    OpenSaved(RecoveryEntry),
    Discard(RecoveryEntry),
    LeaveProject(Id),
    Retry,
    Shutdown,
}
pub(super) enum Status {
    StorageReady,
    BeginReady(u64),
    Idle,
    Fault,
}
pub(super) struct Writer {
    control: mpsc::SyncSender<Request>,
    events: Producer<MidiReceipt>,
    status: mpsc::Receiver<Status>,
    terminal: Arc<AtomicU64>,
    last_frame: u64,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Writer {
    pub fn new(root: PathBuf, messages: Inbox) -> Self {
        let (control, requests) = mpsc::sync_channel(8);
        let (events, mut receipts) = RingBuffer::<MidiReceipt>::new(32768);
        let (status, updates) = mpsc::sync_channel(8);
        let terminal = Arc::new(AtomicU64::new(u64::MAX));
        let terminal_worker = terminal.clone();
        let worker = std::thread::spawn(move || {
            let initialized = (|| {
                let choices = RecoveryStore::scan(&root)?;
                let _ = messages.send(Message::Recoveries(choices));
                RecoveryStore::create(&root)
            })();
            let store = match initialized {
                Ok(s) => s,
                Err(e) => {
                    let _ =
                        messages.send(Message::Error(format!("Recovery storage unavailable: {e}")));
                    return;
                }
            };
            let _ = messages.send(Message::StorageReady);
            let _ = status.send(Status::StorageReady);
            let mut take: Option<TakeJournal> = None;
            let mut base = String::new();
            let mut revision = 0;
            let mut retry_base: Option<Session> = None;
            let mut synced = Instant::now();
            let mut interrupted = false;
            loop {
                for _ in 0..8 {
                    let request = match requests.try_recv() {
                        Ok(r) => r,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(_) => return,
                    };
                    let recording_request =
                        matches!(&request, Request::Begin { .. } | Request::Retry);
                    let result: Result<(), String> = (|| {
                        match request {
                            Request::Shutdown => {
                                let closing = (|| {
                                    if let Some(j) = &mut take {
                                        j.sync()?;
                                    }
                                    store.clean_exit()
                                })();
                                if let Err(error) = closing {
                                    let _ = messages.send(Message::Error(format!(
                                        "Recovery shutdown: {error}"
                                    )));
                                }
                                // A storage failure must retain recovery data, never prevent exit.
                                return Err("__shutdown__".into());
                            }
                            Request::Begin {
                                session,
                                targets,
                                revision: rev,
                                count_in,
                            } => {
                                if take.is_some() {
                                    return Err("A MIDI take is already open".into());
                                }
                                base = daw_project::recovery::digest(&session.project)
                                    .map_err(|e| e.to_string())?;
                                revision = rev;
                                interrupted = false;
                                retry_base = Some(session.clone());
                                take = Some(
                                    TakeJournal::begin(store.clone(), session, &targets, rev)
                                        .map_err(|e| e.to_string())?,
                                );
                                let _ = status.send(Status::BeginReady(count_in));
                            }
                            Request::Snapshot(snapshot, rev, folder) => {
                                if take.is_none()
                                    && !store.folder.join("active-take").exists()
                                    && !store.folder.join("recovered-pending").exists()
                                {
                                    store
                                        .checkpoint(snapshot, rev, folder, None)
                                        .map_err(|e| e.to_string())?;
                                }
                            }
                            Request::Acknowledge => {
                                TakeJournal::acknowledge(&store).map_err(|e| e.to_string())?;
                                retry_base = None;
                                let _ = messages.send(Message::TakeRetained(false));
                            }
                            Request::LeaveProject(id) => {
                                store.discard_project(id).map_err(|e| e.to_string())?
                            }
                            Request::Discard(entry) => {
                                RecoveryStore::discard(&entry).map_err(|e| e.to_string())?
                            }
                            Request::OpenSaved(entry) => {
                                if let Some(folder) = &entry.original_folder {
                                    let s = Session::open_document(folder)
                                        .map_err(|e| e.to_string())?
                                        .into_editor_session();
                                    let _ = messages.send(Message::Recovered(s));
                                }
                                RecoveryStore::discard(&entry).map_err(|e| e.to_string())?;
                            }
                            Request::Recover(entry) => {
                                let s =
                                    RecoveryStore::recover(&entry).map_err(|e| e.to_string())?;
                                if let Some(before) = &s.recovered_take_before {
                                    let checkpoint = store
                                        .checkpoint(
                                            before
                                                .save_snapshot(None)
                                                .map_err(|e| e.to_string())?,
                                            entry.revision,
                                            entry.original_folder.clone(),
                                            None,
                                        )
                                        .map_err(|e| e.to_string())?;
                                    store
                                        .checkpoint_take(
                                            s.save_snapshot(None).map_err(|e| e.to_string())?,
                                            entry.revision.saturating_add(1),
                                            entry.original_folder.clone(),
                                            entry.applied_take.unwrap_or_else(Id::new_v4),
                                            checkpoint.generation,
                                        )
                                        .map_err(|e| e.to_string())?;
                                    daw_project::recovery::mark_recovered_pending(&store)
                                        .map_err(|e| e.to_string())?;
                                } else {
                                    store
                                        .checkpoint(
                                            s.save_snapshot(None).map_err(|e| e.to_string())?,
                                            entry.revision.saturating_add(1),
                                            entry.original_folder.clone(),
                                            entry.applied_take,
                                        )
                                        .map_err(|e| e.to_string())?;
                                }
                                RecoveryStore::discard_claimed(&entry, &s)
                                    .map_err(|e| e.to_string())?;
                                let _ = messages.send(Message::Recovered(s));
                            }
                            Request::Retry => {
                                let entry = TakeJournal::recover_pending(&store)
                                    .map_err(|e| e.to_string())?
                                    .ok_or("No interrupted MIDI take is available")?;
                                let document = Session::open_document(&entry.generation)
                                    .map_err(|e| e.to_string())?
                                    .into_editor_session();
                                let original = retry_base
                                    .as_ref()
                                    .ok_or("Reopen the retained recovery after restarting DAW")?;
                                let mut candidate = original.clone();
                                candidate.project = document.project;
                                for asset in &mut candidate.project.assets {
                                    if let Some(before) =
                                        original.project.assets.iter().find(|a| a.id == asset.id)
                                    {
                                        asset.source = before.source.clone();
                                    }
                                }
                                candidate.project.validate().map_err(|e| e.to_string())?;
                                let _ = messages.send(Message::Take {
                                    base: base.clone(),
                                    session: candidate,
                                });
                            }
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        if error == "__shutdown__" {
                            return;
                        }
                        let _ = messages.send(Message::Error(error));
                        if recording_request && take.is_none() {
                            let _ = status.send(Status::Idle);
                            let _ = messages.send(Message::Recording(false));
                        }
                    }
                }
                for _ in 0..4096 {
                    let Ok(receipt) = receipts.pop() else {
                        break;
                    };
                    if interrupted {
                        continue;
                    }
                    let Some(journal) = &mut take else {
                        continue;
                    };
                    let result = match receipt.action {
                        MidiAction::Record { .. } => journal.start(receipt.frame),
                        MidiAction::Route { track, enabled } => journal.event(CaptureEvent {
                            frame: receipt.frame,
                            action: CaptureAction::Route { track, enabled },
                        }),
                        MidiAction::Attack {
                            token,
                            pitch,
                            velocity,
                        } => journal.event(CaptureEvent {
                            frame: receipt.frame,
                            action: CaptureAction::Attack {
                                token,
                                pitch,
                                velocity,
                            },
                        }),
                        MidiAction::ChannelAttack {
                            token,
                            pitch,
                            velocity,
                            channel,
                        } => journal.event(CaptureEvent {
                            frame: receipt.frame,
                            action: CaptureAction::ChannelAttack {
                                token,
                                pitch,
                                velocity,
                                channel,
                            },
                        }),
                        MidiAction::Release { token } => journal.event(CaptureEvent {
                            frame: receipt.frame,
                            action: CaptureAction::Release { token },
                        }),
                        _ => {
                            finish(
                                &mut take,
                                receipt.frame,
                                revision,
                                &base,
                                &messages,
                                &status,
                            );
                            continue;
                        }
                    };
                    if let Err(error) = result {
                        interrupted = true;
                        let _ = status.send(Status::Fault);
                        let _ = messages
                            .send(Message::Error(format!("MIDI take interrupted: {error}")));
                        terminal_worker.store(receipt.frame, Ordering::Release);
                    }
                }
                if receipts.is_empty() {
                    let cutoff = terminal_worker.swap(u64::MAX, Ordering::AcqRel);
                    if cutoff != u64::MAX {
                        finish(&mut take, cutoff, revision, &base, &messages, &status);
                    }
                }
                if synced.elapsed() >= Duration::from_millis(250) {
                    if let Some(j) = &mut take
                        && let Err(error) = j.sync()
                    {
                        interrupted = true;
                        let _ = status.send(Status::Fault);
                        let _ = messages.send(Message::Error(error.to_string()));
                        terminal_worker.store(0, Ordering::Release);
                    }
                    synced = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        Self {
            control,
            events,
            status: updates,
            terminal,
            last_frame: 0,
            worker: Some(worker),
        }
    }
    pub fn request(&self, request: Request) -> Result<(), String> {
        self.control
            .try_send(request)
            .map_err(|_| "Recovery worker is busy or unavailable".into())
    }
    pub fn packet(&mut self, receipt: MidiReceipt) -> Result<(), String> {
        if self.events.push(receipt).is_err() {
            self.interrupt(self.last_frame);
            return Err(
                "MIDI recording queue overflow; take interrupted at its accepted prefix".into(),
            );
        }
        self.last_frame = receipt.frame;
        Ok(())
    }
    pub fn interrupt(&self, frame: u64) {
        self.terminal.store(frame, Ordering::Release);
    }
    pub fn poll(&self) -> Option<Status> {
        self.status.try_recv().ok()
    }
}
fn finish(
    take: &mut Option<TakeJournal>,
    frame: u64,
    revision: u64,
    base: &str,
    messages: &Inbox,
    status: &mpsc::SyncSender<Status>,
) {
    let Some(journal) = take.take() else {
        return;
    };
    match journal.finish(frame, revision.saturating_add(1)) {
        Ok((session, _)) => {
            let _ = messages.send(Message::Take {
                base: base.into(),
                session,
            });
        }
        Err(error) => {
            let _ = messages.send(Message::TakeRetained(true));
            let _ = messages.send(Message::Error(format!(
                "The complete MIDI take is retained for recovery: {error}"
            )));
        }
    }
    let _ = messages.send(Message::Recording(false));
    let _ = status.send(Status::Idle);
}
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.control.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn wait_status(writer: &Writer, ready: impl Fn(Status) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if writer.poll().is_some_and(&ready) {
                return;
            }
            assert!(Instant::now() < deadline, "storage worker timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn wait_take(inbox: &Inbox) -> Session {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(Message::Take { session, .. }) = inbox.try_recv() {
                return session;
            }
            assert!(Instant::now() < deadline, "take publication timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn pending_whole_take_survives_autosave_and_retry_keeps_its_identities() {
        let root = std::env::temp_dir().join(format!("daw-writer-pending-{}", Id::new_v4()));
        let inbox = Inbox::default();
        let mut writer = Writer::new(root.clone(), inbox.clone());
        wait_status(&writer, |s| matches!(s, Status::StorageReady));
        let mut session = Session::default();
        let a = session.project.add_midi_track().unwrap();
        let b = session.project.add_midi_track().unwrap();
        writer
            .request(Request::Begin {
                session: session.clone(),
                targets: vec![a, b],
                revision: 0,
                count_in: 0,
            })
            .unwrap();
        wait_status(&writer, |s| matches!(s, Status::BeginReady(0)));
        let token = Id::new_v4();
        for (sequence, (frame, action)) in [
            (0, MidiAction::Record { count_in_frames: 0 }),
            (
                0,
                MidiAction::Attack {
                    token,
                    pitch: 60,
                    velocity: 100,
                },
            ),
            (24000, MidiAction::Release { token }),
            (24000, MidiAction::Seal),
        ]
        .into_iter()
        .enumerate()
        {
            writer
                .packet(MidiReceipt {
                    action,
                    frame,
                    clock: frame,
                    sequence: sequence as u64,
                })
                .unwrap();
        }
        wait_status(&writer, |s| matches!(s, Status::Idle));
        let first = wait_take(&inbox);
        assert_eq!(first.project.tracks[0].clips.len(), 1);
        assert_eq!(first.project.tracks[1].clips.len(), 1);
        writer
            .request(Request::Snapshot(
                session.save_snapshot(None).unwrap(),
                99,
                None,
            ))
            .unwrap();
        writer.request(Request::Retry).unwrap();
        let retry = wait_take(&inbox);
        assert_eq!(retry.project, first.project);
        let owner = std::fs::read_dir(&root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let generations = std::fs::read_dir(owner)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("recovery.json").exists())
            .count();
        assert_eq!(
            generations, 2,
            "autosave must not supersede an unacknowledged take"
        );
        drop(writer);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn storage_failure_during_shutdown_does_not_block_exit() {
        let root = std::env::temp_dir().join(format!("daw-writer-exit-{}", Id::new_v4()));
        let writer = Writer::new(root.clone(), Inbox::default());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if matches!(writer.poll(), Some(Status::StorageReady)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "recovery writer did not initialize"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        // Simulate storage disappearing after initialization. No user data is involved.
        std::fs::remove_dir_all(&root).unwrap();
        let (finished, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            drop(writer);
            let _ = finished.send(());
        });
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("storage failure must not prevent shutdown");
    }
}
