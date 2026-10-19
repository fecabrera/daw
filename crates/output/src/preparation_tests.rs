use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};
thread_local! {
    static COUNT_HEAP: Cell<bool> = const { Cell::new(false) };
    static HEAP_BYTES: Cell<usize> = const { Cell::new(0) };
    static HEAP_CALLS: Cell<usize> = const { Cell::new(0) };
}
struct CheckedAllocator;
fn heap_call() {
    COUNT_HEAP.with(|enabled| {
        if enabled.get() {
            HEAP_CALLS.with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: Layouts and pointers are forwarded unchanged to the system allocator.
unsafe impl GlobalAlloc for CheckedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        heap_call();
        COUNT_HEAP.with(|enabled| {
            if enabled.get() {
                HEAP_BYTES.with(|n| n.set(n.get() + layout.size()));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        heap_call();
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        heap_call();
        COUNT_HEAP.with(|enabled| {
            if enabled.get() {
                HEAP_BYTES.with(|n| n.set(n.get() + size));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CheckedAllocator = CheckedAllocator;
fn realtime<T>(operation: impl FnOnce() -> T) -> T {
    HEAP_CALLS.with(|n| n.set(0));
    COUNT_HEAP.with(|v| v.set(true));
    let result = operation();
    COUNT_HEAP.with(|v| v.set(false));
    assert_eq!(
        HEAP_CALLS.with(Cell::get),
        0,
        "callback allocated or retired storage"
    );
    result
}
fn ready(worker: &preparation::Worker) -> preparation::Ready {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(ready) = worker.take() {
            return ready;
        }
        assert!(Instant::now() < deadline, "preparation did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn worker_transport_activates_only_latest_revision_and_preserves_pause_resume_start() {
    let mut plan = tests::transport_plan();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::limiter());
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let worker = preparation::Worker::new(Arc::new(Status::default())).unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(8);
    let status = Arc::new(Status::default());
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    let mut state = TransportState {
        frame: 8000,
        playing: true,
        paused: false,
        playback_start: Some(4800),
    };
    let stale = worker.submit(plan.clone(), bindings.clone(), Some(state));
    let stale_ready = ready(&worker);
    state.frame = 12_000;
    let revision = worker.submit(plan.clone(), bindings.clone(), Some(state));
    status.requested_revision.store(revision, Ordering::Release);
    producer.push(Command::Plan(stale_ready.renderer)).unwrap();
    realtime(|| callback.update());
    assert_eq!(retired.pop().unwrap().revision, stale);
    assert!(!callback.renderer.playing);
    let prepared = ready(&worker);
    assert!(prepared.renderer.prepared_transport);
    assert_eq!(prepared.renderer.transport_state(), state);
    producer.push(Command::Plan(prepared.renderer)).unwrap();
    realtime(|| callback.update());
    assert_eq!(callback.renderer.transport_state(), state);
    assert_eq!(status.applied_revision.load(Ordering::Acquire), revision);
    for _ in 0..1000 {
        realtime(|| callback.next());
    }
    let at = callback.renderer.playhead;
    status
        .requested_revision
        .store(revision + 1, Ordering::Release);
    producer.push(Command::PauseAt(revision + 1)).unwrap();
    realtime(|| callback.update());
    for _ in 0..300 {
        realtime(|| callback.next());
    }
    assert_eq!(callback.renderer.playhead, at);
    assert!(callback.renderer.transport_state().paused);
    status
        .requested_revision
        .store(revision + 2, Ordering::Release);
    producer.push(Command::ResumeAt(revision + 2)).unwrap();
    realtime(|| callback.update());
    assert_eq!(
        callback.renderer.transport_state().playback_start,
        Some(4800)
    );
    realtime(|| callback.next());
    assert_eq!(callback.renderer.playhead, at + 1);
}
#[test]
fn latency_changes_keep_playing_and_activate_at_the_prepared_sample_clock() {
    for rate in [44_100, 48_000, 96_000] {
        let mut plan = tests::transport_plan();
        let bindings = preparation::Bindings {
            tracks: track_meters(&plan, &HashMap::new()),
            ..Default::default()
        };
        let status = Arc::new(Status::default());
        let worker = preparation::Worker::new(status.clone()).unwrap();
        let (mut producer, consumer) = RingBuffer::new(4);
        let (retire, mut retired) = RingBuffer::new(8);
        let mut callback = Callback::new(
            plan.clone(),
            &bindings.tracks,
            consumer,
            retire,
            status.clone(),
            rate,
        )
        .unwrap();
        callback.renderer.play();
        for _ in 0..512 {
            realtime(|| callback.next());
        }
        let captured = callback.preparation_anchor();
        plan.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::limiter());
        let revision = worker.submit(plan.clone(), bindings.clone(), None);
        status.requested_revision.store(revision, Ordering::Release);
        producer
            .push(Command::Plan(ready(&worker).renderer))
            .unwrap();
        realtime(|| callback.update());
        let mut returned = retired.pop().unwrap();
        assert_eq!(returned.reprepare, Some(captured));
        assert!(callback.renderer.playing);
        let transport = returned.reprepare.take().unwrap();
        worker.reprepare(
            preparation::Ready {
                revision,
                renderer: returned,
                bindings: bindings.clone(),
            },
            transport,
        );
        // Audio and position continue while the replacement is being prepared.
        for _ in 0..2048 {
            assert!(callback.next()[0] > 0.1);
        }
        assert!(callback.renderer.playhead > captured.transport.frame);
        status
            .rendered_frames
            .store(callback.rendered_frames, Ordering::Release);
        let prepared = ready(&worker);
        let at = prepared.renderer.activate_at.unwrap();
        let expected = captured.transport.frame + at - captured.clock;
        assert_eq!(prepared.renderer.playhead, expected);
        producer.push(Command::Plan(prepared.renderer)).unwrap();
        realtime(|| callback.update());
        while callback.renderer.revision != revision {
            realtime(|| callback.next());
            assert!(callback.renderer.playing);
            assert!(callback.rendered_frames <= at + 4096);
        }
        assert_eq!(
            callback.renderer.playhead,
            captured.transport.frame + callback.rendered_frames - captured.clock
        );
        assert!(callback.renderer.latency_frames() > 0);
        assert_eq!(status.applied_revision.load(Ordering::Acquire), revision);
        assert!(retired.pop().is_ok());
    }
}

#[test]
fn pause_pending_stops_active_audio_before_preparation_without_acknowledging_it() {
    let plan = tests::transport_plan();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let worker = preparation::Worker::new(Arc::new(Status::default())).unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(8);
    let status = Arc::new(Status::default());
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.play();
    for _ in 0..512 {
        realtime(|| callback.next());
    }
    let at = callback.renderer.playhead;
    let paused = TransportState {
        frame: 8_000,
        playing: false,
        paused: true,
        playback_start: Some(0),
    };
    let revision = worker.submit(plan, bindings, Some(paused));
    status.requested_revision.store(revision, Ordering::Release);
    producer.push(Command::PausePending(revision)).unwrap();
    realtime(|| callback.update());
    assert!(!callback.renderer.playing);
    assert!(callback.renderer.transport_state().paused);
    assert_ne!(status.applied_revision.load(Ordering::Acquire), revision);
    for _ in 0..512 {
        realtime(|| callback.next());
    }
    assert_eq!(callback.renderer.playhead, at);
    producer
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    realtime(|| callback.update());
    assert_eq!(callback.renderer.transport_state(), paused);
    assert_eq!(status.applied_revision.load(Ordering::Acquire), revision);
    retired.pop().unwrap();
    status
        .requested_revision
        .store(revision + 1, Ordering::Release);
    producer.push(Command::ResumeAt(revision + 1)).unwrap();
    producer.push(Command::PausePending(revision)).unwrap();
    realtime(|| callback.update());
    assert!(
        callback.renderer.playing,
        "A stale pending pause must not stop resumed audio"
    );
    assert_eq!(callback.renderer.transport_state().playback_start, Some(0));
}

fn modulated_note_plan() -> RenderPlan {
    use daw_core::*;
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument = Some(OscillatorSettings::default().into());
    let instrument = project.tracks[0].instrument.as_mut().unwrap();
    let lfo = LfoInstance::default();
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: lfo.id,
    };
    instrument.main.lfos.push(lfo);
    let env = &mut instrument.layers[0].envelopes[0];
    env.settings.attack.time_ms = 40.0;
    let mut route = Modulation::new(ModulationParameter::EnvelopeAttack);
    route.source = Some(source);
    route.amount = 2.0;
    env.modulations.push(route);
    let clip = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 48,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 0,
        end_frame: 2400,
    };
    RenderPlan {
        project,
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}

#[test]
fn note_edits_keep_playing_and_preserve_modulation_through_loop_catch_up() {
    let mut plan = modulated_note_plan();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let status = Arc::new(Status::default());
    let worker = preparation::Worker::new(status.clone()).unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(8);
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.play();
    for _ in 0..1800 {
        realtime(|| callback.next());
    }
    let anchor = callback.preparation_anchor();
    let clip = plan.project.tracks[0].clips[0].id;
    let mut pattern = plan
        .project
        .assets
        .iter()
        .find(|a| a.id == plan.project.tracks[0].clips[0].asset_id)
        .and_then(|a| a.midi_pattern.clone())
        .unwrap();
    pattern.notes.push(daw_core::MidiNote {
        id: Id::new_v4(),
        start_tick: 48,
        length_ticks: 960,
        pitch: 60,
        velocity: 100,
    });
    plan.project
        .edit(daw_core::Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    let revision = worker.submit(plan.clone(), bindings.clone(), None);
    status.requested_revision.store(revision, Ordering::Release);
    producer
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    realtime(|| callback.update());
    assert!(callback.renderer.playing);
    let mut returned = retired.pop().unwrap();
    let captured = returned.reprepare.take().unwrap();
    assert_eq!(anchor, captured);
    worker.reprepare(
        preparation::Ready {
            revision,
            renderer: returned,
            bindings,
        },
        captured,
    );
    for _ in 0..700 {
        realtime(|| callback.next());
    }
    assert!(callback.renderer.playing);
    assert!(
        callback.renderer.playhead < anchor.transport.frame,
        "loop wrapped during preparation"
    );
    status
        .rendered_frames
        .store(callback.rendered_frames, Ordering::Release);
    let prepared = ready(&worker);
    let at = prepared.renderer.activate_at.unwrap();
    let mut reference = Renderer::new(plan);
    reference.prepare_transport(anchor.transport);
    for _ in anchor.clock..at {
        reference.next_sample();
    }
    assert_eq!(
        prepared.renderer.transport_state(),
        reference.transport_state()
    );
    producer.push(Command::Plan(prepared.renderer)).unwrap();
    realtime(|| callback.update());
    while callback.rendered_frames < at {
        realtime(|| callback.next());
    }
    realtime(|| callback.update());
    assert_eq!(callback.renderer.revision, revision);
    // The 5 ms activation fade is intentionally different; subsequent DSP is exact.
    for _ in 0..240 {
        realtime(|| callback.next());
        reference.next_sample();
    }
    let mut audible = false;
    for _ in 0..6000 {
        let actual = realtime(|| callback.next());
        let expected = reference.next_sample();
        assert_eq!(actual, expected);
        audible |= actual[0].abs() > 0.01;
        assert!(callback.renderer.playing);
    }
    assert!(audible);
}

#[test]
fn late_or_blocked_replacements_retry_without_pausing_or_rewinding() {
    let plan = tests::transport_plan();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let status = Arc::new(Status::default());
    let worker = preparation::Worker::new(status.clone()).unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(1);
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.play();
    let anchor = callback.preparation_anchor();
    let revision = worker.submit(plan.clone(), bindings.clone(), None);
    status.requested_revision.store(revision, Ordering::Release);
    worker.reprepare(ready(&worker), anchor);
    let prepared = ready(&worker);
    let at = prepared.renderer.activate_at.unwrap();
    callback
        .retired
        .push(Box::new(PreparedRenderer::new(plan, &bindings.tracks)))
        .unwrap();
    producer.push(Command::Plan(prepared.renderer)).unwrap();
    realtime(|| callback.update());
    assert!(callback.pending.is_some());
    for _ in 0..at + 100 {
        realtime(|| callback.next());
    }
    retired.pop().unwrap();
    let before = callback.renderer.transport_state();
    realtime(|| callback.update());
    assert_eq!(callback.renderer.transport_state(), before);
    assert!(before.playing);
    let mut late = retired.pop().unwrap();
    assert_eq!(late.reprepare.unwrap().transport, before);
    assert_eq!(late.preparation_lead, 9600);
    let anchor = late.reprepare.take().unwrap();
    worker.reprepare(
        preparation::Ready {
            revision,
            renderer: late,
            bindings,
        },
        anchor,
    );
    status
        .rendered_frames
        .store(callback.rendered_frames, Ordering::Release);
    producer
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    realtime(|| callback.update());
    assert!(callback.scheduled.is_some());
    // A newer pause cancels a scheduled edit, even before its activation time.
    status
        .requested_revision
        .store(revision + 1, Ordering::Release);
    producer.push(Command::PausePending(revision + 1)).unwrap();
    realtime(|| callback.update());
    assert!(callback.scheduled.is_none());
    for _ in 0..12000 {
        realtime(|| callback.next());
    }
    assert!(!callback.renderer.playing);
    assert_eq!(callback.renderer.playhead, before.frame);
    assert_ne!(status.applied_revision.load(Ordering::Acquire), revision);
}

#[test]
fn pending_seek_display_follows_audible_position_until_activation() {
    let playing = TransportState {
        frame: 8000,
        playing: true,
        paused: false,
        playback_start: Some(0),
    };
    let seek = TransportState {
        frame: 20000,
        ..playing
    };
    assert_eq!(displayed_frame(seek, playing), 8000);
    assert_eq!(
        displayed_frame(
            seek,
            TransportState {
                frame: 9000,
                ..playing
            }
        ),
        9000
    );
    assert_eq!(displayed_frame(seek, seek), 20000);
    let paused = TransportState {
        playing: false,
        paused: true,
        ..seek
    };
    assert_eq!(displayed_frame(paused, playing), 20000);
    assert_eq!(displayed_frame(seek, TransportState::default()), 20000);
}

#[test]
fn forward_and_backward_seeks_keep_old_audio_until_prepared_activation() {
    let plan = tests::transport_plan();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let status = Arc::new(Status::default());
    let worker = preparation::Worker::new(status.clone()).unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(8);
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.play();
    for _ in 0..512 {
        callback.next();
    }
    for frame in [20_000, 4_000] {
        let before = callback.renderer.transport_state();
        let requested = TransportState { frame, ..before };
        let revision = worker.submit(plan.clone(), bindings.clone(), Some(requested));
        status.requested_revision.store(revision, Ordering::Release);
        for _ in 0..512 {
            assert!(realtime(|| callback.next())[0] > 0.1);
        }
        assert_eq!(callback.renderer.playhead, before.frame + 512);
        producer
            .push(Command::Plan(ready(&worker).renderer))
            .unwrap();
        realtime(|| callback.update());
        assert_eq!(callback.renderer.transport_state(), requested);
        for _ in 0..512 {
            assert!(realtime(|| callback.next())[0] > 0.1);
        }
        retired.pop().unwrap();
    }
}

#[test]
fn cached_transport_matches_full_history_with_modulation_effects_and_backward_seeks() {
    let mut plan = modulated_note_plan();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::saturator());
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::limiter());
    let mut cache = daw_engine::TransportCache::new(plan.clone());
    let mut state = TransportState {
        frame: 24_000,
        playing: true,
        paused: false,
        playback_start: Some(1800),
    };
    cache.prepare(state, || false).unwrap();
    for frame in [32_123, 14_123, 28_123, 37_123] {
        state.frame = frame;
        let mut cached = cache.prepare(state, || false).unwrap();
        let mut reference = Renderer::new(plan.clone());
        reference.prepare_transport(state);
        assert_eq!(cached.transport_state(), reference.transport_state());
        for _ in 0..4096 {
            assert_eq!(cached.next_sample(), reference.next_sample());
        }
    }
    // Playback/paused reconstruction share live loop rules; stopped seeks do not.
    for (playing, paused) in [(false, true), (false, false), (true, false)] {
        state.playing = playing;
        state.paused = paused;
        let mut cached = cache.prepare(state, || false).unwrap();
        let mut reference = Renderer::new(plan.clone());
        reference.prepare_transport(state);
        for _ in 0..512 {
            assert_eq!(cached.next_sample(), reference.next_sample());
        }
    }
    let mut checks = 0;
    assert!(
        cache
            .prepare(
                TransportState {
                    frame: 96_000,
                    ..state
                },
                || {
                    checks += 1;
                    checks > 2
                }
            )
            .is_none()
    );
    let mut cached = cache.prepare(state, || false).unwrap();
    let mut reference = Renderer::new(plan);
    reference.prepare_transport(state);
    for _ in 0..512 {
        assert_eq!(cached.next_sample(), reference.next_sample());
    }
}

#[test]
fn changed_plans_invalidate_cached_modulation_history() {
    let plan = Arc::new(modulated_note_plan());
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let status = Arc::new(Status::default());
    let worker = preparation::Worker::new(status).unwrap();
    let state = TransportState {
        frame: 12_123,
        playing: true,
        paused: false,
        playback_start: Some(0),
    };
    worker.submit(plan.clone(), bindings.clone(), Some(state));
    let mut original = ready(&worker).renderer;
    let mut edited = (*plan).clone();
    let env = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0];
    env.modulations[0].amount = -2.0;
    env.settings.attack.time_ms = 400.0;
    worker.submit(Arc::new(edited.clone()), bindings, Some(state));
    let mut changed = ready(&worker).renderer;
    let mut reference = Renderer::new(edited);
    reference.prepare_transport(state);
    let mut different = false;
    for _ in 0..2048 {
        let actual = changed.next_sample();
        assert_eq!(actual, reference.next_sample());
        different |= actual != original.next_sample();
    }
    assert!(
        different,
        "the edit must not restore the previous plan's envelope history"
    );
}

#[test]
fn renderer_memory_estimate_covers_clone_allocations_for_all_effects_and_modulated_voices() {
    let mut plan = modulated_note_plan();
    plan.project.tracks[0].effects = vec![
        daw_core::EffectInstance::equalizer(),
        daw_core::EffectInstance::drive(),
        daw_core::EffectInstance::auto_filter(),
        daw_core::EffectInstance::compressor(),
        daw_core::EffectInstance::ott(),
        daw_core::EffectInstance::limiter(),
        daw_core::EffectInstance::direction_mixer(),
        daw_core::EffectInstance::stereo_spread(),
        daw_core::EffectInstance::saturator(),
        daw_core::EffectInstance::bitcrusher(),
    ];
    let renderer = Renderer::new(plan);
    let memory = renderer.memory_usage();
    HEAP_BYTES.with(|n| n.set(0));
    COUNT_HEAP.with(|n| n.set(true));
    let cloned = renderer.clone();
    COUNT_HEAP.with(|n| n.set(false));
    let actual = HEAP_BYTES.with(Cell::get) as u64;
    assert!(
        memory.owned_bytes() >= actual,
        "estimated {} < allocated {actual}",
        memory.owned_bytes()
    );
    assert!(memory.schedule_bytes > 0 && memory.processor_bytes > 0);
    assert_eq!(
        cloned.memory_usage().shared_audio_bytes,
        memory.shared_audio_bytes
    );
}

#[test]
fn idle_preparation_builds_exact_checkpoints_before_first_seek_and_reuses_them_after_handoff() {
    let mut plan = modulated_note_plan();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::saturator());
    let preparation = AudioPreparation::new(plan.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while preparation.performance().cache_memory[6] < plan.project.end() {
        assert!(Instant::now() < deadline, "idle preparation did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(preparation.performance().cache_memory[5] > 0);
    let status = preparation.status.clone();
    let worker = preparation.worker;
    // A new Arc with only a changed requested playhead is still the same DSP plan.
    plan.project.transport.playhead_frame = 32_123;
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let transport = TransportState {
        frame: 32_123,
        playing: true,
        paused: false,
        playback_start: Some(0),
    };
    worker.submit(Arc::new(plan.clone()), bindings, Some(transport));
    let mut prepared = ready(&worker).renderer;
    assert!(status.performance.snapshot().cache_memory[7] < 12_000);
    let mut reference = Renderer::new(plan);
    reference.prepare_transport(transport);
    for _ in 0..1024 {
        assert_eq!(realtime(|| prepared.next_sample()), reference.next_sample());
    }
}

#[test]
fn every_cache_profile_preserves_exact_history_and_required_requests_bypass_optional_gates() {
    use daw_engine::CacheProfile;
    let mut plan = modulated_note_plan();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::saturator());
    for profile in CacheProfile::ALL {
        let preparation = AudioPreparation::with_profile(plan.clone(), profile).unwrap();
        // Neither an active gesture nor Light's playback gate may delay a seek.
        preparation.configure_cache(profile, true);
        preparation.status.playing.store(true, Ordering::Relaxed);
        let worker = preparation.worker;
        let transport = TransportState {
            frame: 32_123,
            playing: true,
            paused: false,
            playback_start: Some(0),
        };
        let bindings = preparation::Bindings {
            tracks: track_meters(&plan, &HashMap::new()),
            ..Default::default()
        };
        let revision = worker.submit(Arc::new(plan.clone()), bindings, Some(transport));
        // A profile change must not cancel or replace the required request.
        worker.configure_cache(CacheProfile::Responsive, true);
        let next = ready(&worker);
        assert_eq!(next.revision, revision);
        let mut prepared = next.renderer;
        let mut reference = Renderer::new(plan.clone());
        reference.prepare_transport(transport);
        for _ in 0..1024 {
            assert_eq!(
                realtime(|| prepared.next_sample()),
                reference.next_sample(),
                "{profile:?}"
            );
        }
        let mut cache = daw_engine::TransportCache::with_profile(plan.clone(), profile);
        cache.warm_step(Duration::from_millis(10), || false);
        let mut cached = cache.prepare(transport, || false).unwrap();
        let mut full = Renderer::new(plan.clone());
        full.prepare_transport(transport);
        for _ in 0..1024 {
            assert_eq!(realtime(|| cached.next_sample()), full.next_sample());
        }
    }
}

#[test]
fn optional_cache_construction_waits_for_editing_and_resumes_after_release() {
    let plan = modulated_note_plan();
    let preparation =
        AudioPreparation::with_profile(plan, daw_engine::CacheProfile::Responsive).unwrap();
    preparation.configure_cache(daw_engine::CacheProfile::Responsive, true);
    // This exceeds Responsive's quiet interval; holding the gesture still blocks
    // construction. The scheduler tests cover exact boundaries without sleeping.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(preparation.performance().cache_memory[0], 0);
    preparation.configure_cache(daw_engine::CacheProfile::Responsive, false);
    let deadline = Instant::now() + Duration::from_secs(30);
    while preparation.performance().cache_memory[6] == 0 {
        assert!(
            Instant::now() < deadline,
            "optional preparation did not resume"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(preparation.performance().cache_memory[0] > 0);
}

#[path = "../../engine/tests/support/cache_project.rs"]
mod parallel_fixture;
#[test]
fn parallel_prepared_renderer_keeps_callback_processing_and_activation_allocation_free() {
    let plan = parallel_fixture::midi_project(48_000, 4, 1);
    let worker = preparation::Worker::new(Arc::new(Status::default())).unwrap();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let state = TransportState {
        frame: 12000,
        playing: true,
        playback_start: Some(0),
        ..Default::default()
    };
    worker.submit(Arc::new(plan.clone()), bindings, Some(state));
    let mut prepared = ready(&worker).renderer;
    let mut reference = Renderer::new(plan.clone());
    reference.prepare_transport(state);
    for _ in 0..1024 {
        assert_eq!(realtime(|| prepared.next_sample()), reference.next_sample());
    }
    let previous = Renderer::new(plan);
    realtime(|| prepared.activate_transport(&previous));
}
