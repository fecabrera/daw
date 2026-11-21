//! Latest-request preparation on a control worker. The audio callback never accesses this state.
use super::cache_schedule::{CacheControls, WarmSchedule};
use super::*;
use std::sync::{Condvar, Mutex};

/// Device startup owns only a silent clock. Logical content is compiled by Worker.
pub(super) fn initial_plan(plan: &RenderPlan) -> RenderPlan {
    let project = daw_core::Project {
        project_id: plan.project.project_id,
        sample_rate_hz: plan.project.sample_rate_hz,
        output_sample_format: plan.project.output_sample_format,
        buffer_size_frames: plan.project.buffer_size_frames,
        tempo_bpm: plan.project.tempo_bpm,
        time_signature: plan.project.time_signature,
        timing: plan.project.timing.clone(),
        markers: plan.project.markers.clone(),
        stretch_engine: plan.project.stretch_engine,
        transport: plan.project.transport.clone(),
        ..Default::default()
    };
    RenderPlan {
        external: Default::default(),
        project,
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}

/// The logical and compiled plans share prepared PCM. Compare before lowering so
/// Warm followed by first Play does not rebuild schedules or derived identities.
#[derive(Default)]
struct BeatPlans {
    logical: Option<Arc<RenderPlan>>,
    compiled: Option<Arc<RenderPlan>>,
}
impl BeatPlans {
    fn resolve(&mut self, plan: Arc<RenderPlan>) -> Arc<RenderPlan> {
        if !plan
            .project
            .tracks
            .iter()
            .any(|t| t.kind == daw_core::TrackKind::Beat)
        {
            self.logical = None;
            self.compiled = None;
            return plan;
        }
        if let Some(previous) = &self.logical
            && (Arc::ptr_eq(previous, &plan) || previous.same_processing_plan(&plan))
        {
            let compiled = self.compiled.as_ref().unwrap().clone();
            self.logical = Some(plan);
            return compiled;
        }
        let compiled = Arc::new((*plan).clone().prepare_beats());
        self.logical = Some(plan);
        self.compiled = Some(compiled.clone());
        compiled
    }
}

#[derive(Clone, Default)]
pub(super) struct Bindings {
    pub tracks: HashMap<Id, Arc<StereoMeter>>,
    pub layers: layer_meters::Layers,
    pub effects: HashMap<Id, Arc<daw_unit_core::PeakMeter>>,
    pub exciter_solos: effect_audition::Solos,
    pub outputs: HashMap<Id, Arc<daw_unit_core::BandMeter<3>>>,
    pub loudness: HashMap<Id, Arc<daw_unit_core::LoudnessMeter>>,
    pub sources: source_meters::Sources,
    pub bands: HashMap<ModulationTarget, Arc<daw_unit_core::BandMeter<3>>>,
    pub spectra: effect_spectrum::Spectra,
    pub modulation: HashMap<ModulationTarget, ModulationMeter>,
    pub live_pitches: daw_engine::LiveMidiPitches,
}
impl Bindings {
    /// Attach shared handle families. Constructors own the ordered track meters.
    pub(super) fn attach(&self, renderer: &mut PreparedRenderer) {
        effect_audition::bind(renderer, &self.exciter_solos);
        bind_effect_meters(renderer, &self.effects);
        bind_band_meters(renderer, &self.bands);
        effect_spectrum::bind(renderer, &self.spectra);
        bind_output_meters(renderer, &self.outputs);
        bind_loudness_meters(renderer, &self.loudness);
        source_meters::bind(renderer, &self.sources);
        renderer.layer_meters = self.layers.clone();
        renderer.bind_modulation(&self.modulation);
        renderer.bind_live_midi_pitches(&self.live_pitches);
    }
    pub fn prepare(plan: &RenderPlan, previous: &Self) -> Self {
        Self {
            tracks: track_meters(plan, &previous.tracks),
            layers: layer_meters::prepare(plan, &previous.layers),
            effects: effect_meters(plan, &previous.effects),
            exciter_solos: effect_audition::prepare(plan, &previous.exciter_solos),
            outputs: output_meters(plan, &previous.outputs),
            loudness: loudness_meters(plan, &previous.loudness),
            sources: source_meters::prepare(plan, &previous.sources),
            bands: band_meters(plan, &previous.bands),
            spectra: effect_spectrum::prepare(plan, &previous.spectra),
            modulation: modulation_meters(plan, &previous.modulation),
            live_pitches: daw_engine::LiveMidiPitches::prepare(
                &plan.project,
                &previous.live_pitches,
            ),
        }
    }
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
#[derive(Clone, Copy)]
pub(super) struct TransportRequest {
    pub state: TransportState,
    pub fast: bool,
}
impl From<TransportState> for TransportRequest {
    fn from(state: TransportState) -> Self {
        Self { state, fast: false }
    }
}
pub(super) struct LiveInsertBase {
    pub publication: u64,
    pub edit: LiveInsertEdit,
}
pub(super) enum LiveInsertEdit {
    Effect(Arc<RenderPlan>),
    Builtin(Id),
}
enum Request {
    Warm(Arc<RenderPlan>, Option<Arc<RenderPlan>>),
    Plan(
        Arc<RenderPlan>,
        Option<TransportRequest>,
        std::time::Instant,
        Option<Arc<RenderPlan>>,
        Option<LiveInsertBase>,
    ),
    Prepared(Box<PreparedRenderer>, Anchor),
}

fn prepare_fast_seek(
    renderer: &mut PreparedRenderer,
    state: TransportState,
    cancelled: impl Fn() -> bool,
) -> Result<bool, String> {
    if cancelled() {
        return Ok(false);
    }
    renderer.prepare_fast_seek(state)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if cancelled() {
            return Ok(false);
        }
        if renderer.fast_seek_streaming_ready() {
            return Ok(true);
        }
        if let Some(error) = renderer.source_error() {
            return Err(error);
        }
        if std::time::Instant::now() >= deadline {
            return Err("Timed out preparing audio at the seek destination".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn resolve_cache_plan(
    beat_plans: &mut BeatPlans,
    cached_plan: &mut Option<Arc<RenderPlan>>,
    cache: &mut Option<daw_engine::TransportCache>,
    plan: Arc<RenderPlan>,
    predecessor: Option<Arc<RenderPlan>>,
    previous_was_donor: bool,
) -> (Arc<RenderPlan>, bool) {
    let matches_predecessor = predecessor.is_some_and(|predecessor| {
        cache
            .as_ref()
            .is_some_and(|cache| cache.matches_predecessor(&beat_plans.resolve(predecessor)))
    });
    let plan = beat_plans.resolve(plan);
    let retained = matches_predecessor
        && cache
            .as_ref()
            .is_some_and(|cache| cache.can_retain_for_append(&plan));
    if !retained
        && (previous_was_donor
            || cached_plan
                .as_ref()
                .is_none_or(|p| !Arc::ptr_eq(p, &plan) && !p.same_processing_plan(&plan)))
    {
        *cache = None;
    }
    *cached_plan = Some(plan.clone());
    (plan, retained)
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
                let mut beat_plans = BeatPlans::default();
                let mut cache: Option<daw_engine::TransportCache> = None;
                let mut donor_revision = None;
                let mut needs_cache = false;
                let mut applied_controls = 0;
                let mut schedule = WarmSchedule::default();
                'requests: loop {
                    let (request, controls, latest) = {
                        let (lock, wake) = &*worker;
                        let mut state = lock.lock().unwrap();
                        while state.request.is_none() && !state.closed {
                            let can_warm = donor_revision.is_none()
                                && (needs_cache || cache.as_ref().is_some_and(|c| c.can_warm()));
                            let delay = schedule.delay(
                                state.cache_controls,
                                status.playing.load(Ordering::Relaxed),
                                status.performance.overruns(),
                                std::time::Instant::now(),
                            );
                            if state.cache_controls.revision != applied_controls
                                || (can_warm && delay.is_zero())
                                || donor_revision.is_some_and(|revision| revision != state.revision)
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
                        (state.request.take(), state.cache_controls, state.revision)
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
                        if let Some(donor) = donor_revision {
                            if donor != latest {
                                donor_revision = None;
                                cache = None;
                                cached_plan = None;
                                status.performance.cache(Default::default());
                            }
                            continue;
                        }
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
                                if let Some(plan) = &cached_plan
                                    && plan.project.hosted_units().is_empty()
                                {
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
                    let mut allow_checkpoint_reuse = false;
                    let (mut renderer, bindings, transport, anchor) = match plan {
                        Request::Warm(plan, predecessor) => {
                            let (_, retained) = resolve_cache_plan(
                                &mut beat_plans,
                                &mut cached_plan,
                                &mut cache,
                                plan,
                                predecessor,
                                donor_revision.take().is_some(),
                            );
                            donor_revision = retained.then_some(revision);
                            needs_cache = cache.is_none()
                                && cached_plan
                                    .as_ref()
                                    .is_some_and(|p| p.project.hosted_units().is_empty());
                            status.performance.cache(cache.as_ref().map_or_else(
                                Default::default,
                                daw_engine::TransportCache::memory_usage,
                            ));
                            continue;
                        }
                        Request::Plan(plan, transport, requested_at, predecessor, live_insert) => {
                            let live_insert = live_insert.and_then(|base| {
                                let kind = match base.edit {
                                    LiveInsertEdit::Effect(previous) => {
                                        if plan.project.hosted_units().is_empty() {
                                            LiveInsertKind::Builtin(
                                                previous.builtin_effect_inserted(&plan)?,
                                            )
                                        } else {
                                            LiveInsertKind::Hosted(
                                                previous.hosted_effect_inserted(&plan)?,
                                            )
                                        }
                                    }
                                    LiveInsertEdit::Builtin(effect) => {
                                        LiveInsertKind::Builtin(effect)
                                    }
                                };
                                Some(LiveInsert {
                                    base_publication: base.publication,
                                    kind,
                                })
                            });
                            let (plan, retained) = resolve_cache_plan(
                                &mut beat_plans,
                                &mut cached_plan,
                                &mut cache,
                                plan,
                                predecessor,
                                donor_revision.take().is_some(),
                            );
                            donor_revision = retained.then_some(revision);
                            if cache.is_none() {
                                status.performance.cache(Default::default());
                            }
                            let bindings = Bindings::prepare(&plan, &previous);
                            let renderer = match PreparedRenderer::try_new_with_insert(
                                (*plan).clone(),
                                &bindings.tracks,
                                live_insert,
                            ) {
                                Ok(renderer) => Box::new(renderer),
                                Err(error) => {
                                    cache = None;
                                    donor_revision = None;
                                    needs_cache = false;
                                    status.performance.cache(Default::default());
                                    publish_error(&worker, revision, error);
                                    continue;
                                }
                            };
                            let mut renderer = renderer;
                            renderer.allow_checkpoint_reuse = retained;
                            renderer.requested_at = Some(requested_at);
                            (renderer, bindings, transport, None)
                        }
                        Request::Prepared(renderer, anchor) => {
                            allow_checkpoint_reuse = renderer.allow_checkpoint_reuse;
                            let lead = renderer.preparation_lead;
                            let requested_at = renderer.requested_at;
                            // Reconstruct unbound DSP so worker pre-roll cannot write active meters.
                            let plan = renderer.plan.clone();
                            drop(renderer);
                            // Active bindings can predate tracks added by this candidate.
                            // Reconcile every binding with the candidate before retrying.
                            let bindings = Bindings::prepare(&plan, &previous);
                            let mut renderer =
                                match PreparedRenderer::try_new(plan, &bindings.tracks) {
                                    Ok(renderer) => Box::new(renderer),
                                    Err(error) => {
                                        cache = None;
                                        donor_revision = None;
                                        needs_cache = false;
                                        status.performance.cache(Default::default());
                                        publish_error(&worker, revision, error);
                                        continue;
                                    }
                                };
                            renderer.preparation_lead = lead;
                            renderer.requested_at = requested_at;
                            (
                                renderer,
                                bindings,
                                Some(anchor.transport.into()),
                                Some(anchor),
                            )
                        }
                    };
                    renderer.revision = revision;
                    if let Some(request) = transport {
                        let transport = request.state;
                        needs_cache = false;
                        if request.fast {
                            match prepare_fast_seek(&mut renderer, transport, || {
                                let state = worker.0.lock().unwrap();
                                state.closed || state.revision != revision
                            }) {
                                Ok(true) => {}
                                Ok(false) => continue,
                                Err(error) => {
                                    publish_error(&worker, revision, error);
                                    continue;
                                }
                            }
                            let mut memory = cache.as_ref().map_or_else(
                                Default::default,
                                daw_engine::TransportCache::memory_usage,
                            );
                            memory.replayed_frames = 0;
                            status.performance.cache(memory);
                        } else if !renderer.plan.project.hosted_units().is_empty() {
                            let result = renderer.renderer.try_prepare_transport_cancellable(
                                transport,
                                || {
                                    let state = worker.0.lock().unwrap();
                                    state.closed || state.revision != revision
                                },
                            );
                            match result {
                                Ok(true) => {}
                                Ok(false) => continue,
                                Err(error) => {
                                    publish_error(&worker, revision, error.to_string());
                                    continue;
                                }
                            }
                        } else {
                            let cancelled = || {
                                let state = worker.0.lock().unwrap();
                                state.closed || state.revision != revision
                            };
                            let prepared = if let Some(donor) = donor_revision.take()
                                && let Some(predecessor) = cache.take()
                            {
                                let (next, prepared) =
                                    daw_engine::TransportCache::prepare_with_predecessor(
                                        &renderer.renderer,
                                        transport,
                                        controls.profile,
                                        predecessor,
                                        allow_checkpoint_reuse && donor == revision,
                                        cancelled,
                                    );
                                cache = Some(next);
                                prepared
                            } else {
                                cache
                                    .get_or_insert_with(|| {
                                        daw_engine::TransportCache::with_profile(
                                            renderer.plan.clone(),
                                            controls.profile,
                                        )
                                    })
                                    .prepare(transport, cancelled)
                            };
                            let Some(prepared) = prepared else {
                                continue;
                            };
                            let cache = cache.as_ref().expect("cache used for preparation");
                            status.performance.cache(cache.memory_usage());
                            renderer.renderer = prepared;
                        }
                        if !transport.playing {
                            renderer.prepare_stopped_monitoring();
                        }
                        // Transport snapshots deliberately omit live input reserves.
                        // Rebuild them on this worker after replacing the renderer,
                        // and admit their actual storage before publication.
                        if !request.fast {
                            renderer.prepare_live_midi();
                        }
                        let streaming = if request.fast {
                            Ok(())
                        } else {
                            renderer.prepare_streaming()
                        };
                        let prepared = streaming.and_then(|()| {
                            let bytes = renderer.memory_usage().owned_bytes();
                            renderer._memory.try_resize(bytes)
                        });
                        if let Err(error) = prepared {
                            publish_error(&worker, revision, error);
                            continue;
                        }
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
                        let cancelled = || {
                            let state = worker.0.lock().unwrap();
                            state.closed || state.revision != revision
                        };
                        let mut parallel = cache
                            .as_mut()
                            .filter(|cache| cache.can_parallel_catch_up(&renderer.renderer));
                        'catch_up: loop {
                            let target = status
                                .rendered_frames
                                .load(Ordering::Acquire)
                                .max(clock)
                                .saturating_add(lead);
                            let block = u64::from(renderer.plan.project.buffer_size_frames.max(1));
                            while clock < target {
                                if clock % block == 0 && cancelled() {
                                    break;
                                }
                                if let Some(cache) = parallel.as_deref_mut() {
                                    match cache.catch_up_tile(
                                        &mut renderer.renderer,
                                        target - clock,
                                        cancelled,
                                    ) {
                                        daw_engine::LiveCatchUp::Scalar => {}
                                        daw_engine::LiveCatchUp::Advanced(frames) => {
                                            clock += frames;
                                            continue;
                                        }
                                        daw_engine::LiveCatchUp::Cancelled => continue 'requests,
                                        daw_engine::LiveCatchUp::Restart => {
                                            // The cache has disabled its failed pool. Rebuild
                                            // from the same anchor and complete checkpoints.
                                            let mut state = worker.0.lock().unwrap();
                                            if !state.closed && state.revision == revision {
                                                state.request = Some((
                                                    revision,
                                                    Request::Prepared(renderer, anchor),
                                                    bindings,
                                                ));
                                            }
                                            continue 'requests;
                                        }
                                    }
                                }
                                renderer.next_sample();
                                if renderer.buffering() {
                                    if let Some(error) = renderer.source_error() {
                                        publish_error(&worker, revision, error);
                                        break 'catch_up;
                                    }
                                    let state = worker.0.lock().unwrap();
                                    if state.closed || state.revision != revision {
                                        break;
                                    }
                                    drop(state);
                                    std::thread::sleep(std::time::Duration::from_millis(1));
                                    continue;
                                }
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
                    if let Some(error) = renderer.source_error() {
                        publish_error(&worker, revision, error);
                        continue;
                    }
                    renderer.reset_track_peaks();
                    bindings.attach(&mut renderer);
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
                        needs_cache = cached_plan
                            .as_ref()
                            .is_some_and(|p| p.project.hosted_units().is_empty());
                    }
                    if let Some(cache) = &cache {
                        status.performance.cache(cache.memory_usage());
                    }
                }
            })
            .map_err(|e| format!("Cannot start audio preparation: {e}"))?;
        Ok(Self(shared))
    }
    #[cfg(test)]
    pub(super) fn suspended() -> Self {
        Self(Arc::new((Mutex::new(State::default()), Condvar::new())))
    }
    #[cfg(test)]
    pub(super) fn fail_for_test(&self, message: &str) {
        let revision = self.0.0.lock().unwrap().revision;
        publish_error(&self.0, revision, message.to_owned());
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
        self.submit_warm(plan, None);
    }
    fn submit_warm(&self, plan: Arc<RenderPlan>, predecessor: Option<Arc<RenderPlan>>) -> u64 {
        let (previous, revision) = {
            let mut state = self.0.0.lock().unwrap();
            state.revision += 1;
            state.error = None;
            state.cache_controls.last_activity = Some(std::time::Instant::now());
            let revision = state.revision;
            (
                state.request.replace((
                    revision,
                    Request::Warm(plan, predecessor),
                    Bindings::default(),
                )),
                revision,
            )
        };
        self.0.1.notify_one();
        drop(previous);
        revision
    }
    /// Invalidate exact-history coverage and cancel stale reconstruction after a live edit.
    pub fn update_effect_cache(
        &self,
        plan: Arc<RenderPlan>,
        predecessor: Option<Arc<RenderPlan>>,
    ) -> u64 {
        self.submit_warm(plan, predecessor)
    }
    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        let mut state = self.0.0.lock().unwrap();
        if state.wake_ui.is_none() {
            state.wake_ui = Some(Arc::new(wake));
        }
    }
    #[cfg(test)]
    pub fn submit(
        &self,
        plan: impl Into<Arc<RenderPlan>>,
        previous: Bindings,
        transport: Option<TransportState>,
    ) -> u64 {
        self.submit_transport(plan, previous, transport.map(Into::into))
    }
    pub fn submit_transport(
        &self,
        plan: impl Into<Arc<RenderPlan>>,
        previous: Bindings,
        transport: Option<TransportRequest>,
    ) -> u64 {
        self.submit_plan(plan.into(), previous, transport, None, None)
    }
    pub fn submit_live_insert(
        &self,
        plan: Arc<RenderPlan>,
        previous: Bindings,
        predecessor: Option<Arc<RenderPlan>>,
        insert: LiveInsertBase,
    ) -> u64 {
        self.submit_plan(plan, previous, None, predecessor, Some(insert))
    }
    pub fn submit_append(
        &self,
        plan: Arc<RenderPlan>,
        previous: Bindings,
        predecessor: Arc<RenderPlan>,
    ) -> u64 {
        self.submit_plan(plan, previous, None, Some(predecessor), None)
    }
    fn submit_plan(
        &self,
        plan: Arc<RenderPlan>,
        previous: Bindings,
        transport: Option<TransportRequest>,
        predecessor: Option<Arc<RenderPlan>>,
        live_insert: Option<LiveInsertBase>,
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
                    Request::Plan(
                        plan,
                        transport,
                        std::time::Instant::now(),
                        predecessor,
                        live_insert,
                    ),
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
        self.0.1.notify_one();
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
                    external: Default::default(),
                    beat_tracks: Default::default(),
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

#[cfg(test)]
#[path = "startup_tests.rs"]
mod startup_tests;
