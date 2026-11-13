use super::*;
use crate::preparation_tests::{ready, realtime};
use daw_core::{BeatEdit, BeatStep, BeatSubtrack, Edit, Project};

fn beat_plan() -> RenderPlan {
    let mut project = Project::default();
    let track = project.add_beat_track().unwrap();
    let mut row = BeatSubtrack::new("Tone".into());
    let row_id = row.id;
    row.instrument.layers[0]
        .oscillators
        .push(Default::default());
    project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    let clip = project.add_beat_pattern(track, 0).unwrap();
    let mut pattern = project.assets[0]
        .beat_pattern
        .as_ref()
        .unwrap()
        .pattern
        .clone();
    pattern.steps.push(BeatStep {
        id: Id::new_v4(),
        subtrack_id: row_id,
        start_tick: 0,
        velocity: 100,
    });
    project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    project.transport.playhead_frame = 1200;
    RenderPlan {
        project,
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}

#[test]
fn warm_beat_plan_is_reused_and_processing_changes_invalidate_it() {
    let plan = Arc::new(beat_plan());
    let mut plans = BeatPlans::default();
    let compiled = plans.resolve(plan.clone());
    assert!(
        compiled
            .project
            .tracks
            .iter()
            .all(|t| t.kind != daw_core::TrackKind::Beat)
    );
    let mut moved_playhead = (*plan).clone();
    moved_playhead.project.transport.playhead_frame += 1;
    assert!(Arc::ptr_eq(
        &compiled,
        &plans.resolve(Arc::new(moved_playhead))
    ));
    let mut edited = (*plan).clone();
    edited.project.tracks[0].clips[0].gain_db = -12.0;
    let next = plans.resolve(Arc::new(edited));
    assert!(!Arc::ptr_eq(&compiled, &next));
    assert!(compiled.beat_tracks != next.beat_tracks);
}

#[test]
fn startup_hands_complete_beat_content_to_callback_only_after_worker_preparation() {
    for warm in [false, true] {
        // First Play and stopped output creation for audition/recreation use the same path.
        for playing in [false, true] {
            let plan = beat_plan();
            let status = Arc::new(Status::default());
            let (mut commands, consumer) = RingBuffer::new(32);
            let (retire, mut retired) = RingBuffer::new(64);
            let mut callback = Callback::initial(
                &plan,
                consumer,
                retire,
                status.clone(),
                plan.project.sample_rate_hz,
            )
            .unwrap();
            assert!(callback.renderer.plan.project.tracks.is_empty());
            assert!(callback.renderer.plan.project.assets.is_empty());
            assert_eq!(
                callback.renderer.playhead,
                plan.project.transport.playhead_frame
            );
            for _ in 0..32 {
                assert_eq!(realtime(|| callback.next()), [0.0; 2]);
            }
            let worker = Worker::new(status.clone()).unwrap();
            if warm {
                worker.prewarm(Arc::new(plan.clone()));
                // Requests can coalesce. Observe one compiled stopped result before handoff.
                worker.submit(
                    plan.clone(),
                    Bindings::default(),
                    Some(TransportState {
                        frame: 0,
                        playing: false,
                        paused: false,
                        playback_start: None,
                    }),
                );
                ready(&worker);
            }
            let state = TransportState {
                frame: 1200,
                playing,
                paused: false,
                playback_start: playing.then_some(1200),
            };
            let revision = worker.submit(plan.clone(), Bindings::default(), Some(state));
            status.requested_revision.store(revision, Ordering::Release);
            let prepared = ready(&worker);
            assert!(
                prepared
                    .renderer
                    .plan
                    .project
                    .tracks
                    .iter()
                    .all(|t| t.kind != daw_core::TrackKind::Beat)
            );
            assert!(!prepared.renderer.plan.beat_tracks.is_empty());
            assert!(!prepared.bindings.layers.is_empty());
            assert_eq!(plan.project.tracks[0].kind, daw_core::TrackKind::Beat);
            commands.push(Command::Plan(prepared.renderer)).unwrap();
            realtime(|| callback.update());
            assert_eq!(callback.renderer.transport_state(), state);
            let mut reference = Renderer::new(plan);
            reference.prepare_transport(state);
            let mut peak = 0.0_f32;
            for frame in 0..1024 {
                let actual = realtime(|| callback.next());
                peak = peak.max(actual[0].abs());
                let expected = reference.next_sample();
                if frame > 300 {
                    assert_eq!(actual, expected);
                }
            }
            assert_eq!(peak > 0.001, playing);
            assert!(retired.pop().is_ok());
        }
    }
}

#[test]
fn startup_stop_discards_a_ready_play_request_before_any_audio_is_published() {
    let plan = beat_plan();
    let status = Arc::new(Status::default());
    let (mut commands, consumer) = RingBuffer::new(32);
    let (retire, mut retired) = RingBuffer::new(64);
    let mut callback = Callback::initial(
        &plan,
        consumer,
        retire,
        status.clone(),
        plan.project.sample_rate_hz,
    )
    .unwrap();
    let worker = Worker::new(status.clone()).unwrap();
    let play = TransportState {
        frame: 1200,
        playing: true,
        paused: false,
        playback_start: Some(1200),
    };
    worker.submit(plan.clone(), Bindings::default(), Some(play));
    let stale = ready(&worker);
    let stopped = TransportState {
        playing: false,
        playback_start: None,
        ..play
    };
    let revision = worker.submit(plan, Bindings::default(), Some(stopped));
    status.requested_revision.store(revision, Ordering::Release);
    commands.push(Command::Plan(stale.renderer)).unwrap();
    realtime(|| callback.update());
    assert!(retired.pop().is_ok());
    assert!(callback.renderer.plan.project.tracks.is_empty());
    commands
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    realtime(|| callback.update());
    assert_eq!(callback.renderer.transport_state(), stopped);
    for _ in 0..1024 {
        assert_eq!(realtime(|| callback.next()), [0.0; 2]);
    }
}

#[test]
fn initial_output_can_audition_while_project_compilation_is_pending() {
    let plan = beat_plan();
    let status = Arc::new(Status::default());
    let (_, consumer) = RingBuffer::new(32);
    let (retire, _retired) = RingBuffer::new(64);
    let mut callback =
        Callback::initial(&plan, consumer, retire, status, plan.project.sample_rate_hz).unwrap();
    let mut audition = callback.audition_sender.take().unwrap();
    audition.play(vec![[0.25; 2]; 2048]).unwrap();
    let mut peak = 0.0_f32;
    realtime(|| callback.update());
    for _ in 0..1024 {
        peak = peak.max(realtime(|| callback.next())[0]);
    }
    assert_eq!(peak, 0.25);
    assert!(callback.renderer.plan.project.tracks.is_empty());
    assert!(!callback.renderer.playing);
}

#[test]
fn first_preparation_preserves_effect_audition_handles_created_from_logical_content() {
    let mut plan = beat_plan();
    plan.project.tracks[0]
        .beat
        .as_mut()
        .unwrap()
        .output
        .effects
        .push(daw_core::EffectInstance::exciter());
    let bindings = Bindings::prepare(&plan, &Bindings::default());
    let (key, handle) = bindings
        .exciter_solos
        .iter()
        .next()
        .map(|(key, handle)| (*key, handle.clone()))
        .unwrap();
    handle.store(true, Ordering::Relaxed);
    let worker = Worker::new(Arc::new(Status::default())).unwrap();
    worker.submit(
        plan,
        bindings,
        Some(TransportState {
            frame: 0,
            playing: false,
            paused: false,
            playback_start: None,
        }),
    );
    let result = ready(&worker);
    let retained = &result.bindings.exciter_solos[&key];
    assert!(Arc::ptr_eq(&handle, retained));
    assert!(retained.load(Ordering::Relaxed));
}

#[test]
fn stopped_startup_publication_keeps_buffered_audition_at_a_different_device_rate() {
    let plan = beat_plan();
    let status = Arc::new(Status::default());
    let (mut commands, consumer) = RingBuffer::new(32);
    let (retire, _retired) = RingBuffer::new(64);
    let mut callback = Callback::initial(&plan, consumer, retire, status.clone(), 44_100).unwrap();
    let (_, control_consumer) = RingBuffer::new(32);
    let (control_retire, _control_retired) = RingBuffer::new(64);
    let mut control = Callback::initial(
        &plan,
        control_consumer,
        control_retire,
        Arc::new(Status::default()),
        44_100,
    )
    .unwrap();
    let samples: Vec<_> = (0..16_000)
        .map(|i| [(i as f32 * 0.031).sin() * 0.25; 2])
        .collect();
    let mut sender = callback.audition_sender.take().unwrap();
    let mut control_sender = control.audition_sender.take().unwrap();
    sender.play(samples.clone()).unwrap();
    control_sender.play(samples).unwrap();
    realtime(|| callback.update());
    realtime(|| control.update());
    for _ in 0..137 {
        assert_eq!(realtime(|| callback.next()), realtime(|| control.next()));
    }
    assert!(callback.index < callback.output_count);
    let worker = Worker::new(status.clone()).unwrap();
    let revision = worker.submit(
        plan,
        Bindings::default(),
        Some(TransportState {
            frame: 1200,
            playing: false,
            paused: false,
            playback_start: None,
        }),
    );
    status.requested_revision.store(revision, Ordering::Release);
    commands
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    realtime(|| callback.update());
    for frame in 0..2048 {
        assert_eq!(
            realtime(|| callback.next()),
            realtime(|| control.next()),
            "audition sample {frame}"
        );
    }
}

#[test]
fn incomplete_track_meter_bindings_return_an_error_instead_of_panicking() {
    let plan = beat_plan().prepare_beats();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PreparedRenderer::try_new(plan, &HashMap::new())
    }));
    assert!(
        result.is_ok(),
        "missing bindings must not panic the preparation worker"
    );
    let error = result.unwrap().err().expect("missing binding rejected");
    assert!(error.contains("meter binding"), "{error}");
}

#[test]
fn retry_reconciles_active_bindings_with_added_beat_tracks() {
    let mut plan = beat_plan();
    let active = Bindings::prepare(&plan.clone().prepare_beats(), &Bindings::default());
    let added = plan.project.add_beat_track().unwrap();
    let status = Arc::new(Status::default());
    let worker = Worker::new(status).unwrap();
    worker.submit(plan, active.clone(), None);
    let mut candidate = ready(&worker);
    let compiled = candidate.renderer.plan.clone();
    assert!(compiled.project.tracks.iter().any(|t| t.id == added));
    // AudioOutput::drain supplies the current active bindings when the callback
    // returns a replacement that missed its activation clock.
    candidate.bindings = active.clone();
    worker.reprepare(
        candidate,
        Anchor {
            transport: TransportState {
                frame: 1200,
                playing: false,
                paused: false,
                playback_start: None,
            },
            clock: 0,
        },
    );
    let retried = ready(&worker);
    assert_eq!(retried.bindings.tracks.len(), compiled.project.tracks.len());
    assert_eq!(retried.renderer.meters.len(), compiled.project.tracks.len());
    for (index, track) in compiled.project.tracks.iter().enumerate() {
        let meter = &retried.bindings.tracks[&track.id];
        assert!(Arc::ptr_eq(meter, &retried.renderer.meters[index]));
        if let Some(previous) = active.tracks.get(&track.id) {
            assert!(Arc::ptr_eq(meter, previous));
        }
    }
    // Retrying must leave the worker available for later edits.
    worker.submit(compiled, retried.bindings, None);
    let _ = ready(&worker);
}
