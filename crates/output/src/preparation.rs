//! Latest-request preparation on a control worker. The audio callback never accesses this state.
use super::*;
use std::sync::{Condvar, Mutex};

#[derive(Clone, Default)]
pub(super) struct Bindings {
    pub tracks: HashMap<Id, Arc<StereoMeter>>,
    pub effects: HashMap<Id, Arc<AtomicU32>>,
    pub bands: HashMap<Id, Arc<[AtomicU32; 3]>>,
    pub modulation: HashMap<ModulationTarget, ModulationMeter>,
}
pub(super) struct Ready {
    pub revision: u64,
    pub renderer: Box<PreparedRenderer>,
    pub bindings: Bindings,
}
enum Request {
    Plan(Arc<RenderPlan>, Option<daw_engine::TransportState>),
    Prepared(Box<PreparedRenderer>, daw_engine::TransportState),
}
#[derive(Default)]
struct State {
    revision: u64,
    request: Option<(u64, Request, Bindings)>,
    ready: Option<Ready>,
    closed: bool,
    wake_ui: Option<Arc<dyn Fn() + Send + Sync>>,
}
pub(super) struct Worker(Arc<(Mutex<State>, Condvar)>);
impl Worker {
    pub fn new() -> Result<Self, String> {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("audio-preparation".into())
            .spawn(move || {
                loop {
                    let (revision, plan, previous) = {
                        let (lock, wake) = &*worker;
                        let mut state = lock.lock().unwrap();
                        while state.request.is_none() && !state.closed {
                            state = wake.wait(state).unwrap();
                        }
                        if state.closed {
                            return;
                        }
                        state.request.take().unwrap()
                    };
                    let (mut renderer, bindings, transport) = match plan {
                        Request::Plan(plan, transport) => {
                            let bindings = Bindings {
                                tracks: track_meters(&plan, &previous.tracks),
                                effects: effect_meters(&plan, &previous.effects),
                                bands: band_meters(&plan, &previous.bands),
                                modulation: modulation_meters(&plan, &previous.modulation),
                            };
                            let renderer =
                                Box::new(PreparedRenderer::new((*plan).clone(), &bindings.tracks));
                            (renderer, bindings, transport)
                        }
                        Request::Prepared(renderer, transport) => {
                            // Reconstruct unbound DSP so worker pre-roll cannot write active meters.
                            let plan = renderer.plan.clone();
                            drop(renderer);
                            let renderer = Box::new(PreparedRenderer::new(plan, &previous.tracks));
                            (renderer, previous, Some(transport))
                        }
                    };
                    renderer.revision = revision;
                    if let Some(transport) = transport {
                        renderer.prepare_transport(transport);
                        renderer.prepared_transport = true;
                    }
                    bind_effect_meters(&mut renderer, &bindings.effects);
                    bind_band_meters(&mut renderer, &bindings.bands);
                    renderer.bind_modulation(&bindings.modulation);
                    renderer.ready_at = Some(std::time::Instant::now());
                    let ready = Ready {
                        revision,
                        renderer,
                        bindings,
                    };
                    let (retired, wake_ui) = {
                        let mut state = worker.0.lock().unwrap();
                        if state.closed || revision != state.revision {
                            (Some(ready), None)
                        } else {
                            (state.ready.replace(ready), state.wake_ui.clone())
                        }
                    };
                    drop(retired);
                    if let Some(wake_ui) = wake_ui {
                        wake_ui();
                    }
                }
            })
            .map_err(|e| format!("Cannot start audio preparation: {e}"))?;
        Ok(Self(shared))
    }
    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        let mut state = self.0.0.lock().unwrap();
        if state.wake_ui.is_none() {
            state.wake_ui = Some(Arc::new(wake));
        }
    }
    pub fn submit(
        &self,
        plan: impl Into<Arc<RenderPlan>>,
        previous: Bindings,
        transport: Option<daw_engine::TransportState>,
    ) -> u64 {
        let (request, ready, revision) = {
            let mut state = self.0.0.lock().unwrap();
            state.revision += 1;
            let revision = state.revision;
            (
                state
                    .request
                    .replace((revision, Request::Plan(plan.into(), transport), previous)),
                state.ready.take(),
                revision,
            )
        };
        self.0.1.notify_one();
        drop((request, ready));
        revision
    }
    pub fn reprepare(&self, ready: Ready, transport: daw_engine::TransportState) {
        let mut state = self.0.0.lock().unwrap();
        if state.revision == ready.revision {
            state.request = Some((
                ready.revision,
                Request::Prepared(ready.renderer, transport),
                ready.bindings,
            ));
            self.0.1.notify_one();
        }
    }
    pub fn cancel(&self) -> u64 {
        let (revision, request, ready) = {
            let mut state = self.0.0.lock().unwrap();
            state.revision += 1;
            (state.revision, state.request.take(), state.ready.take())
        };
        drop((request, ready));
        revision
    }
    pub fn take(&self) -> Option<Ready> {
        self.0.0.lock().unwrap().ready.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let retired = {
            let mut state = self.0.0.lock().unwrap();
            state.closed = true;
            (state.request.take(), state.ready.take())
        };
        self.0.1.notify_one();
        // An in-flight preparation completes on its worker. UI shutdown does not join it.
        drop(retired);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_coalesces_requests_and_keeps_latest_settings() {
        let worker = Worker::new().unwrap();
        let mut project = daw_core::Project::default();
        project.add_track().unwrap();
        let mut revision = 0;
        for gain in 0..32 {
            project.master.gain_db = -(gain as f32);
            revision = worker.submit(
                RenderPlan {
                    project: project.clone(),
                    audio: Default::default(),
                    stretched_audio: Default::default(),
                },
                Bindings::default(),
                None,
            );
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(ready) = worker.take() {
                assert_eq!(ready.revision, revision);
                assert_eq!(ready.renderer.plan.project.master.gain_db, -31.0);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
