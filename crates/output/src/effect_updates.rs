//! Independent, latest-request effect preparation. Exact transport replay never blocks this worker.
use super::*;
use std::sync::{Condvar, Mutex};

pub(super) struct Request {
    pub revision: u64,
    pub base_revision: u64,
    pub base_publication: u64,
    pub previous: Arc<RenderPlan>,
    pub desired: Arc<RenderPlan>,
    pub bindings: preparation::Bindings,
    pub partial_only: bool,
}
pub(super) struct Prepared {
    pub publication: u64,
    pub revision: u64,
    pub base_revision: u64,
    pub base_publication: u64,
    pub update: daw_engine::EffectUpdate,
    pub memory: performance::RendererMemoryLease,
    pub modulation: Vec<(ModulationTarget, ModulationMeter)>,
    pub requested_at: std::time::Instant,
    pub applied: bool,
    pub complete: bool,
}
pub(super) struct Ready {
    pub revision: u64,
    pub desired: Arc<RenderPlan>,
    pub bindings: preparation::Bindings,
    pub prepared: Result<Option<Box<Prepared>>, String>,
    pub fallback: Option<daw_engine::EffectUpdateFallback>,
}
#[derive(Default)]
struct State {
    revision: u64,
    request: Option<Request>,
    ready: Option<Ready>,
    closed: bool,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
pub(super) struct Worker(Arc<(Mutex<State>, Condvar)>);
impl Worker {
    pub fn new() -> Result<Self, String> {
        Self::start(false)
    }
    pub fn controls() -> Result<Self, String> {
        Self::start(true)
    }
    fn start(controls: bool) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let shared = state.clone();
        std::thread::Builder::new()
            .name(
                if controls {
                    "effect-controls"
                } else {
                    "effect-preparation"
                }
                .into(),
            )
            .spawn(move || {
                loop {
                    let request = {
                        let mut state = shared.0.lock().unwrap();
                        while !state.closed && state.request.is_none() {
                            state = shared.1.wait(state).unwrap();
                        }
                        if state.closed {
                            return;
                        }
                        state.request.take().unwrap()
                    };
                    let requested_at = std::time::Instant::now();
                    let bindings = if controls {
                        request.bindings.clone()
                    } else {
                        preparation::Bindings::prepare(&request.desired, &request.bindings)
                    };
                    let (update, fallback) = if controls {
                        (
                            daw_engine::EffectUpdate::prepare_controls(
                                &request.previous,
                                &request.desired,
                            ),
                            None,
                        )
                    } else {
                        match daw_engine::EffectUpdate::prepare_with_reason(
                            &request.previous,
                            (*request.desired).clone(),
                        ) {
                            Ok(update) => (Some(update), None),
                            Err(reason) => (None, Some(reason)),
                        }
                    };
                    let effective = if controls {
                        update
                            .as_ref()
                            .map(|u| Arc::new(u.plan().clone()))
                            .unwrap_or_else(|| request.previous.clone())
                    } else {
                        request.desired.clone()
                    };
                    let complete = !controls
                        || (!request.partial_only
                            && effective.same_processing_plan(
                                &request.desired.as_ref().clone().prepare_beats(),
                            ));
                    let prepared = update
                        .map(|mut update| {
                            let memory =
                                performance::RendererMemoryLease::try_new(update.heap_bytes())?;
                            update.bind_meters(
                                &bindings.effects,
                                &bindings.outputs,
                                &bindings.bands,
                                &bindings.loudness,
                                &bindings.spectra,
                                &bindings.exciter_solos,
                            );
                            Ok(Box::new(Prepared {
                                publication: 0,
                                revision: request.revision,
                                base_revision: request.base_revision,
                                base_publication: request.base_publication,
                                update,
                                memory,
                                modulation: bindings
                                    .modulation
                                    .iter()
                                    .map(|(k, v)| (*k, v.clone()))
                                    .collect(),
                                requested_at,
                                applied: false,
                                complete,
                            }))
                        })
                        .transpose();
                    let wake = {
                        let mut state = shared.0.lock().unwrap();
                        if state.closed || state.revision != request.revision {
                            continue;
                        }
                        state.ready = Some(Ready {
                            revision: request.revision,
                            desired: effective,
                            bindings,
                            prepared,
                            fallback,
                        });
                        state.wake.clone()
                    };
                    if let Some(wake) = wake {
                        wake();
                    }
                }
            })
            .map_err(|e| format!("Cannot start effect preparation: {e}"))?;
        Ok(Self(state))
    }
    pub fn submit(&self, request: Request) {
        let mut state = self.0.0.lock().unwrap();
        state.revision = request.revision;
        state.request = Some(request);
        state.ready = None;
        self.0.1.notify_one();
    }
    pub fn cancel(&self, revision: u64) {
        let mut state = self.0.0.lock().unwrap();
        state.revision = revision;
        state.request = None;
        state.ready = None;
    }
    pub fn take(&self) -> Option<Ready> {
        self.0.0.lock().unwrap().ready.take()
    }
    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.0.0.lock().unwrap().wake = Some(Arc::new(wake));
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let mut state = self.0.0.lock().unwrap();
        state.closed = true;
        state.request = None;
        state.ready = None;
        self.0.1.notify_one();
    }
}
