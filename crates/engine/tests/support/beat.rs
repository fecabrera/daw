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
fn beat_retrigger_during_stop_fade_preserves_continuity_and_bus_processing() {
    for rate in SAMPLE_RATES {
        for compression in [4, 8] {
            for layer_effect in [false, true] {
                let mut plan = beat_plan(rate, true, true);
                plan.project.edit(Edit::SetTempo { bpm: 300.0 }).unwrap();
                let beat = plan.project.tracks[0].beat.as_mut().unwrap();
                let (first, second) = (beat.subtracks[0].id, beat.subtracks[1].id);
                if layer_effect {
                    // Drive exercises the per-layer input path without latency.
                    beat.subtracks[0].instrument.layers[0]
                        .effects
                        .push(EffectInstance::drive());
                }
                let id = plan.project.tracks[0].clips[0].id;
                let content = BeatPattern::default()
                    .with_grid(1, BeatResolution::ThirtySecond)
                    .unwrap()
                    .with_step(first, 0, Some(127))
                    .unwrap()
                    .with_step(second, 960, Some(127))
                    .unwrap()
                    .with_step(first, 1080, Some(127))
                    .unwrap();
                plan.project
                    .edit(Edit::ReplaceBeatPattern {
                        clip_id: id,
                        pattern: content,
                    })
                    .unwrap();
                let source_frame = plan.project.midi_frame(1080);
                let clip = &mut plan.project.tracks[0].clips[0];
                clip.stretch = Some(ClipStretch::new(compression, 1).unwrap());
                clip.length_frames /= compression;
                plan.project.validate().unwrap();
                // Scheduling uses ceil when source-frame events map to stretched output.
                let retrigger = source_frame.div_ceil(compression) as usize;
                let mut data = vec![[0.0; 2]; retrigger + (rate / 100) as usize];
                Renderer::new(plan.clone()).render_offline(&mut data);
                if compression == 8 {
                    assert!(data[retrigger - 1][0].abs() > 0.01);
                } else {
                    assert!(data[retrigger - 1][0].abs() < 1e-7);
                }
                for (before, after) in data[retrigger - 1].into_iter().zip(data[retrigger]) {
                    let jump = (after - before).abs();
                    assert!(
                        jump < 0.01,
                        "rate={rate}, compression={compression}, effects={layer_effect}, jump={jump}"
                    );
                }
                let mut seek = Renderer::new(plan.clone());
                let mut slice = vec![[0.0; 2]; 64];
                seek.render_offline_at((retrigger - 4) as u64, &mut slice);
                assert_eq!(slice, data[retrigger - 4..retrigger + 60]);
                let mut live = Renderer::new(plan);
                live.play();
                ALLOCATIONS.with(|c| c.set(0));
                TRACK_ALLOCATIONS.with(|c| c.set(true));
                for (frame, expected) in data.iter().enumerate() {
                    let actual = live.next_sample();
                    if frame > (rate / 200) as usize {
                        assert!((actual[0] - expected[0]).abs() < 1e-6);
                        assert!((actual[1] - expected[1]).abs() < 1e-6);
                    }
                }
                TRACK_ALLOCATIONS.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            }
        }
    }
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

#[test]
fn beat_clip_gain_retains_timing_through_nested_latency_and_loops() {
    for rate in [44_100, 48_000, 96_000] {
        for placement in ["layer", "subtrack", "output", "sibling", "parent"] {
            let mut dry = beat_plan(rate, false, false);
            let first = &mut dry.project.tracks[0].clips[0];
            first.start_frame = 100;
            first.length_frames = 1000;
            first.gain_db = -12.0;
            let mut next = first.clone();
            next.id = Id::new_v4();
            next.start_frame = first.end();
            next.gain_db = -6.0;
            dry.project.tracks[0].clips.push(next);
            let mut wet = dry.clone();
            let mut limiter = EffectInstance::limiter();
            let EffectSettings::Limiter(settings) = &mut limiter.unit else {
                unreachable!()
            };
            settings.enabled = false;
            let beat = wet.project.tracks[0].beat.as_mut().unwrap();
            match placement {
                "layer" => beat.subtracks[0].instrument.layers[0].effects.push(limiter),
                "subtrack" => beat.subtracks[0].instrument.main.effects.push(limiter),
                "output" => beat.output.effects.push(limiter),
                "sibling" => beat.subtracks[1].instrument.main.effects.push(limiter),
                _ => wet.project.tracks[0].effects.push(limiter),
            }
            wet.project.validate().unwrap();
            let mut expected = vec![[0.0; 2]; 2400];
            let mut actual = expected.clone();
            Renderer::new(dry.clone()).render_offline(&mut expected);
            Renderer::new(wet.clone()).render_offline(&mut actual);
            assert!(expected.iter().any(|s| s[0].abs() > 0.001));
            for (frame, (a, b)) in expected.iter().zip(&actual).enumerate() {
                assert!(
                    (a[0] - b[0]).abs() < 1e-6,
                    "{rate} {placement} offline {frame}"
                );
            }
            let mut seek = [[0.0; 2]; 80];
            Renderer::new(wet.clone()).render_offline_at(1050, &mut seek);
            assert_eq!(seek.as_slice(), &actual[1050..1130]);
            for plan in [&mut dry, &mut wet] {
                plan.project.transport.r#loop.enabled = true;
                plan.project.transport.r#loop.start_frame = 100;
                plan.project.transport.r#loop.end_frame = 1700;
            }
            let mut dry = Renderer::new(dry);
            let mut wet = Renderer::new(wet);
            dry.play();
            wet.play();
            dry.ramp = 1.0;
            wet.ramp = 1.0;
            for frame in 0..4000 {
                let a = dry.next_sample();
                let b = wet.next_sample();
                assert!(
                    (a[0] - b[0]).abs() < 1e-6,
                    "{rate} {placement} loop {frame}: {a:?} != {b:?}"
                );
            }
        }
    }
}

#[test]
fn beat_output_tails_retain_clip_attenuation() {
    for gain_db in [SILENT_GAIN_DB, -24.0] {
        for latency in [false, true] {
            let mut reference_plan = beat_plan(48_000, false, false);
            reference_plan.project.tracks[0].clips[0].length_frames = 2400;
            let parent = reference_plan.project.tracks[0].id;
            reference_plan
                .project
                .add_beat_pattern(parent, 24000)
                .unwrap();
            let mut chorus = EffectInstance::chorus();
            if let EffectSettings::Chorus(c) = &mut chorus.unit {
                c.feedback = 0.8;
                c.dry_wet = 1.0;
                c.delay_ms = 30.0;
                c.depth = 0.0;
            }
            let effects = &mut reference_plan.project.tracks[0]
                .beat
                .as_mut()
                .unwrap()
                .output
                .effects;
            effects.push(chorus);
            if latency {
                let mut limiter = EffectInstance::limiter();
                if let EffectSettings::Limiter(settings) = &mut limiter.unit {
                    settings.enabled = false;
                }
                effects.push(limiter);
            }
            let mut plan = reference_plan.clone();
            plan.project.tracks[0].clips[0].gain_db = gain_db;
            plan.project.validate().unwrap();
            let gain = linear_gain(gain_db);
            let mut reference = vec![[0.0; 2]; 12000];
            Renderer::new(reference_plan.clone()).render_offline(&mut reference);
            assert!(reference[2400..].iter().any(|s| s[0].abs() > 0.01));
            let mut actual = reference.clone();
            Renderer::new(plan.clone()).render_offline(&mut actual);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, (before, after)) in reference.iter().zip(&actual).enumerate() {
                let live = live.next_sample();
                for ch in 0..2 {
                    assert!(
                        (after[ch] - before[ch] * gain).abs() < 1e-6,
                        "{frame} {gain_db} {latency}"
                    );
                    if frame >= 2400 {
                        assert!((live[ch] - after[ch]).abs() < 1e-6);
                    }
                }
            }
            let mut seek = [[0.0; 2]; 128];
            Renderer::new(plan.clone()).render_offline_at(3000, &mut seek);
            assert_eq!(seek.as_slice(), &actual[3000..3128]);
            // A later empty clip takes over the shared tail gain without changing the DSP.
            let mut following = plan.clone();
            let mut following_reference = reference_plan.clone();
            for project in [&mut following.project, &mut following_reference.project] {
                project.tracks[0].clips[1].start_frame = 6000;
            }
            following.project.tracks[0].clips[1].gain_db = -6.0;
            let mut before = vec![[0.0; 2]; 8000];
            let mut after = before.clone();
            Renderer::new(following_reference).render_offline(&mut before);
            Renderer::new(following).render_offline(&mut after);
            assert!(before[6000..6240].iter().any(|s| s[0].abs() > 1e-5));
            for frame in 5800_usize..8000 {
                let amount = (frame.saturating_sub(6000) as f32 / 240.0).min(1.0);
                let expected_gain = if frame >= 6240 {
                    linear_gain(-6.0)
                } else {
                    gain + (linear_gain(-6.0) - gain) * amount
                };
                for ch in 0..2 {
                    assert!((after[frame][ch] - before[frame][ch] * expected_gain).abs() < 1e-6);
                }
            }
            for project in [&mut plan.project, &mut reference_plan.project] {
                project.transport.r#loop.enabled = true;
                project.transport.r#loop.start_frame = 0;
                project.transport.r#loop.end_frame = 10000;
            }
            let mut reference = Renderer::new(reference_plan);
            let mut actual = Renderer::new(plan);
            reference.play();
            actual.play();
            for frame in 0..22000 {
                let a = reference.next_sample();
                let b = actual.next_sample();
                assert!(
                    (b[0] - a[0] * gain).abs() < 1e-6,
                    "loop {frame} {gain_db} {latency}"
                );
            }
        }
    }
}

#[test]
fn beat_loop_wrap_retains_last_one_shot_tail() {
    for rate in SAMPLE_RATES {
        for effect in [false, true] {
            let mut plan = beat_plan(rate, true, false);
            let owner = plan.project.tracks[0].beat.as_ref().unwrap().subtracks[0].id;
            if effect {
                plan.project.tracks[0].beat.as_mut().unwrap().subtracks[0]
                    .instrument
                    .layers[0]
                    .effects
                    .push(EffectInstance::limiter());
            }
            let clip = plan.project.tracks[0].clips[0].id;
            let pattern = BeatPattern::default()
                .with_step(owner, 3600, Some(127))
                .unwrap();
            plan.project
                .edit(Edit::ReplaceBeatPattern {
                    clip_id: clip,
                    pattern: pattern.clone(),
                })
                .unwrap();
            let end = plan.project.tracks[0].clips[0].end();
            let mut reference = plan.clone();
            reference
                .project
                .edit(Edit::ReplaceBeatPattern {
                    clip_id: clip,
                    pattern: pattern.with_grid(2, BeatResolution::Sixteenth).unwrap(),
                })
                .unwrap();
            plan.project.transport.r#loop = Loop {
                enabled: true,
                start_frame: 0,
                end_frame: end,
            };
            let mut actual = Renderer::new(plan);
            let mut expected = Renderer::new(reference);
            actual.play();
            expected.play();
            for frame in 0..end + u64::from(rate / 12) {
                ALLOCATIONS.with(|count| count.set(0));
                TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
                let a = actual.next_sample();
                TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                let b = expected.next_sample();
                if frame > end + u64::from(rate / 100) {
                    for channel in 0..2 {
                        assert!(
                            (a[channel] - b[channel]).abs() < 1e-5,
                            "rate={rate} effect={effect} frame={frame} {a:?} {b:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn beat_choke_finishes_across_transport_wrap_without_recovering_gain() {
    let rate = 48000;
    let mut plan = beat_plan(rate, true, true);
    plan.project.edit(Edit::SetTempo { bpm: 6000.0 }).unwrap();
    let beat = plan.project.tracks[0].beat.as_ref().unwrap();
    let (open, closed) = (beat.subtracks[0].id, beat.subtracks[1].id);
    let clip = plan.project.tracks[0].clips[0].id;
    let pattern = BeatPattern::default()
        .with_step(closed, 0, Some(127))
        .unwrap()
        .with_step(open, 1920, Some(127))
        .unwrap()
        .with_step(closed, 3600, Some(127))
        .unwrap();
    plan.project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern: pattern.clone(),
        })
        .unwrap();
    let end = plan.project.tracks[0].clips[0].end();
    let mut reference = plan.clone();
    reference
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern: pattern.with_grid(2, BeatResolution::Sixteenth).unwrap(),
        })
        .unwrap();
    // A stop at the loop start must not restart an already fading choke.
    plan.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 0,
        end_frame: end,
    };
    let mut actual = Renderer::new(plan);
    let mut expected = Renderer::new(reference);
    actual.seek(0);
    expected.seek(0);
    actual.play();
    expected.play();
    let mut previous = f32::MAX;
    for frame in 0..end + rate as u64 / 50 {
        let value = actual.next_sample()[0].abs();
        expected.next_sample();
        if frame >= end {
            assert!(value <= previous + 1e-6, "choke recovered gain at {frame}");
        }
        previous = value;
    }
    assert_eq!(actual.next_sample(), [0.0; 2]);
    assert_eq!(expected.next_sample(), [0.0; 2]);
}

#[test]
fn beat_noise_sources_render_repeatably_with_stereo_and_seek_parity() {
    for rate in SAMPLE_RATES {
        let mut p = beat_plan(rate, false, false);
        let layer = &mut p.project.tracks[0].beat.as_mut().unwrap().subtracks[0]
            .instrument
            .layers[0];
        layer.clear_sources();
        layer.noise = Some(NoiseSettings {
            width: 1.0,
            ..Default::default()
        });
        let p = p.prepare_beats();
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(p.clone()).render_offline(&mut expected);
        assert!(expected.iter().any(|s| s[0].abs() > 0.001));
        assert!(expected.iter().any(|s| s[0] != s[1]));
        let mut live = Renderer::new(p.clone());
        live.play();
        for (frame, sample) in expected.iter().enumerate() {
            let actual = live.next_sample();
            if frame >= (rate / 200) as usize {
                assert_eq!(*sample, actual);
            }
        }
        let mut range = vec![[0.0; 2]; 128];
        p.render(1024, &mut range);
        assert_eq!(range, expected[1024..1152]);
    }
}
