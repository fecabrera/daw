use super::*;
fn beat_plan(rate: u32, sample: bool, stop: bool) -> RenderPlan {
    let mut plan = if sample {
        sampler_plan(rate, SamplerTrigger::OneShot)
    } else {
        plan(Waveform::Sine, rate)
    };
    let instrument = plan.project.tracks[0].instrument.clone().unwrap();
    plan.project.tracks.clear();
    let track = plan.project.add_beat_track().unwrap();
    let mut open = BeatSubtrack::new("Open".into());
    open.instrument = instrument;
    if sample {
        open.trigger_pitch = 20;
    }
    let open_id = open.id;
    let mut closed = BeatSubtrack::new("Closed".into());
    if stop {
        closed.stops.push(open_id);
    }
    let closed_id = closed.id;
    plan.project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Batch(vec![
                BeatEdit::Add {
                    subtrack: Box::new(open),
                    index: 0,
                },
                BeatEdit::Add {
                    subtrack: Box::new(closed),
                    index: 1,
                },
            ]),
        })
        .unwrap();
    let clip = plan.project.add_beat_pattern(track, 0).unwrap();
    let pattern = BeatPattern::default()
        .with_step(open_id, 0, Some(127))
        .unwrap()
        .with_step(closed_id, 240, Some(127))
        .unwrap();
    plan.project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    plan.project.validate().unwrap();
    plan
}
#[test]
fn beat_stops_one_shots_with_five_ms_fade_at_every_rate_and_matches_live_seek() {
    for rate in SAMPLE_RATES {
        let plan = beat_plan(rate, true, true);
        let length = rate as usize / 5;
        let mut offline = vec![[0.0; 2]; length];
        Renderer::new(plan.clone()).render_offline(&mut offline);
        let stop = plan.project.midi_frame(240) as usize;
        let fade = (rate / 200) as usize;
        assert!(
            offline[stop - 20][0].abs() > 0.1,
            "one-shot must continue before the stop"
        );
        assert!(
            offline[stop + fade + 1..].iter().all(|v| v[0].abs() < 1e-7),
            "stop must silence the sample after 5 ms at {rate}"
        );
        let mut live = Renderer::new(plan.clone());
        live.play();
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|c| c.set(true));
        for (frame, expected) in offline.iter().enumerate() {
            let actual = live.next_sample();
            if frame > fade {
                assert!((actual[0] - expected[0]).abs() < 1e-6);
            }
        }
        TRACK_ALLOCATIONS.with(|c| c.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        let mut seek = Renderer::new(plan);
        let mut slice = vec![[0.0; 2]; 64];
        seek.render_offline_at((stop + fade) as u64, &mut slice);
        assert_eq!(slice, offline[stop + fade..stop + fade + 64]);
    }
}
#[test]
fn beat_subtrack_reordering_and_collapsing_do_not_change_rendered_audio() {
    let mut plan = beat_plan(48_000, false, false);
    let mut reference = vec![[0.0; 2]; 12000];
    Renderer::new(plan.clone()).render_offline(&mut reference);
    let track = plan.project.tracks[0].id;
    let row = plan.project.beat(track).unwrap().subtracks[0].id;
    plan.project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Reorder {
                ids: vec![row],
                index: 1,
            },
        })
        .unwrap();
    for row in &mut plan.project.tracks[0].beat.as_mut().unwrap().subtracks {
        row.instrument.main.collapsed = !row.instrument.main.collapsed;
    }
    let mut reordered = vec![[0.0; 2]; 12000];
    Renderer::new(plan).render_offline(&mut reordered);
    assert_eq!(reference, reordered);
    assert!(reference.iter().any(|v| v[0].abs() > 0.01));
}
#[test]
fn beat_voice_storage_is_bounded_by_subtracks_instead_of_hit_count() {
    let sparse = beat_plan(48_000, false, false);
    let mut dense = sparse.clone();
    let row = dense
        .project
        .beat(dense.project.tracks[0].id)
        .unwrap()
        .subtracks[0]
        .id;
    let clip = dense.project.tracks[0].clips[0].id;
    let mut pattern = BeatPattern::default()
        .with_grid(16, BeatResolution::ThirtySecond)
        .unwrap();
    for tick in (0..pattern.length_ticks()).step_by(120) {
        pattern = pattern.with_step(row, tick, Some(100)).unwrap();
    }
    let closed = dense
        .project
        .beat(dense.project.tracks[0].id)
        .unwrap()
        .subtracks[1]
        .id;
    pattern = pattern.with_step(closed, 240, Some(127)).unwrap();
    dense
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    assert_eq!(
        Renderer::new(sparse).memory_usage().processor_bytes,
        Renderer::new(dense).memory_usage().processor_bytes
    );
}
#[test]
fn beat_container_preparation_is_idempotent_and_stop_links_invalidate_cache() {
    let a = beat_plan(48_000, true, false).prepare_beats();
    assert_eq!(a.project, a.clone().prepare_beats().project);
    let b = beat_plan(48_000, true, true).prepare_beats();
    assert!(!a.same_processing_plan(&b));
}

#[test]
fn beat_output_controls_apply_once_and_its_local_macro_drives_shared_effects() {
    let mut plan = beat_plan(48_000, false, false);
    let track = plan.project.tracks[0].id;
    let row = plan.project.beat(track).unwrap().subtracks[0].id;
    let mut reference = vec![[0.0; 2]; 5000];
    Renderer::new(plan.clone()).render_offline(&mut reference);
    plan.project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::SetMix {
                id: row,
                gain_db: -6.0,
                pan: 0.0,
                muted: false,
                soloed: false,
            },
        })
        .unwrap();
    let mut output = plan.project.beat(track).unwrap().output.clone();
    output.gain_db = -6.0;
    plan.project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::SetOutput(Box::new(output)),
        })
        .unwrap();
    plan.project.tracks[0].gain_db = -6.0;
    let mut attenuated = reference.clone();
    Renderer::new(plan.clone()).render_offline(&mut attenuated);
    let gain = linear_gain(-18.0);
    for (before, after) in reference.iter().zip(&attenuated).skip(240) {
        assert!((after[0] - before[0] * gain).abs() < 1e-6);
    }
    let output = &mut plan.project.tracks[0].beat.as_mut().unwrap().output;
    output.macro_module.add();
    output.macro_module.controls[0].settings.value = 0.5;
    let source = ModulationSourceId {
        layer: output.id,
        unit: output.macro_module.controls[0].id,
    };
    let root = output.id;
    let mut effect = EffectInstance::drive();
    let effect_id = effect.id;
    if let EffectSettings::Drive(settings) = &mut effect.unit {
        let mut route = Modulation::new(ModulationParameter::DriveGain);
        route.source = Some(source);
        route.amount = 0.25;
        settings.modulations.push(route);
    }
    output.effects.push(effect);
    plan.project.validate().unwrap();
    let mut renderer = Renderer::new(plan);
    renderer.render_offline(&mut attenuated);
    let value = renderer
        .modulation_values(daw_engine::ModulationTarget::Macro {
            track: root,
            layer: root,
            unit: source.unit,
        })
        .unwrap();
    assert!((value[0] - 0.5).abs() < 1e-6);
    assert!(
        renderer
            .modulation_values(daw_engine::ModulationTarget::Effect(effect_id))
            .unwrap()
            .iter()
            .all(|v| v.is_finite())
    );
    assert_ne!(attenuated, reference);
}

#[test]
fn empty_beat_instruments_preserve_the_arranged_transport_duration() {
    let mut project = Project::default();
    let track = project.add_beat_track().unwrap();
    project.add_beat_pattern(track, 0).unwrap();
    let plan = RenderPlan {
        project,
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    };
    let mut renderer = Renderer::new(plan);
    renderer.play();
    for _ in 0..8000 {
        assert_eq!(renderer.next_sample(), [0.0; 2]);
    }
    assert!(renderer.playing);
    assert_eq!(renderer.playhead, 8000);
}

#[test]
fn beat_clip_gain_follows_the_final_output_effects() {
    let mut plan = beat_plan(48_000, false, false);
    plan.project.tracks[0]
        .beat
        .as_mut()
        .unwrap()
        .output
        .effects
        .push(EffectInstance::drive());
    let mut reference = vec![[0.0; 2]; 5000];
    Renderer::new(plan.clone()).render_offline(&mut reference);
    plan.project.tracks[0].clips[0].gain_db = -12.0;
    let mut actual = reference.clone();
    Renderer::new(plan).render_offline(&mut actual);
    let gain = linear_gain(-12.0);
    assert!(reference.iter().any(|s| s[0].abs() > 0.01));
    for (before, after) in reference.iter().zip(actual).skip(240) {
        assert!((after[0] - before[0] * gain).abs() < 1e-6);
        assert!((after[1] - before[1] * gain).abs() < 1e-6);
    }
}
