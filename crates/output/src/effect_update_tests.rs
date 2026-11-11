use super::*;
use crate::preparation_tests::realtime;
use daw_core::{EffectInstance as E, EffectSettings as S};

fn effects() -> Vec<E> {
    vec![
        E::equalizer(),
        E::drive(),
        E::drive_v2(),
        E::filter(),
        E::auto_filter(),
        E::compressor(),
        E::ott(),
        E::limiter(),
        E::direction_mixer(),
        E::stereo_spread(),
        E::saturator(),
        E::bitcrusher(),
        E::chorus(),
        E::plate_reverb(),
        E::ambient_reverb(),
        E::shimmer_reverb(),
        E::loudness(),
        E::harmonic_lift(),
        E::multiband_equalizer(),
        E::exciter(),
        E::multiband_compressor(),
    ]
}

fn disable(s: &mut S) {
    match s {
        S::Eq3(s) => s.enabled = false,
        S::Drive(s) => s.enabled = false,
        S::DriveV2(s) => s.enabled = false,
        S::Filter(s) => s.enabled = false,
        S::AutoFilter(s) => s.enabled = false,
        S::Compressor(s) => s.enabled = false,
        S::Ott(s) => s.enabled = false,
        S::Limiter(s) => s.enabled = false,
        S::DirectionMixer(s) => s.enabled = false,
        S::StereoSpread(s) => s.enabled = false,
        S::Saturator(s) => s.enabled = false,
        S::Bitcrusher(s) => s.enabled = false,
        S::Chorus(s) => s.enabled = false,
        S::PlateReverb(s) => s.enabled = false,
        S::AmbientReverb(s) => s.enabled = false,
        S::ShimmerReverb(s) => s.enabled = false,
        S::Loudness(s) => s.enabled = false,
        S::HarmonicLift(s) => s.enabled = false,
        S::MultibandEq(s) => s.enabled = false,
        S::Exciter(s) => s.enabled = false,
        S::MultibandCompressor(s) => s.enabled = false,
        S::Missing { .. } => unreachable!(),
    }
}

#[test]
fn all_effect_controls_preserve_history_without_construction_or_callback_heap_calls() {
    for effect in effects() {
        let mut plan = tests::transport_plan();
        plan.project.tracks[0].effects = vec![E::drive(), effect, E::filter()];
        let mut active = Renderer::new(plan.clone());
        active.play();
        for _ in 0..1024 {
            active.next_sample();
        }
        let mut old_reference = active.clone();
        let mut next = plan.clone();
        disable(&mut next.project.tracks[0].effects[1].unit);
        let mut update =
            daw_engine::EffectUpdate::prepare(&plan, next.clone()).expect("control update");
        assert_eq!(update.prepared_processors(), 0);
        let mut expected = Renderer::new(next.clone());
        expected.inherit(&mut old_reference);
        realtime(|| active.apply_effect_update(&mut update));
        assert_eq!(active.plan.project, next.project);
        realtime(|| {
            for _ in 0..512 {
                assert_eq!(active.next_sample(), expected.next_sample());
            }
        });
    }
}

#[test]
fn structural_updates_prepare_only_new_processors_and_retain_surviving_histories() {
    let mut plan = tests::transport_plan();
    plan.project.tracks[0].effects = vec![E::filter(), E::drive(), E::chorus()];
    let mut active = Renderer::new(plan.clone());
    active.play();
    for _ in 0..5000 {
        active.next_sample();
    }
    for step in 0..4 {
        let mut next = plan.clone();
        match step {
            0 => next.project.tracks[0].effects.swap(1, 2),
            1 => {
                next.project.tracks[0].effects.insert(1, E::bitcrusher());
            }
            2 => {
                next.project.tracks[0].effects.remove(0);
            }
            _ => {
                next.project
                    .master
                    .effects
                    .push(next.project.tracks[0].effects.remove(0));
            }
        }
        let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
        assert_eq!(update.prepared_processors(), usize::from(step == 1));
        let mut reference_old = active.clone();
        let mut expected = Renderer::new(next.clone());
        // Existing inheritance does not transfer Track <-> Master owners. For that case,
        // check the patch's retained tail and metadata instead of this legacy reference.
        if step != 3 {
            expected.inherit(&mut reference_old);
        }
        realtime(|| active.apply_effect_update(&mut update));
        realtime(|| {
            for _ in 0..512 {
                let sample = active.next_sample();
                assert!(sample.iter().all(|v| v.is_finite()));
                if step != 3 {
                    assert_eq!(sample, expected.next_sample(), "step {step}");
                }
            }
        });
        assert_eq!(active.plan.project, next.project);
        plan = next;
    }
}

#[test]
fn timing_and_arrangement_changes_use_full_reconstruction() {
    let plan = tests::transport_plan();
    let mut next = plan.clone();
    next.project.tracks[0].effects.push(E::limiter());
    assert!(daw_engine::EffectUpdate::prepare(&plan, next).is_none());
    let mut next = plan.clone();
    next.project.tracks[0].clips[0].start_frame += 100;
    assert!(daw_engine::EffectUpdate::prepare(&plan, next).is_none());
}

#[test]
fn async_rapid_edits_publish_the_final_value_without_waiting_for_exact_replay() {
    let (mut output, mut callback) = crate::preparation_tests::controlled_output(48_000);
    // The exact-replay worker in this fixture is suspended deliberately.
    output.active_plan = Some(output.plan.clone());
    output
        .status
        .processing_bytes
        .store(callback.renderer.owned_memory_bytes(), Ordering::Release);
    callback.renderer.play();
    let frame = callback.renderer.playhead;
    let mut desired = (*output.plan).clone();
    desired.project.tracks[0].effects.push(E::drive());
    for value in 0..100 {
        let S::Drive(s) = &mut desired.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        s.gain = value as f32 / 100.;
        output.sync(desired.clone()).unwrap();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while output.preparing() {
        assert!(
            std::time::Instant::now() < deadline,
            "effect publication stalled"
        );
        output.drain();
        realtime(|| callback.update());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    output.drain();
    assert_eq!(callback.renderer.plan.project, desired.project);
    assert_eq!(
        output.active_plan.as_ref().unwrap().project,
        desired.project
    );
    assert_eq!(callback.renderer.playhead, frame);
    assert!(callback.renderer.playing);
    assert!(output.published_plans.len() <= 4);
}

#[test]
fn controls_during_a_pending_seek_update_audible_dsp_without_completing_transport() {
    let (mut output, mut callback) = crate::preparation_tests::controlled_output(48_000);
    output.active_plan = Some(output.plan.clone());
    let mut desired = (*output.plan).clone();
    desired.project.tracks[0].effects.push(E::drive());
    output.sync(desired.clone()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while output.preparing() {
        assert!(std::time::Instant::now() < deadline);
        output.drain();
        realtime(|| callback.update());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    output.drain();
    callback.renderer.play();
    output.status.publish_transport(&callback.renderer);
    let audible_frame = callback.renderer.playhead;
    let applied = output.status.applied_revision.load(Ordering::Acquire);
    output.seek(12_000).unwrap();
    if let S::Drive(s) = &mut desired.project.tracks[0].effects[0].unit {
        s.gain = 0.9;
    }
    output.sync(desired.clone()).unwrap();
    // The fixture's exact preparation worker never runs. The independent control
    // worker must still reach the audible renderer while keeping Seek pending.
    while callback.renderer.plan.project.tracks[0].effects[0].unit
        != desired.project.tracks[0].effects[0].unit
    {
        assert!(std::time::Instant::now() < deadline);
        output.drain();
        realtime(|| callback.update());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(callback.renderer.playhead, audible_frame);
    assert!(callback.renderer.playing);
    assert_eq!(
        output.status.applied_revision.load(Ordering::Acquire),
        applied
    );
    assert_eq!(output.transport_intent().frame, 12_000);
    assert!(output.preparing());
}

#[test]
fn surviving_controls_apply_before_topology_without_acknowledging_uninstalled_units() {
    let mut plan = tests::transport_plan();
    plan.project.tracks[0].effects.push(E::drive());
    let bindings = preparation::Bindings::prepare(&plan, &Default::default());
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, _retired) = RingBuffer::new(8);
    let status = Arc::new(Status::default());
    let mut cb = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    cb.renderer.revision = 1;
    status.applied_revision.store(1, Ordering::Release);
    status.requested_revision.store(2, Ordering::Release);
    let mut desired = plan.clone();
    disable(&mut desired.project.tracks[0].effects[0].unit);
    desired.project.tracks[0].effects.push(E::filter());
    let old_topology = prepared(&plan, desired.clone(), 1, 2);
    let update = daw_engine::EffectUpdate::prepare_controls(&plan, &desired).unwrap();
    assert_eq!(update.prepared_processors(), 0);
    let memory = performance::RendererMemoryLease::try_new(update.heap_bytes()).unwrap();
    producer
        .push(Command::Effects(Box::new(effect_updates::Prepared {
            publication: 0,
            revision: 2,
            base_revision: 1,
            base_publication: 0,
            update,
            memory,
            modulation: Vec::new(),
            requested_at: std::time::Instant::now(),
            applied: false,
            complete: false,
        })))
        .ok()
        .unwrap();
    realtime(|| cb.update());
    assert_eq!(status.applied_revision.load(Ordering::Acquire), 1);
    assert_eq!(cb.renderer.plan.project.tracks[0].effects.len(), 1);
    assert_eq!(
        cb.renderer.plan.project.tracks[0].effects[0].unit,
        desired.project.tracks[0].effects[0].unit
    );
    producer.push(Command::Effects(old_topology)).ok().unwrap();
    realtime(|| cb.update());
    assert_eq!(cb.renderer.plan.project.tracks[0].effects.len(), 1);
    let rebased = prepared(&cb.renderer.plan, desired.clone(), cb.renderer.revision, 2);
    producer.push(Command::Effects(rebased)).ok().unwrap();
    realtime(|| cb.update());
    assert_eq!(status.applied_revision.load(Ordering::Acquire), 2);
    assert_eq!(cb.renderer.plan.project, desired.project);
}

fn note_plan() -> RenderPlan {
    let mut project = daw_core::Project::default();
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
    let clip = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(daw_core::Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: daw_core::MidiPattern {
                length_ticks: 3840,
                notes: vec![daw_core::MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 69,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    RenderPlan {
        project,
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}

#[test]
fn layer_and_output_moves_retain_voice_state_and_reject_changed_delay_distribution() {
    for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
        let mut plan = note_plan();
        plan.project.sample_rate_hz = rate;
        let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
        instrument.layers[0].effects.push(E::drive());
        let mut active = Renderer::new(plan.clone());
        let mut next = plan.clone();
        let i = next.project.tracks[0].instrument.as_mut().unwrap();
        i.main.effects.push(i.layers[0].effects.remove(0));
        let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
        assert_eq!(update.prepared_processors(), 0);
        realtime(|| active.apply_effect_update(&mut update));
        assert_eq!(active.plan.project, next.project);
        let mut bad = next.clone();
        bad.project.tracks[0].instrument.as_mut().unwrap().layers[0]
            .effects
            .push(E::limiter());
        assert!(daw_engine::EffectUpdate::prepare(&next, bad).is_none());
    }
}

#[test]
fn saved_layer_order_does_not_change_the_runtime_owner_of_a_control_update() {
    let mut plan = note_plan();
    let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
    let mut second = daw_core::InstrumentLayerSettings::duplicate_group(&[&instrument.layers[0]])
        .pop()
        .unwrap();
    instrument.layers[0].id = Id::from_u128(20);
    second.id = Id::from_u128(10);
    instrument.layers[0].pan = -1.;
    second.pan = 1.;
    instrument.layers[0].effects.push(E::drive());
    second.effects.push(E::filter());
    instrument.layers.push(second);
    let mut active = Renderer::new(plan.clone());
    active.play();
    for _ in 0..2000 {
        active.next_sample();
    }
    let mut next = plan.clone();
    let S::Drive(s) =
        &mut next.project.tracks[0].instrument.as_mut().unwrap().layers[0].effects[0].unit
    else {
        unreachable!()
    };
    s.gain = 0.9;
    let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
    assert_eq!(update.prepared_processors(), 0);
    let mut previous = active.clone();
    let mut expected = Renderer::new(next);
    expected.inherit(&mut previous);
    realtime(|| active.apply_effect_update(&mut update));
    realtime(|| {
        for _ in 0..1000 {
            assert_eq!(active.next_sample(), expected.next_sample());
        }
    });
}

#[test]
fn internal_and_external_route_controls_preserve_smoothing_without_processor_preparation() {
    for internal in [false, true] {
        let mut plan = note_plan();
        let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
        let lfo = daw_core::LfoInstance::default();
        let source = daw_core::ModulationSourceId {
            layer: instrument.main.id,
            unit: lfo.id,
        };
        instrument.main.lfos.push(lfo);
        let mut effect = if internal {
            E::multiband_equalizer()
        } else {
            E::drive()
        };
        let mut route = daw_core::Modulation::new(if internal {
            daw_core::ModulationParameter::EqBandGain
        } else {
            daw_core::ModulationParameter::DriveInput
        });
        route.source = Some(source);
        route.amount = 0.2;
        if internal {
            route.target = effect.unit.band_ids().next();
        }
        match &mut effect.unit {
            S::Drive(s) => s.modulations.push(route),
            S::MultibandEq(s) => s.modulations.push(route),
            _ => unreachable!(),
        }
        plan.project.tracks[0].effects.push(effect);
        let mut active = Renderer::new(plan.clone());
        active.play();
        for _ in 0..1000 {
            active.next_sample();
        }
        let mut next = plan.clone();
        next.project.tracks[0].effects[0].unit.modulations_mut()[0].amount = 0.8;
        let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
        assert_eq!(update.prepared_processors(), 0);
        let mut previous = active.clone();
        let mut expected = Renderer::new(next.clone());
        expected.inherit(&mut previous);
        realtime(|| active.apply_effect_update(&mut update));
        realtime(|| {
            for _ in 0..1024 {
                assert_eq!(active.next_sample(), expected.next_sample());
            }
        });
    }
}

#[test]
fn variable_band_topology_prepares_only_the_changed_unit() {
    for mut effect in [
        E::multiband_equalizer(),
        E::exciter(),
        E::multiband_compressor(),
    ] {
        let mut plan = tests::transport_plan();
        plan.project.tracks[0].effects = vec![E::drive(), effect.clone(), E::filter()];
        match &mut effect.unit {
            S::MultibandEq(s) => {
                s.bands.push(daw_core::MultibandEqBand::default());
            }
            S::Exciter(s) => {
                s.add_band();
            }
            S::MultibandCompressor(s) => {
                s.add_band();
            }
            _ => unreachable!(),
        }
        let mut next = plan.clone();
        next.project.tracks[0].effects[1] = effect;
        let mut update = daw_engine::EffectUpdate::prepare(&plan, next).unwrap();
        assert_eq!(update.prepared_processors(), 1);
        let mut active = Renderer::new(plan);
        realtime(|| active.apply_effect_update(&mut update));
    }
}

#[test]
fn beat_compilation_retains_derived_owners_for_effect_controls_and_reorders() {
    let mut project = daw_core::Project::default();
    let track = project.add_beat_track().unwrap();
    let mut row = daw_core::BeatSubtrack::new("Tone".into());
    row.instrument.layers[0]
        .oscillators
        .push(Default::default());
    row.instrument.layers[0].effects = vec![E::drive(), E::filter()];
    let row_id = row.id;
    project
        .edit(daw_core::Edit::Beat {
            track_id: track,
            edit: daw_core::BeatEdit::Add {
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
    pattern.steps.push(daw_core::BeatStep {
        id: Id::new_v4(),
        subtrack_id: row_id,
        start_tick: 0,
        velocity: 100,
    });
    project
        .edit(daw_core::Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    let plan = RenderPlan {
        project,
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    };
    let mut next = plan.clone();
    let effects = &mut next.project.tracks[0].beat.as_mut().unwrap().subtracks[0]
        .instrument
        .layers[0]
        .effects;
    effects.swap(0, 1);
    disable(&mut effects[1].unit);
    let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
    assert_eq!(update.prepared_processors(), 0);
    let mut active = Renderer::new(plan);
    active.play();
    for _ in 0..1000 {
        active.next_sample();
    }
    let mut previous = active.clone();
    let mut expected = Renderer::new(next.clone());
    expected.inherit(&mut previous);
    realtime(|| active.apply_effect_update(&mut update));
    assert_eq!(active.plan.project, next.prepare_beats().project);
    realtime(|| {
        for _ in 0..512 {
            assert_eq!(active.next_sample(), expected.next_sample());
        }
    });
}

#[test]
fn group_return_and_sidechain_stage_descriptors_gate_local_publication() {
    let mut plan = tests::transport_plan();
    let group = plan.project.add_group_track().unwrap();
    let child = plan
        .project
        .add_track_in_group(group, daw_core::TrackKind::Audio)
        .unwrap();
    let ret = plan.project.add_return_track().unwrap();
    let child_index = plan
        .project
        .tracks
        .iter()
        .position(|t| t.id == child)
        .unwrap();
    let ret_index = plan
        .project
        .tracks
        .iter()
        .position(|t| t.id == ret)
        .unwrap();
    let group_index = plan
        .project
        .tracks
        .iter()
        .position(|t| t.id == group)
        .unwrap();
    let mut send = daw_core::TrackSend::new(ret);
    send.gain_db = -6.;
    plan.project.tracks[child_index].sends.push(send);
    plan.project.tracks[group_index].effects.push(E::drive());
    plan.project.tracks[ret_index].effects.push(E::filter());
    let mut next = plan.clone();
    let moved = next.project.tracks[group_index].effects.remove(0);
    next.project.tracks[ret_index].effects.push(moved);
    let mut update = daw_engine::EffectUpdate::prepare(&plan, next.clone()).unwrap();
    assert_eq!(update.prepared_processors(), 0);
    let mut active = Renderer::new(plan);
    realtime(|| active.apply_effect_update(&mut update));
    assert_eq!(active.plan.project, next.project);

    let source = next.project.tracks[0].id;
    let mut drive = E::drive();
    let mut route = daw_core::Modulation::new(daw_core::ModulationParameter::DriveInput);
    route.sidechain = Some(daw_unit_core::AudioSidechain::new(source));
    if let S::Drive(s) = &mut drive.unit {
        s.modulations.push(route);
    }
    next.project.tracks[child_index].effects = vec![E::limiter(), drive];
    let mut changed = next.clone();
    changed.project.tracks[child_index].effects.swap(0, 1);
    assert!(matches!(
        daw_engine::EffectUpdate::prepare_with_reason(&next, changed),
        Err(daw_engine::EffectUpdateFallback::Sidechain)
    ));
}

fn prepared(
    plan: &RenderPlan,
    next: RenderPlan,
    base: u64,
    revision: u64,
) -> Box<effect_updates::Prepared> {
    let update = daw_engine::EffectUpdate::prepare(plan, next).unwrap();
    let memory = performance::RendererMemoryLease::try_new(update.heap_bytes()).unwrap();
    Box::new(effect_updates::Prepared {
        publication: 0,
        revision,
        base_revision: base,
        base_publication: 0,
        update,
        memory,
        modulation: Vec::new(),
        requested_at: std::time::Instant::now(),
        applied: false,
        complete: true,
    })
}

#[test]
fn callback_rejects_stale_or_wrong_base_transactions_and_retries_full_retirement() {
    let plan = tests::transport_plan();
    let bindings = preparation::Bindings::prepare(&plan, &Default::default());
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, _retired) = RingBuffer::new(8);
    let status = Arc::new(Status::default());
    let mut cb = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    cb.renderer.revision = 1;
    let mut desired = plan.clone();
    desired.project.tracks[0].effects.push(E::drive());
    status.requested_revision.store(3, Ordering::Release);
    for (base, revision) in [(1, 2), (9, 3)] {
        producer
            .push(Command::Effects(prepared(
                &plan,
                desired.clone(),
                base,
                revision,
            )))
            .ok()
            .unwrap();
        realtime(|| cb.update());
        assert!(!cb.effect_receiver.as_mut().unwrap().pop().unwrap().applied);
        assert_eq!(cb.renderer.revision, 1);
    }
    // Fill the bounded retire queue without touching the active renderer.
    for _ in 0..64 {
        cb.effect_retired
            .push(prepared(&plan, desired.clone(), 1, 2))
            .ok()
            .unwrap();
    }
    producer
        .push(Command::Effects(prepared(&plan, desired.clone(), 1, 3)))
        .ok()
        .unwrap();
    realtime(|| cb.update());
    assert_eq!(cb.renderer.revision, 1);
    assert!(cb.pending_effect.is_some());
    drop(cb.effect_receiver.as_mut().unwrap().pop().unwrap());
    realtime(|| cb.update());
    assert_eq!(cb.renderer.revision, 3);
    assert_eq!(cb.renderer.plan.project, desired.project);
}

#[test]
fn rejecting_a_duplicate_revision_cannot_erase_the_applied_publication_before_ack() {
    let (mut output, mut callback) = crate::preparation_tests::controlled_output(48_000);
    let previous = output.plan.clone();
    let mut desired = (*previous).clone();
    desired.project.tracks[0].effects.push(E::drive());
    let bindings = preparation::Bindings::prepare(&desired, &output.bindings());
    output.prepare_revision = 1;
    output.status.requested_revision.store(1, Ordering::Release);
    for publication in [1, 2] {
        let mut update = prepared(&previous, desired.clone(), 0, 1);
        update.publication = publication;
        output.published_plans.push_back(PublishedPlan {
            revision: 1,
            publication,
            plan: Arc::new(desired.clone()),
            bindings: bindings.clone(),
        });
        output.commands.push(Command::Effects(update)).ok().unwrap();
    }
    realtime(|| callback.update());
    let applied = output.retired_effects.pop().unwrap();
    let rejected = output.retired_effects.pop().unwrap();
    assert!(applied.applied);
    assert!(!rejected.applied);
    assert_eq!(applied.revision, rejected.revision);
    // Model the callback completing between drain's first ACK read and its stale
    // cleanup. Only the rejected owner can be forgotten, even for the same revision.
    output.forget_publication(rejected.publication);
    assert!(
        output
            .published_plans
            .iter()
            .any(|p| p.publication == applied.publication)
    );
    output.acknowledge_processing();
    assert_eq!(output.active_publication, applied.publication);
    assert_eq!(output.active_revision, 1);
    assert_eq!(
        output.active_plan.as_ref().unwrap().project,
        callback.renderer.plan.project
    );
    assert!(output.published_plans.is_empty());
}

#[test]
fn concurrent_control_and_topology_publication_keeps_the_final_acknowledged_snapshot() {
    let (mut output, mut callback) = crate::preparation_tests::controlled_output(48_000);
    output.active_plan = Some(output.plan.clone());
    let running = Arc::new(AtomicBool::new(true));
    let audio_running = running.clone();
    let audio = std::thread::spawn(move || {
        while audio_running.load(Ordering::Acquire) {
            realtime(|| callback.update());
            std::thread::yield_now();
        }
        callback
    });
    let mut desired = (*output.plan).clone();
    desired.project.tracks[0].effects.push(E::drive());
    let filter = E::filter();
    for i in 0..1000 {
        let S::Drive(s) = &mut desired.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        s.gain = (i % 100) as f32 / 100.;
        if i % 32 == 0 {
            desired.project.tracks[0].effects.truncate(1);
            if i % 64 == 0 {
                desired.project.tracks[0].effects.push(filter.clone());
            }
        }
        output.sync(desired.clone()).unwrap();
        if i % 8 == 0 {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while output.preparing() && std::time::Instant::now() < deadline {
        output.drain();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    running.store(false, Ordering::Release);
    let callback = audio.join().expect("callback heap guard failed");
    output.drain();
    assert!(
        !output.preparing(),
        "publication lost its base snapshot or final value"
    );
    assert_eq!(callback.renderer.plan.project, desired.project);
    assert_eq!(
        output.active_plan.as_ref().unwrap().project,
        desired.project
    );
    assert!(output.published_plans.is_empty());
}
