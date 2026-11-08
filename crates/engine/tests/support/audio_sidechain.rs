use super::*;
use daw_unit_core::{AudioSidechain, SidechainTap};

fn setup(rate: u32) -> (RenderPlan, Id, Id, Id) {
    let mut p = plan(rate, false);
    let source = p.project.tracks[0].id;
    let destination = Id::new_v4();
    p.project
        .edit(Edit::DuplicateTrack {
            track_id: source,
            new_track_id: destination,
        })
        .unwrap();
    let mut clip = p.project.tracks[0].clips[0].clone();
    clip.id = Id::new_v4();
    p.project.tracks[1].clips.push(clip);
    p.project.tracks[0].muted = true;
    let mut effect = EffectInstance::drive();
    let EffectSettings::Drive(s) = &mut effect.unit else {
        unreachable!()
    };
    s.gain = 0.0;
    s.tone = 0.5;
    let mut route = Modulation::new(ModulationParameter::Volume);
    route.amount = -12.0;
    let mut settings = AudioSidechain::new(source);
    settings.attack_ms = 0.0;
    settings.release_ms = 0.0;
    settings.lower_db = -60.0;
    settings.upper_db = -24.0;
    route.sidechain = Some(settings);
    let id = route.id;
    s.modulations.push(route);
    p.project.tracks[1].effects.push(effect);
    p.project.validate().unwrap();
    (p, source, destination, id)
}
fn route_mut(p: &mut RenderPlan, destination: Id) -> &mut Modulation {
    p.project
        .tracks
        .iter_mut()
        .find(|t| t.id == destination)
        .unwrap()
        .effects[0]
        .unit
        .modulations_mut()
        .first_mut()
        .unwrap()
}
#[test]
fn sidechain_live_export_blocks_seek_reorder_and_callback_allocation() {
    for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
        let (p, _, _, route) = setup(rate);
        let mut whole = vec![[0.0; 2]; 4096];
        Renderer::new(p.clone()).render_offline(&mut whole);
        let mut blocks = vec![[0.0; 2]; 4096];
        let mut renderer = Renderer::new(p.clone());
        for block in blocks.chunks_mut(31) {
            renderer.render_offline(block);
        }
        assert_eq!(whole, blocks);
        let reading = renderer
            .modulation_values(daw_engine::ModulationTarget::Sidechain { route })
            .unwrap();
        assert!((0.0..=1.0).contains(&reading[1]));
        assert!(reading[0].is_finite());
        let mut live = Renderer::new(p.clone());
        live.play();
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|c| c.set(true));
        for (i, sample) in whole.iter().enumerate() {
            let actual = live.next_sample();
            if i > rate as usize / 200 {
                assert!((actual[0] - sample[0]).abs() < 1e-6);
            }
        }
        TRACK_ALLOCATIONS.with(|c| c.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        let mut seek = Renderer::new(p.clone());
        seek.prepare_transport(daw_engine::TransportState {
            frame: 2048,
            ..Default::default()
        });
        let mut tail = vec![[0.0; 2]; 512];
        seek.render_offline(&mut tail);
        assert_eq!(tail, whole[2048..2560]);
        let mut reordered = p;
        reordered.project.tracks.reverse();
        let mut actual = vec![[0.0; 2]; 4096];
        Renderer::new(reordered).render_offline(&mut actual);
        assert_eq!(actual, whole);
    }
}
#[test]
fn sidechain_pre_fader_ducks_a_muted_source_and_missing_inverted_source_is_neutral() {
    let (p, _, destination, _) = setup(48_000);
    let mut baseline = p.clone();
    route_mut(&mut baseline, destination).amount = 0.0;
    let mut dry = vec![[0.0; 2]; 4096];
    Renderer::new(baseline.clone()).render_offline(&mut dry);
    let mut ducked = dry.clone();
    let mut measured = Renderer::new(p.clone());
    measured.render_offline(&mut ducked);
    let power = |v: &[[f32; 2]]| v[1000..].iter().map(|v| v[0] * v[0]).sum::<f32>();
    assert!(
        power(&ducked) < power(&dry) * 0.5,
        "ducked={} dry={} sidechain={:?} effect={:?}",
        power(&ducked),
        power(&dry),
        measured.modulation_values(daw_engine::ModulationTarget::Sidechain {
            route: route_mut(&mut p.clone(), destination).id
        }),
        measured.modulation_values(daw_engine::ModulationTarget::Effect(
            p.project.tracks[1].effects[0].id
        ))
    );
    for missing in [true, false] {
        let mut next = p.clone();
        let route = route_mut(&mut next, destination);
        let s = route.sidechain.as_mut().unwrap();
        if missing {
            s.track = Id::new_v4();
            s.invert = true;
        } else {
            s.tap = SidechainTap::PostFader;
        }
        let mut actual = dry.clone();
        Renderer::new(next).render_offline(&mut actual);
        assert_eq!(actual, dry, "missing={missing}");
    }
}
#[test]
fn sidechain_latency_groups_returns_and_master_share_the_prepared_graph() {
    for source_kind in ["track", "group", "return"] {
        let (mut p, source, destination, _) = setup(48_000);
        let tap = match source_kind {
            "group" => {
                let id = Id::new_v4();
                p.project
                    .edit(Edit::GroupTracks {
                        track_ids: vec![source],
                        group_id: id,
                        name: "Drums".into(),
                    })
                    .unwrap();
                p.project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == source)
                    .unwrap()
                    .muted = false;
                p.project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == id)
                    .unwrap()
                    .gain_db = -60.0;
                id
            }
            "return" => {
                let id = p.project.add_return_track().unwrap();
                let mut send = TrackSend::new(id);
                send.gain_db = 0.0;
                p.project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == source)
                    .unwrap()
                    .sends
                    .push(send);
                p.project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == id)
                    .unwrap()
                    .gain_db = -60.0;
                id
            }
            _ => source,
        };
        route_mut(&mut p, destination)
            .sidechain
            .as_mut()
            .unwrap()
            .track = tap;
        p.project
            .tracks
            .iter_mut()
            .find(|t| t.id == tap)
            .unwrap()
            .effects
            .push(EffectInstance::limiter());
        p.project.validate().unwrap();
        let mut renderer = Renderer::new(p.clone());
        assert!(renderer.latency_frames() > 0);
        let mut whole = vec![[0.0; 2]; 4096];
        renderer.render_offline(&mut whole);
        let mut seek = Renderer::new(p.clone());
        seek.prepare_transport(daw_engine::TransportState {
            frame: 2000,
            ..Default::default()
        });
        let mut tail = vec![[0.0; 2]; 128];
        seek.render_offline(&mut tail);
        assert_eq!(tail, whole[2000..2128]);
        let effect = p
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == destination)
            .unwrap()
            .effects
            .remove(0);
        p.project.master.effects.push(effect);
        p.project.validate().unwrap();
        let mut master = Renderer::new(p);
        master.render_offline(&mut tail);
        assert!(tail.iter().flatten().all(|v| v.is_finite()));
    }
}
#[test]
fn sidechain_compatible_setting_edits_inherit_without_callback_allocation() {
    let (p, _, destination, _) = setup(48_000);
    let mut old = Renderer::new(p.clone());
    old.play();
    for _ in 0..1000 {
        old.next_sample();
    }
    let mut p = p;
    route_mut(&mut p, destination)
        .sidechain
        .as_mut()
        .unwrap()
        .release_ms = 250.0;
    let mut next = Renderer::new(p);
    assert!(next.can_inherit_without_preroll(&old));
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut old);
    for _ in 0..512 {
        next.next_sample();
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
#[test]
fn sidechain_shared_values_drive_per_note_oscillator_gain() {
    let (mut p, source, destination, _) = setup(48_000);
    p.project.tracks.retain(|t| t.id != destination);
    let midi = p.project.add_midi_track().unwrap();
    p.project
        .tracks
        .iter_mut()
        .find(|t| t.id == midi)
        .unwrap()
        .instrument = Some(OscillatorSettings::default().into());
    let clip = p.project.add_midi_pattern(midi, 0).unwrap();
    p.project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 69,
                    velocity: 127,
                }],
            },
        })
        .unwrap();
    let i = p
        .project
        .tracks
        .iter_mut()
        .find(|t| t.id == midi)
        .unwrap()
        .instrument
        .as_mut()
        .unwrap();
    let layer = &mut i.layers[0];
    let mut route = Modulation::new(ModulationParameter::OscillatorGain);
    route.target = Some(layer.oscillators[0].id);
    route.amount = -12.0;
    let mut s = AudioSidechain::new(source);
    s.lower_db = -60.0;
    s.upper_db = -24.0;
    route.sidechain = Some(s);
    layer.oscillator_modulations.push(route);
    p.project.validate().unwrap();
    let target = daw_engine::ModulationTarget::Oscillator {
        track: midi,
        layer: layer_id(&p, midi),
        oscillator: p.project.instrument_settings(midi).unwrap().layers[0].oscillators[0].id,
    };
    let mut renderer = Renderer::new(p);
    let mut samples = vec![[0.0; 2]; 4096];
    renderer.render_offline(&mut samples);
    let read = renderer.modulation_values(target).unwrap();
    assert!(read[2] < -3.0, "{read:?}");
    assert!(samples.iter().any(|v| v[0] != 0.0));
}
fn layer_id(p: &RenderPlan, track: Id) -> Id {
    p.project.instrument_settings(track).unwrap().layers[0].id
}

#[test]
fn sidechain_taps_stay_sample_aligned_across_source_and_destination_effect_latency() {
    let (mut p, source, destination, _) = setup(48_000);
    let asset = p.project.tracks[0].clips[0].asset_id;
    p.audio.get_mut(&asset).unwrap().samples = Arc::new(
        (0..8192)
            .map(|i| {
                if (1000..1100).contains(&i) {
                    [0.2; 2]
                } else {
                    [0.01; 2]
                }
            })
            .collect(),
    );
    let mut expected = vec![[0.0; 2]; 4096];
    Renderer::new(p.clone()).render_offline(&mut expected);
    for (source_delay, destination_delay) in [(true, false), (false, true), (true, true)] {
        let mut p = p.clone();
        for (id, enabled) in [(source, source_delay), (destination, destination_delay)] {
            if enabled {
                let mut e = EffectInstance::limiter();
                let EffectSettings::Limiter(s) = &mut e.unit else {
                    unreachable!()
                };
                s.enabled = false;
                p.project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == id)
                    .unwrap()
                    .effects
                    .insert(0, e);
            }
        }
        let mut actual = expected.clone();
        Renderer::new(p).render_offline(&mut actual);
        for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert!(
                (actual[0] - expected[0]).abs() < 1e-6,
                "source={source_delay} destination={destination_delay} frame {i}: {actual:?} != {expected:?}"
            );
        }
    }
}

#[test]
fn sidechain_delayed_midi_uses_its_own_loop_boundary_without_callback_allocations() {
    let (mut p, source, destination, _) = setup(48_000);
    p.project.tracks.retain(|t| t.id != destination);
    let asset = p.project.tracks[0].clips[0].asset_id;
    p.audio.get_mut(&asset).unwrap().samples = Arc::new(vec![[0.1; 2]; 8192]);
    let midi = p.project.add_midi_track().unwrap();
    p.project
        .tracks
        .iter_mut()
        .find(|t| t.id == midi)
        .unwrap()
        .instrument = Some(OscillatorSettings::default().into());
    let clip = p.project.add_midi_pattern(midi, 0).unwrap();
    p.project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 69,
                    velocity: 127,
                }],
            },
        })
        .unwrap();
    let layer = &mut p
        .project
        .tracks
        .iter_mut()
        .find(|t| t.id == midi)
        .unwrap()
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.envelopes[0].settings = EnvelopeSettings::legacy();
    let mut route = Modulation::new(ModulationParameter::OscillatorGain);
    route.target = Some(layer.oscillators[0].id);
    route.amount = -12.0;
    let mut settings = AudioSidechain::new(source);
    settings.attack_ms = 0.0;
    settings.release_ms = 0.0;
    settings.lower_db = -60.0;
    settings.upper_db = -24.0;
    route.sidechain = Some(settings);
    layer.oscillator_modulations.push(route);
    p.project.transport.r#loop.enabled = true;
    p.project.transport.r#loop.start_frame = 0;
    p.project.transport.r#loop.end_frame = 2048;
    p.project.validate().unwrap();
    let mut reference = Renderer::new(p.clone());
    reference.play();
    let expected: Vec<_> = (0..8192).map(|_| reference.next_sample()).collect();
    let mut delay = EffectInstance::limiter();
    let EffectSettings::Limiter(s) = &mut delay.unit else {
        unreachable!()
    };
    s.enabled = false;
    p.project.tracks[0].effects.push(delay);
    let mut delayed = Renderer::new(p);
    delayed.play();
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    for (i, expected) in expected.iter().enumerate() {
        let actual = delayed.next_sample();
        if i >= 4096 {
            assert!(
                (actual[0] - expected[0]).abs() < 1e-5,
                "frame {i}: {actual:?} != {expected:?}"
            );
        }
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
