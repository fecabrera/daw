//! Latest-request preparation on a control worker. The audio callback never accesses this state.
use super::cache_schedule::{CacheControls, WarmSchedule};
use super::*;
use std::sync::{Condvar, Mutex};

#[derive(Clone, Default)]
pub(super) struct Bindings {
    pub tracks: HashMap<Id, Arc<StereoMeter>>,
    pub effects: HashMap<Id, Arc<daw_unit_core::PeakMeter>>,
    pub bands: HashMap<ModulationTarget, Arc<daw_unit_core::BandMeter<3>>>,
    pub modulation: HashMap<ModulationTarget, ModulationMeter>,
    pub live_pitches: daw_engine::LiveMidiPitches,
}
pub(super) struct Ready {
    pub revision: u64,
    pub renderer: Box<PreparedRenderer>,
    pub bindings: Bindings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Anchor {
    pub transport: TransportState,
    pub clock: u64,
}
enum Request {
    Warm(Arc<RenderPlan>),
    Plan(
        Arc<RenderPlan>,
        Option<daw_engine::TransportState>,
        std::time::Instant,
    ),
    Prepared(Box<PreparedRenderer>, Anchor),
}
#[derive(Default)]
struct State {
    revision: u64,
    cache_controls: CacheControls,
    request: Option<(u64, Request, Bindings)>,
    ready: Option<Ready>,
    closed: bool,
    error: Option<(u64, String)>,
    wake_ui: Option<Arc<dyn Fn() + Send + Sync>>,
}
pub(super) struct Worker(Arc<(Mutex<State>, Condvar)>);
impl Worker {
    pub fn new(status: Arc<Status>) -> Result<Self, String> {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("audio-preparation".into())
            .spawn(move || {
                let mut cached_plan: Option<Arc<RenderPlan>> = None;
                let mut cache: Option<daw_engine::TransportCache> = None;
                let mut needs_cache = false;
                let mut applied_controls = 0;
                let mut schedule = WarmSchedule::default();
                loop {
                    let (request, controls) = {
                        let (lock, wake) = &*worker;
                        let mut state = lock.lock().unwrap();
                        while state.request.is_none() && !state.closed {
                            let can_warm =
                                needs_cache || cache.as_ref().is_some_and(|c| c.can_warm());
                            let delay = schedule.delay(
                                state.cache_controls,
                                status.playing.load(Ordering::Relaxed),
                                status.performance.overruns(),
                                std::time::Instant::now(),
                            );
                            if state.cache_controls.revision != applied_controls
                                || (can_warm && delay.is_zero())
                            {
                                break;
                            }
                            state = if can_warm {
                                wake.wait_timeout(
                                    state,
                                    delay.min(std::time::Duration::from_millis(100)),
                                )
                                .unwrap()
                                .0
                            } else {
                                wake.wait(state).unwrap()
                            };
                        }
                        if state.closed {
                            return;
                        }
                        (state.request.take(), state.cache_controls)
                    };
                    if applied_controls != controls.revision {
                        if let Some(cache) = &mut cache {
                            cache.set_profile(controls.profile);
                            status.performance.cache(cache.memory_usage());
                        }
                        applied_controls = controls.revision;
                    }
                    let policy = controls.profile.policy();
                    let Some((revision, plan, previous)) = request else {
                        if schedule
                            .delay(
                                controls,
                                status.playing.load(Ordering::Relaxed),
                                status.performance.overruns(),
                                std::time::Instant::now(),
                            )
                            .is_zero()
                        {
                            if needs_cache {
                                if let Some(plan) = &cached_plan {
                                    cache = Some(daw_engine::TransportCache::with_profile(
                                        (**plan).clone(),
                                        controls.profile,
                                    ));
                                }
                                needs_cache = false;
                            }
                            if let Some(cache) = &mut cache {
                                cache.warm_step(policy.work_slice, || {
                                    let state = worker.0.lock().unwrap();
                                    state.closed
                                        || state.request.is_some()
                                        || state.cache_controls.revision != controls.revision
                                        || (status.playing.load(Ordering::Relaxed)
                                            && !policy.warm_while_playing)
                                        || status.performance.overruns() != schedule.overrun_count()
                                });
                                status.performance.cache(cache.memory_usage());
                            }
                        }
                        // Optional preparation yields CPU and wakes immediately for a request.
                        let state = worker.0.lock().unwrap();
                        if !state.closed
                            && state.request.is_none()
                            && state.cache_controls.revision == controls.revision
                        {
                            drop(worker.1.wait_timeout(state, policy.idle_wait).unwrap());
                        }
                        continue;
                    };
                    let (mut renderer, bindings, transport, anchor) = match plan {
                        Request::Warm(plan) => {
                            if cached_plan
                                .as_ref()
                                .is_none_or(|p| !p.same_processing_plan(&plan))
                            {
                                cache = None;
                                status.performance.cache(Default::default());
                                cached_plan = Some(plan);
                            }
                            needs_cache = cache.is_none();
                            if let Some(cache) = &cache {
                                status.performance.cache(cache.memory_usage());
                            }
                            continue;
                        }
                        Request::Plan(plan, transport, requested_at) => {
                            if cached_plan.as_ref().is_none_or(|p| {
                                !Arc::ptr_eq(p, &plan) && !p.same_processing_plan(&plan)
                            }) {
                                cache = None;
                                status.performance.cache(Default::default());
                                cached_plan = Some(plan.clone());
                            }
                            let bindings = Bindings {
                                tracks: track_meters(&plan, &previous.tracks),
                                effects: effect_meters(&plan, &previous.effects),
                                bands: band_meters(&plan, &previous.bands),
                                modulation: modulation_meters(&plan, &previous.modulation),
                                live_pitches: daw_engine::LiveMidiPitches::prepare(
                                    &plan.project,
                                    &previous.live_pitches,
                                ),
                            };
                            let renderer = match PreparedRenderer::try_new(
                                (*plan).clone(),
                                &bindings.tracks,
                            ) {
                                Ok(renderer) => Box::new(renderer),
                                Err(error) => {
                                    cache = None;
                                    needs_cache = false;
                                    status.performance.cache(Default::default());
                                    publish_error(&worker, revision, error);
                                    continue;
                                }
                            };
                            let mut renderer = renderer;
                            renderer.requested_at = Some(requested_at);
                            (renderer, bindings, transport, None)
                        }
                        Request::Prepared(renderer, anchor) => {
                            let lead = renderer.preparation_lead;
                            let requested_at = renderer.requested_at;
                            // Reconstruct unbound DSP so worker pre-roll cannot write active meters.
                            let plan = renderer.plan.clone();
                            drop(renderer);
                            let mut renderer =
                                match PreparedRenderer::try_new(plan, &previous.tracks) {
                                    Ok(renderer) => Box::new(renderer),
                                    Err(error) => {
                                        cache = None;
                                        needs_cache = false;
                                        status.performance.cache(Default::default());
                                        publish_error(&worker, revision, error);
                                        continue;
                                    }
                                };
                            renderer.preparation_lead = lead;
                            renderer.requested_at = requested_at;
                            (renderer, previous, Some(anchor.transport), Some(anchor))
                        }
                    };
                    renderer.revision = revision;
                    if let Some(transport) = transport {
                        needs_cache = false;
                        let cache = cache.get_or_insert_with(|| {
                            daw_engine::TransportCache::with_profile(
                                renderer.plan.clone(),
                                controls.profile,
                            )
                        });
                        let Some(prepared) = cache.prepare(transport, || {
                            let state = worker.0.lock().unwrap();
                            state.closed || state.revision != revision
                        }) else {
                            continue;
                        };
                        status.performance.cache(cache.memory_usage());
                        renderer.renderer = prepared;
                        renderer.prepared_transport = true;
                    }
                    if let Some(anchor) = anchor.filter(|a| a.transport.playing) {
                        // Replay through the shared live renderer, including loop wraps,
                        // latency compensation, and all modulation state. Stay ahead of
                        // the callback; the callback only swaps at the exact sample clock.
                        let lead = renderer
                            .preparation_lead
                            .max(u64::from(renderer.plan.project.sample_rate_hz) / 10);
                        renderer.preparation_lead = lead;
                        let mut clock = anchor.clock;
                        loop {
                            let target = status
                                .rendered_frames
                                .load(Ordering::Acquire)
                                .max(clock)
                                .saturating_add(lead);
                            let block = u64::from(renderer.plan.project.buffer_size_frames.max(1));
                            while clock < target {
                                if clock % block == 0 {
                                    let state = worker.0.lock().unwrap();
                                    if state.closed || state.revision != revision {
                                        break;
                                    }
                                }
                                renderer.next_sample();
                                clock += 1;
                            }
                            let state = worker.0.lock().unwrap();
                            if state.closed || state.revision != revision {
                                break;
                            }
                            drop(state);
                            // Leave at least half the delivery lead after catch-up.
                            if clock
                                > status
                                    .rendered_frames
                                    .load(Ordering::Acquire)
                                    .saturating_add(lead / 2)
                            {
                                renderer.activate_at = Some(clock);
                                break;
                            }
                        }
                        if renderer.activate_at.is_none() {
                            continue;
                        }
                    }
                    renderer.reset_track_peaks();
                    bind_effect_meters(&mut renderer, &bindings.effects);
                    bind_band_meters(&mut renderer, &bindings.bands);
                    renderer.bind_modulation(&bindings.modulation);
                    renderer.bind_live_midi_pitches(&bindings.live_pitches);
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
                    if cache.is_none() {
                        needs_cache = cached_plan.is_some();
                    }
                    if let Some(cache) = &cache {
                        status.performance.cache(cache.memory_usage());
                    }
                }
            })
            .map_err(|e| format!("Cannot start audio preparation: {e}"))?;
        Ok(Self(shared))
    }
    pub fn configure_cache(&self, profile: daw_engine::CacheProfile, editing: bool) {
        let mut state = self.0.0.lock().unwrap();
        if state.cache_controls.profile != profile || state.cache_controls.editing != editing {
            state.cache_controls.profile = profile;
            state.cache_controls.editing = editing;
            state.cache_controls.last_activity = Some(std::time::Instant::now());
            state.cache_controls.revision += 1;
            self.0.1.notify_one();
        }
    }
    pub fn prewarm(&self, plan: Arc<RenderPlan>) {
        let previous = {
            let mut state = self.0.0.lock().unwrap();
            state.revision += 1;
            state.error = None;
            state.cache_controls.last_activity = Some(std::time::Instant::now());
            let revision = state.revision;
            state
                .request
                .replace((revision, Request::Warm(plan), Bindings::default()))
        };
        self.0.1.notify_one();
        drop(previous);
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
            state.error = None;
            state.cache_controls.last_activity = Some(std::time::Instant::now());
            let revision = state.revision;
            (
                state.request.replace((
                    revision,
                    Request::Plan(plan.into(), transport, std::time::Instant::now()),
                    previous,
                )),
                state.ready.take(),
                revision,
            )
        };
        self.0.1.notify_one();
        drop((request, ready));
        revision
    }
    pub fn reprepare(&self, ready: Ready, transport: Anchor) {
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
    pub fn take_error(&self) -> Option<String> {
        let mut state = self.0.0.lock().unwrap();
        let (revision, error) = state.error.take()?;
        (revision == state.revision).then_some(error)
    }
    pub fn take(&self) -> Option<Ready> {
        self.0.0.lock().unwrap().ready.take()
    }
}
fn publish_error(worker: &Arc<(Mutex<State>, Condvar)>, revision: u64, error: String) {
    let wake = {
        let mut state = worker.0.lock().unwrap();
        if state.closed || state.revision != revision {
            return;
        }
        state.error = Some((revision, error));
        state.wake_ui.clone()
    };
    if let Some(wake) = wake {
        wake();
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
        let worker = Worker::new(Arc::new(Status::default())).unwrap();
        let mut project = daw_core::Project::default();
        project.add_track().unwrap();
        let mut revision = 0;
        for gain in 0..32 {
            project.master.gain_db = -(gain as f32);
            revision = worker.submit(
                RenderPlan {
                    project: project.clone(),
                    audio: Default::default(),
                    sampler_audio: Default::default(),
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
