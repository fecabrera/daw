use daw_core::{Edit, TrackSend};
use daw_project::Session;
#[test]
fn held_note_preview_accepts_a_track_with_a_return_send() {
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let ret = session.project.add_return_track().unwrap();
    session
        .project
        .edit(Edit::SetTrackSends {
            track_id: track,
            sends: vec![TrackSend::new(ret)],
        })
        .unwrap();
    session.project.validate().unwrap();
    let result = session.held_note_audition(track, 60, &|| false);
    assert!(result.is_ok(), "{}", result.err().unwrap());
}
#[test]
fn note_and_beat_previews_accept_direct_or_ancestor_sends() {
    let mut failures = vec![];
    for beat in [false, true] {
        for ancestor in [false, true] {
            for enabled in [false, true] {
                let mut session = Session::default();
                let track = if beat {
                    session.project.add_beat_track().unwrap()
                } else {
                    session.project.add_midi_track().unwrap()
                };
                if beat {
                    session
                        .project
                        .edit(Edit::Beat {
                            track_id: track,
                            edit: daw_core::BeatEdit::Add {
                                subtrack: Box::new(daw_core::BeatSubtrack::new("Kick".into())),
                                index: 0,
                            },
                        })
                        .unwrap();
                }
                let owner = if ancestor {
                    let g = daw_core::Id::new_v4();
                    session
                        .project
                        .edit(Edit::GroupTracks {
                            track_ids: vec![track],
                            group_id: g,
                            name: "Group".into(),
                        })
                        .unwrap();
                    g
                } else {
                    track
                };
                let ret = session.project.add_return_track().unwrap();
                let mut send = TrackSend::new(ret);
                send.enabled = enabled;
                session
                    .project
                    .edit(Edit::SetTrackSends {
                        track_id: owner,
                        sends: vec![send],
                    })
                    .unwrap();
                let result = if beat {
                    let row = session.project.beat(track).unwrap().subtracks[0].id;
                    let mut held = session
                        .beat_subtrack_audition(track, row, 0, Default::default(), &|| false)
                        .unwrap();
                    assert!(held.next_chunk(false).is_some());
                    session.audition_beat_step(track, row, 0, Default::default(), &|| false)
                } else {
                    let mut held = session.held_note_audition(track, 60, &|| false).unwrap();
                    assert!(held.next_chunk(false).is_some());
                    session.audition_note(track, 60, &|| false)
                };
                if let Err(error) = result {
                    failures.push(format!(
                        "beat={beat}, ancestor={ancestor}, enabled={enabled}: {error}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
#[test]
fn retaining_graph_owners_and_clearing_clips_preserves_valid_isolation() {
    use daw_core::{
        EffectInstance, EffectSettings, Id, Modulation, ModulationParameter, OscillatorSettings,
    };
    fn isolated_audio(session: Session, track: Id) -> Vec<[f32; 2]> {
        let before = session.project.clone();
        let samples = session.audition_note(track, 60, &|| false).unwrap();
        assert_eq!(session.project, before);
        samples
    }

    for ancestor in [false, true] {
        for enabled in [false, true] {
            for silent in [false, true] {
                let mut session = Session::default();
                let track = session.project.add_midi_track().unwrap();
                session.project.tracks[0].instrument = Some(OscillatorSettings::default().into());
                let owner = if ancestor {
                    let g = Id::new_v4();
                    session
                        .project
                        .edit(Edit::GroupTracks {
                            track_ids: vec![track],
                            group_id: g,
                            name: "Group".into(),
                        })
                        .unwrap();
                    g
                } else {
                    track
                };
                let ret = session.project.add_return_track().unwrap();
                let mut send = TrackSend::new(ret);
                send.enabled = enabled;
                if !silent {
                    send.gain_db = -6.0;
                }
                session
                    .project
                    .edit(Edit::SetTrackSends {
                        track_id: owner,
                        sends: vec![send],
                    })
                    .unwrap();
                let other = session.project.add_midi_track().unwrap();
                let i = session
                    .project
                    .tracks
                    .iter()
                    .position(|t| t.id == other)
                    .unwrap();
                session.project.tracks[i].instrument = Some(OscillatorSettings::default().into());
                let mut effect = EffectInstance::drive();
                let mut route = Modulation::new(ModulationParameter::Volume);
                route.sidechain = Some(
                    serde_json::from_value(serde_json::json!({
                        "version": 1, "track": other,
                        "tap": "pre_fader", "channel": "stereo", "detector": "peak",
                        "filter": false, "high_pass_hz": 30.0, "low_pass_hz": 20000.0,
                        "lower_db": -36.0, "upper_db": -6.0,
                        "attack_ms": 5.0, "release_ms": 120.0,
                        "curve": "decibels", "invert": false
                    }))
                    .unwrap(),
                );
                if let EffectSettings::Drive(s) = &mut effect.unit {
                    s.modulations.push(route);
                }
                session
                    .project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![effect],
                        compact: false,
                    })
                    .unwrap();
                let expected = isolated_audio(session.clone(), track);
                assert!(expected.iter().flatten().any(|x| x.abs() > 0.0001));
                let clip = session.project.add_midi_pattern(other, 0).unwrap();
                session
                    .project
                    .edit(Edit::ReplaceMidiPattern {
                        clip_id: clip,
                        pattern: daw_core::MidiPattern {
                            length_ticks: 960,
                            notes: vec![daw_core::MidiNote {
                                id: Id::new_v4(),
                                start_tick: 0,
                                length_ticks: 960,
                                pitch: 84,
                                velocity: 127,
                            }],
                        },
                    })
                    .unwrap();
                session.project.tracks[i].soloed = true;
                assert_eq!(
                    isolated_audio(session.clone(), track),
                    expected,
                    "unrelated soloed sounding track leaked"
                );
                for t in &mut session.project.tracks {
                    if t.id == track {
                        t.muted = true;
                        t.sends.clear();
                    }
                }
                if ancestor {
                    session
                        .project
                        .edit(Edit::SetTrackSends {
                            track_id: owner,
                            sends: vec![],
                        })
                        .unwrap();
                }
                assert!(
                    isolated_audio(session, track)
                        .iter()
                        .flatten()
                        .all(|x| *x == 0.0)
                );
            }
        }
    }
}
