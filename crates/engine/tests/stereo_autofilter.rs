#[path = "support/cache_project.rs"]
mod fixture;
use daw_core::*;
use daw_engine::Renderer;
#[test]
fn stereo_auto_filter_pan_should_match_a_bypassed_stereo_identity() {
    let mut p = fixture::midi_project(48000, 1, 0);
    p.project.tracks[0].pan = 0.0;
    let mut e = EffectInstance::auto_filter();
    let EffectSettings::AutoFilter(s) = &mut e.unit else {
        unreachable!()
    };
    s.stereo_phase_degrees = 180.0;
    s.strength_octaves = 4.0;
    p.project.tracks[0].effects.push(e);
    let mut a = Renderer::new(p.clone());
    let mut identity = EffectInstance::direction_mixer();
    let EffectSettings::DirectionMixer(s) = &mut identity.unit else {
        unreachable!()
    };
    s.enabled = false;
    p.project.tracks[0].effects.push(identity);
    let mut b = Renderer::new(p);
    let mut left = vec![[0.0; 2]; 16000];
    let mut right = left.clone();
    a.render_offline(&mut left);
    b.render_offline(&mut right);
    let stereo = left.iter().any(|s| (s[0] - s[1]).abs() > 1e-3);
    assert!(stereo);
    let ratio = left
        .iter()
        .zip(&right)
        .skip(1000)
        .find(|(x, _)| x[0].abs() > 0.01)
        .map(|(x, y)| y[0] / x[0])
        .unwrap();
    assert!(
        (ratio - 1.0).abs() < 1e-5,
        "bypassed effect should not change sound; ratio {ratio}"
    );
}
#[test]
fn stereo_autofilter_pan_is_stable_in_regular_and_beat_processing_stages() {
    for beat in [false, true] {
        for stage in ["layer", "output", "track"] {
            if beat && stage == "track" {
                continue;
            }
            for (on, modulated, pan) in [true, false].into_iter().flat_map(|on| {
                [false, true].into_iter().flat_map(move |modulated| {
                    [-0.5, 0.0, 0.5]
                        .into_iter()
                        .map(move |pan| (on, modulated, pan))
                })
            }) {
                let mut p = fixture::midi_project(48000, 1, 0);
                p.project.tracks[0].pan = 0.0;
                let mut auto = EffectInstance::auto_filter();
                let EffectSettings::AutoFilter(s) = &mut auto.unit else {
                    unreachable!()
                };
                s.stereo_phase_degrees = 180.0;
                s.strength_octaves = 4.0;
                s.enabled = on;
                if modulated {
                    let main = &p.project.tracks[0].instrument.as_ref().unwrap().main;
                    let mut route = Modulation::new(ModulationParameter::AutoFilterStereoPhase);
                    route.source = Some(ModulationSourceId {
                        layer: main.id,
                        unit: main.lfos[0].id,
                    });
                    route.amount = -90.0;
                    s.modulations.push(route);
                }
                let mut identity = EffectInstance::direction_mixer();
                let EffectSettings::DirectionMixer(s) = &mut identity.unit else {
                    unreachable!()
                };
                s.enabled = false;
                if !beat {
                    let track = &mut p.project.tracks[0];
                    match stage {
                        "layer" => track.instrument.as_mut().unwrap().layers[0]
                            .effects
                            .push(auto),
                        "output" => track.instrument.as_mut().unwrap().main.effects.push(auto),
                        _ => track.effects.push(auto),
                    }
                } else {
                    let mut instrument = p.project.tracks[0].instrument.clone().unwrap();
                    match stage {
                        "layer" => instrument.layers[0].effects.push(auto),
                        _ => instrument.main.effects.push(auto),
                    }
                    p.project.tracks.clear();
                    let track = p.project.add_beat_track().unwrap();
                    let mut row = BeatSubtrack::new("Test".into());
                    row.instrument = instrument;
                    let row_id = row.id;
                    p.project
                        .edit(Edit::Beat {
                            track_id: track,
                            edit: BeatEdit::Add {
                                subtrack: Box::new(row),
                                index: 0,
                            },
                        })
                        .unwrap();
                    let clip = p.project.add_beat_pattern(track, 0).unwrap();
                    p.project
                        .edit(Edit::ReplaceBeatPattern {
                            clip_id: clip,
                            pattern: BeatPattern::default()
                                .with_step(row_id, 0, Some(127))
                                .unwrap(),
                        })
                        .unwrap();
                }
                p.project.tracks[0].pan = pan;
                let ret = p.project.add_return_track().unwrap();
                let owner = p.project.tracks[0].id;
                let mut send = TrackSend::new(ret);
                send.gain_db = -6.0;
                p.project
                    .edit(Edit::SetTrackSends {
                        track_id: owner,
                        sends: vec![send],
                    })
                    .unwrap();
                let mut a = Renderer::new(p.clone());
                let instrument = if beat {
                    &mut p.project.tracks[0].beat.as_mut().unwrap().subtracks[0].instrument
                } else {
                    p.project.tracks[0].instrument.as_mut().unwrap()
                };
                match stage {
                    "layer" => instrument.layers[0].effects.push(identity),
                    "output" => instrument.main.effects.push(identity),
                    _ => p.project.tracks[0].effects.push(identity),
                }
                let mut b = Renderer::new(p);
                let mut x = vec![[0.; 2]; 16000];
                let mut y = x.clone();
                a.render_offline(&mut x);
                b.render_offline(&mut y);
                let ratio = x
                    .iter()
                    .zip(&y)
                    .skip(500)
                    .find(|(x, _)| x[0].abs() > 0.001)
                    .map(|(x, y)| y[0] / x[0])
                    .unwrap();
                let stereo = x.iter().any(|s| (s[0] - s[1]).abs() > 1e-4);
                assert!((ratio - 1.0).abs() < 1e-5);
                if pan == 0.0 {
                    assert_eq!(stereo, on);
                }
                assert_eq!(
                    x, y,
                    "beat={beat}, stage={stage}, on={on}, modulated={modulated}, pan={pan}"
                );
            }
        }
    }
}
