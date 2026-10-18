#[cfg(test)]
mod compressor_tests {
    use crate::*;
    #[test]
    fn compressor_state_round_trips_and_duplicate_ids_change() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let mut effect = EffectInstance::compressor();
        let EffectSettings::Compressor(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.detection = CompressorDetection::Rms;
        settings.enabled = false;
        settings.collapsed = true;
        settings.threshold_db = -36.0;
        settings.ratio = 8.0;
        settings.knee_db = 12.0;
        settings.attack_ms = 0.1;
        settings.release_ms = 2000.0;
        settings.level_db = SILENT_GAIN_DB;
        settings.dry_wet = 0.5;
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: true,
            })
            .unwrap();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.tracks[0].effects, vec![effect.clone()]);
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: Id::new_v4(),
            })
            .unwrap();
        assert_eq!(project.tracks[1].effects[0].unit, effect.unit);
        assert_ne!(project.tracks[1].effects[0].id, effect.id);
        assert!(project.tracks[1].effects_compact);
        let encoded = serde_json::json!({"type_id": COMPRESSOR_TYPE_ID, "state": {"version": 99, "future_control": [1, 2]}});
        let future: EffectSettings = serde_json::from_value(encoded.clone()).unwrap();
        assert!(future.valid());
        assert!(matches!(future, EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(future).unwrap(), encoded);
    }
    #[test]
    fn compressor_invalid_values_reject_the_entire_chain_edit() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let effect = EffectInstance::compressor();
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: false,
            })
            .unwrap();
        let saved = serde_json::to_string(&project).unwrap();
        for parameter in 0..11 {
            let mut bad = effect.clone();
            let EffectSettings::Compressor(s) = &mut bad.unit else {
                unreachable!()
            };
            match parameter {
                0 => s.threshold_db = -60.1,
                1 => s.threshold_db = f32::NAN,
                2 => s.ratio = 0.9,
                3 => s.ratio = 20.1,
                4 => s.attack_ms = 0.0,
                5 => s.release_ms = f32::INFINITY,
                6 => s.knee_db = 24.1,
                7 => s.knee_db = -0.1,
                8 => s.level_db = 12.1,
                9 => s.dry_wet = -0.1,
                _ => s.version = 0,
            }
            assert!(
                project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![EffectInstance::drive(), bad],
                        compact: true
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_string(&project).unwrap(), saved);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn effect_edits_validate_atomically_and_duplicate_with_new_ids() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let eq = EffectInstance::equalizer();
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![eq.clone()],
                compact: true,
            })
            .unwrap();
        let original = serde_json::to_value(&project).unwrap();
        let mut bad = eq.clone();
        let EffectSettings::Eq3(settings) = &mut bad.unit else {
            unreachable!()
        };
        settings.bands[1].q = 0.0;
        assert!(
            project
                .edit(Edit::SetEffects {
                    track_id: track,
                    effects: vec![bad],
                    compact: false
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), original);
        assert!(
            project
                .edit(Edit::SetEffects {
                    track_id: track,
                    effects: vec![eq.clone(), eq.clone()],
                    compact: false
                })
                .is_err()
        );
        assert!(
            project
                .edit(Edit::SetEffects {
                    track_id: Id::new_v4(),
                    effects: vec![],
                    compact: false
                })
                .is_err()
        );
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: Id::new_v4(),
            })
            .unwrap();
        assert_ne!(project.tracks[1].effects[0].id, eq.id);
        assert_eq!(project.tracks[1].effects[0].unit, eq.unit);
        assert!(project.tracks[1].effects_compact);
        assert!(project.tracks[1].clips.is_empty());
    }
    #[test]
    fn old_projects_default_to_no_effects_and_unknown_effects_round_trip() {
        let mut project = Project::default();
        project.add_track().unwrap();
        let mut json = serde_json::to_value(&project).unwrap();
        json["tracks"][0].as_object_mut().unwrap().remove("effects");
        json["tracks"][0]
            .as_object_mut()
            .unwrap()
            .remove("effects_compact");
        let mut loaded: Project = serde_json::from_value(json).unwrap();
        assert!(loaded.tracks[0].effects.is_empty());
        assert!(!loaded.tracks[0].effects_compact);
        let missing = EffectInstance {
            id: Id::new_v4(),
            unit: EffectSettings::Missing {
                type_id: "third.party.effect".into(),
                state: serde_json::json!({"version": 42, "opaque": [1, 2, 3]}),
            },
        };
        loaded.tracks[0].effects.push(missing.clone());
        loaded.validate().unwrap();
        let reopened: Project =
            serde_json::from_str(&serde_json::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(reopened.tracks[0].effects, vec![missing]);
        let future = serde_json::json!({"id": Id::new_v4(), "unit": {"type_id": daw_unit_core::EQ_TYPE_ID, "state": {"version": 99, "future": "retain"}}});
        let future: EffectInstance = serde_json::from_value(future.clone()).unwrap();
        assert!(future.unit.valid());
        assert!(matches!(future.unit, EffectSettings::Missing { .. }));
        let encoded = serde_json::to_value(future).unwrap();
        assert_eq!(encoded["unit"]["state"]["future"], "retain");
    }
}

#[cfg(test)]
mod drive_tests {
    use crate::*;
    #[test]
    fn drive_settings_round_trip_and_invalid_edits_leave_the_project_unchanged() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let mut effect = EffectInstance::drive();
        let EffectSettings::Drive(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.collapsed = true;
        settings.color_shape = DriveColorShape::MidHumpBassCut;
        settings.color_frequency_hz = 1250.0;
        settings.gain = 0.75;
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: true,
            })
            .unwrap();
        let saved = serde_json::to_string(&project).unwrap();
        let restored: Project = serde_json::from_str(&saved).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.tracks[0].effects, vec![effect.clone()]);
        for parameter in 0..6 {
            let mut bad = effect.clone();
            let EffectSettings::Drive(settings) = &mut bad.unit else {
                unreachable!()
            };
            match parameter {
                0 => settings.volume_db = 12.1,
                1 => settings.gain = f32::NAN,
                2 => settings.tone = -0.01,
                3 => settings.color_strength = 1.01,
                4 => settings.color_frequency_hz = 2001.0,
                _ => settings.version = 0,
            }
            assert!(
                project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![bad],
                        compact: false
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_string(&project).unwrap(), saved);
        }
        let copy = Id::new_v4();
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: copy,
            })
            .unwrap();
        assert_eq!(project.tracks[1].effects[0].unit, effect.unit);
        assert_ne!(project.tracks[1].effects[0].id, effect.id);
        assert!(project.tracks[1].effects_compact);
        let future = serde_json::json!({"type_id": DRIVE_TYPE_ID, "state": {"version": 99, "opaque": [1, 2]}});
        let decoded: EffectSettings = serde_json::from_value(future.clone()).unwrap();
        assert!(decoded.valid());
        assert!(matches!(decoded, EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(decoded).unwrap(), future);
    }
}

#[cfg(test)]
mod auto_filter_tests {
    use crate::*;
    #[test]
    fn auto_filter_state_round_trips_duplicates_and_validates_atomically() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let mut effect = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.collapsed = true;
        settings.wave = LfoWave::SawDown;
        settings.sync_phase_degrees = -180.0;
        settings.stereo_phase_degrees = 180.0;
        settings.rate_mode = LfoRateMode::Hz;
        settings.rate_hz = 0.01;
        settings.note_rate = NoteRate(26);
        settings.shape = FilterShape::Notch;
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: true,
            })
            .unwrap();
        let saved = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&saved).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.tracks[0].effects, vec![effect.clone()]);
        for parameter in 0..11 {
            let mut bad = effect.clone();
            let EffectSettings::AutoFilter(s) = &mut bad.unit else {
                unreachable!()
            };
            match parameter {
                0 => s.frequency_hz = 20001.0,
                1 => s.q = 0.0,
                2 => s.rate_hz = f32::NAN,
                3 => s.note_rate = NoteRate(27),
                4 => s.strength_octaves = 6.1,
                5 => s.sync_phase_degrees = -181.0,
                6 => s.stereo_phase_degrees = 181.0,
                7 => s.level_db = 12.1,
                8 => s.dry_wet = -0.01,
                9 => s.version = 0,
                _ => s.q = f32::INFINITY,
            }
            assert!(
                project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![bad],
                        compact: false
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_string(&project).unwrap(), saved);
        }
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: Id::new_v4(),
            })
            .unwrap();
        assert_ne!(project.tracks[1].effects[0].id, effect.id);
        assert_eq!(project.tracks[1].effects[0].unit, effect.unit);
        let encoded = serde_json::json!({"type_id": AUTO_FILTER_TYPE_ID, "state": {"version": 99, "unknown": "retained"}});
        let future: EffectSettings = serde_json::from_value(encoded.clone()).unwrap();
        assert!(future.valid());
        assert!(matches!(future, EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(future).unwrap(), encoded);
    }
}

#[cfg(test)]
mod ott_tests {
    use crate::*;
    #[test]
    fn ott_settings_roundtrip_duplicate_and_future_state_survive() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let mut effect = EffectInstance::ott();
        let EffectSettings::Ott(s) = &mut effect.unit else {
            unreachable!()
        };
        s.enabled = false;
        s.collapsed = true;
        s.detection = CompressorDetection::Rms;
        s.low_frequency_hz = 100.0;
        s.high_frequency_hz = 10_000.0;
        s.bands[0].depth = 0.2;
        s.bands[1].enabled = false;
        s.bands[2].output_db = -12.0;
        s.bands[2].attack_ms = 0.1;
        s.bands[0].release_ms = 2000.0;
        s.level_db = SILENT_GAIN_DB;
        s.dry_wet = 0.25;
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: true,
            })
            .unwrap();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.tracks[0].effects, vec![effect.clone()]);
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: Id::new_v4(),
            })
            .unwrap();
        assert_eq!(project.tracks[1].effects[0].unit, effect.unit);
        assert_ne!(project.tracks[1].effects[0].id, effect.id);
        let encoded =
            serde_json::json!({"type_id": OTT_TYPE_ID, "state": {"version": 99, "future": [1, 2]}});
        let unknown: EffectSettings = serde_json::from_value(encoded.clone()).unwrap();
        assert!(unknown.valid());
        assert!(matches!(unknown, EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(unknown).unwrap(), encoded);
    }
    #[test]
    fn ott_invalid_band_and_crossover_values_roll_back_chain_atomically() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let before = serde_json::to_value(&project).unwrap();
        for parameter in 0..12 {
            let mut effect = EffectInstance::ott();
            let EffectSettings::Ott(s) = &mut effect.unit else {
                unreachable!()
            };
            match parameter {
                0 => s.low_frequency_hz = 0.0,
                1 => s.high_frequency_hz = f32::NAN,
                2 => {
                    s.low_frequency_hz = 2000.0;
                    s.high_frequency_hz = 2000.0;
                }
                3 => s.bands[0].depth = 1.1,
                4 => s.bands[1].input_db = 24.1,
                5 => s.bands[2].output_db = -24.1,
                6 => s.bands[1].attack_ms = 0.0,
                7 => s.bands[2].release_ms = f32::INFINITY,
                8 => s.level_db = 12.1,
                9 => s.dry_wet = -0.1,
                10 => s.version = 0,
                _ => s.bands[0].depth = f32::NAN,
            }
            assert!(
                project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![EffectInstance::drive(), effect],
                        compact: true
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_value(&project).unwrap(), before);
        }
    }
}

#[cfg(test)]
mod master_tests {
    use crate::*;

    #[test]
    fn master_effects_migrate_round_trip_and_validate_atomically() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let mut legacy = serde_json::to_value(&project).unwrap();
        legacy["master"].as_object_mut().unwrap().remove("effects");
        legacy["master"]
            .as_object_mut()
            .unwrap()
            .remove("effects_compact");
        let legacy: Project = serde_json::from_value(legacy).unwrap();
        assert!(legacy.master.effects.is_empty());
        assert!(!legacy.master.effects_compact);
        legacy.validate().unwrap();
        let chain = vec![
            EffectInstance::equalizer(),
            EffectInstance::drive(),
            EffectInstance::auto_filter(),
            EffectInstance::compressor(),
            EffectInstance::ott(),
            EffectInstance {
                id: Id::new_v4(),
                unit: EffectSettings::Missing {
                    type_id: "future.master".into(),
                    state: serde_json::json!({"state": [1, 2]}),
                },
            },
        ];
        project
            .edit(Edit::SetMasterEffects {
                effects: chain.clone(),
                compact: true,
            })
            .unwrap();
        let encoded = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&encoded).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.master.effects, chain);
        assert!(loaded.master.effects_compact);
        assert!(loaded.tracks[0].effects.is_empty());
        let mut bad = EffectInstance::compressor();
        let EffectSettings::Compressor(settings) = &mut bad.unit else {
            unreachable!()
        };
        settings.ratio = 0.0;
        for effects in [vec![bad], vec![chain[0].clone(), chain[0].clone()]] {
            assert!(
                project
                    .edit(Edit::SetMasterEffects {
                        effects,
                        compact: false
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_string(&project).unwrap(), encoded);
        }
        assert!(
            project
                .edit(Edit::SetEffects {
                    track_id: track,
                    effects: vec![chain[0].clone()],
                    compact: false
                })
                .is_err()
        );
        assert_eq!(serde_json::to_string(&project).unwrap(), encoded);
        let mut duplicate_track = chain[0].clone();
        duplicate_track.id = track;
        assert!(
            project
                .edit(Edit::SetMasterEffects {
                    effects: vec![duplicate_track],
                    compact: false
                })
                .is_err()
        );
        assert_eq!(serde_json::to_string(&project).unwrap(), encoded);
    }
}

#[cfg(test)]
mod limiter_tests {
    use crate::*;
    #[test]
    fn limiter_round_trips_on_track_and_master_and_future_state_is_preserved() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let mut effect = EffectInstance::limiter();
        let EffectSettings::Limiter(s) = &mut effect.unit else {
            unreachable!()
        };
        s.input_db = 18.0;
        s.ceiling_db = -3.0;
        s.lookahead_ms = 10.0;
        s.release_ms = 500.0;
        s.routing = LimiterRouting::MidSide;
        s.mode = LimiterMode::TruePeak;
        s.collapsed = true;
        s.enabled = false;
        project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: true,
            })
            .unwrap();
        let mut master = effect.clone();
        master.id = Id::new_v4();
        project
            .edit(Edit::SetMasterEffects {
                effects: vec![master],
                compact: false,
            })
            .unwrap();
        let loaded: Project =
            serde_json::from_value(serde_json::to_value(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(
            serde_json::to_value(loaded).unwrap(),
            serde_json::to_value(&project).unwrap()
        );
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: Id::new_v4(),
            })
            .unwrap();
        assert_eq!(project.tracks[1].effects[0].unit, effect.unit);
        assert_ne!(project.tracks[1].effects[0].id, effect.id);
        let encoded = serde_json::json!({"type_id": LIMITER_TYPE_ID, "state": {"version": 99, "future": [1, 2]}});
        let future: EffectSettings = serde_json::from_value(encoded.clone()).unwrap();
        assert!(future.valid());
        assert_eq!(serde_json::to_value(future).unwrap(), encoded);
    }
    #[test]
    fn invalid_limiter_parameters_reject_the_complete_transaction() {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let original = serde_json::to_string(&project).unwrap();
        for field in 0..9 {
            let mut effect = EffectInstance::limiter();
            let EffectSettings::Limiter(s) = &mut effect.unit else {
                unreachable!()
            };
            match field {
                0 => s.input_db = -24.1,
                1 => s.input_db = f32::NAN,
                2 => s.release_ms = 0.0,
                3 => s.release_ms = 2000.1,
                4 => s.lookahead_ms = -0.1,
                5 => s.lookahead_ms = 10.1,
                6 => s.ceiling_db = 0.1,
                7 => s.ceiling_db = f32::INFINITY,
                _ => s.version = 0,
            }
            assert!(
                project
                    .edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![EffectInstance::drive(), effect.clone()],
                        compact: true
                    })
                    .is_err()
            );
            assert!(
                project
                    .edit(Edit::SetMasterEffects {
                        effects: vec![effect],
                        compact: true
                    })
                    .is_err()
            );
            assert_eq!(serde_json::to_string(&project).unwrap(), original);
        }
    }
}

#[cfg(test)]
mod effect_modulation_tests {
    use crate::*;
    fn fixture(kind: usize) -> (Project, Id, EffectInstance) {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let layer = &mut p.tracks[0].instrument.as_mut().unwrap().layers[0];
        layer.envelopes[0].present = true;
        layer.envelopes[0].mode = EnvelopeMode::Retrigger;
        let source = ModulationSourceId {
            layer: layer.id,
            unit: layer.envelopes[0].id,
        };
        let mut effect = [
            EffectInstance::equalizer,
            EffectInstance::compressor,
            EffectInstance::limiter,
            EffectInstance::ott,
            EffectInstance::direction_mixer,
            EffectInstance::stereo_spread,
        ][kind]();
        let (open, routes) = match &mut effect.unit {
            EffectSettings::Eq3(s) => (&mut s.modulation_open, &mut s.modulations),
            EffectSettings::Compressor(s) => (&mut s.modulation_open, &mut s.modulations),
            EffectSettings::Limiter(s) => (&mut s.modulation_open, &mut s.modulations),
            EffectSettings::Ott(s) => (&mut s.modulation_open, &mut s.modulations),
            EffectSettings::DirectionMixer(s) => (&mut s.modulation_open, &mut s.modulations),
            EffectSettings::StereoSpread(s) => (&mut s.modulation_open, &mut s.modulations),
            _ => unreachable!(),
        };
        *open = true;
        *routes = [
            &ModulationParameter::EQUALIZER[..],
            &ModulationParameter::COMPRESSOR[..],
            &ModulationParameter::LIMITER[..],
            &ModulationParameter::OTT[..],
            &ModulationParameter::DIRECTION_MIXER[..],
            &ModulationParameter::STEREO_SPREAD[..],
        ][kind]
            .iter()
            .copied()
            .map(|parameter| Modulation {
                source: Some(source),
                amount: -parameter.limit(),
                ..Modulation::new(parameter)
            })
            .collect();
        if kind == 4 {
            let EffectSettings::DirectionMixer(s) = &mut effect.unit else {
                unreachable!()
            };
            let extra: Vec<_> = s
                .modulations
                .iter()
                .map(|r| Modulation {
                    id: Id::new_v4(),
                    ..*r
                })
                .collect();
            s.modulations.extend(extra);
        }
        (p, track, effect)
    }
    #[test]
    fn effect_modulation_defaults_roundtrip_and_invalid_edits_are_atomic() {
        for kind in 0..6 {
            let (mut p, track, effect) = fixture(kind);
            p.edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: false,
            })
            .unwrap();
            let saved = serde_json::to_value(&p).unwrap();
            let loaded: Project = serde_json::from_value(saved.clone()).unwrap();
            loaded.validate().unwrap();
            assert_eq!(loaded.tracks[0].effects[0], effect);
            for case in 0..9 {
                let mut invalid = effect.clone();
                let routes = invalid.unit.modulations_mut();
                match case {
                    0 => routes[0].amount = f32::NAN,
                    1 => routes[1].amount = routes[1].parameter.limit() + 0.01,
                    2 => routes[2].amount = -routes[2].parameter.limit() - 0.01,
                    3 => routes[3].parameter = ModulationParameter::Amplitude,
                    4 => routes[0].source.as_mut().unwrap().unit = Id::new_v4(),
                    5 => routes[0].target = Some(Id::new_v4()),
                    6 => routes[0].id = Id::nil(),
                    7 => routes[0].amount = routes[0].parameter.limit() + 0.01,
                    _ => routes[1].id = routes[0].id,
                }
                assert!(
                    p.edit(Edit::SetEffects {
                        track_id: track,
                        effects: vec![invalid],
                        compact: true
                    })
                    .is_err()
                );
                assert_eq!(serde_json::to_value(&p).unwrap(), saved);
            }
            let expected = [
                EffectInstance::equalizer,
                EffectInstance::compressor,
                EffectInstance::limiter,
                EffectInstance::ott,
                EffectInstance::direction_mixer,
                EffectInstance::stereo_spread,
            ][kind]()
            .unit;
            let mut legacy = serde_json::to_value(&expected).unwrap();
            legacy["state"]
                .as_object_mut()
                .unwrap()
                .remove("modulations");
            legacy["state"]
                .as_object_mut()
                .unwrap()
                .remove("modulation_open");
            assert_eq!(
                serde_json::from_value::<EffectSettings>(legacy.clone()).unwrap(),
                expected
            );
            for field in ["modulations", "modulation_open"] {
                let mut malformed = legacy.clone();
                malformed["state"][field] = serde_json::Value::Null;
                assert!(serde_json::from_value::<EffectSettings>(malformed).is_err());
            }
        }
    }

    #[test]
    fn effect_modulation_uses_same_instrument_shared_sources_and_follows_provider_moves() {
        for kind in 0..6 {
            let (mut p, track, effect) = fixture(kind);
            let source = effect.unit.modulations()[0].source.unwrap();
            let mut instrument = p.tracks[0].instrument.clone().unwrap();
            instrument.layers[0].envelopes[0].mode = EnvelopeMode::PerNote;
            p.edit(Edit::SetEffects {
                track_id: track,
                effects: vec![effect.clone()],
                compact: false,
            })
            .unwrap();
            let saved = serde_json::to_value(&p).unwrap();
            assert!(
                p.edit(Edit::SetInstrument {
                    track_id: track,
                    instrument: Some(instrument)
                })
                .is_err()
            );
            assert_eq!(serde_json::to_value(&p).unwrap(), saved);
            assert!(
                p.edit(Edit::SetMasterEffects {
                    effects: vec![EffectInstance {
                        id: Id::new_v4(),
                        ..effect.clone()
                    }],
                    compact: false
                })
                .is_err()
            );
            let audio = p.add_track().unwrap();
            assert!(
                p.edit(Edit::SetEffects {
                    track_id: audio,
                    effects: vec![EffectInstance {
                        id: Id::new_v4(),
                        ..effect.clone()
                    }],
                    compact: false
                })
                .is_err()
            );
            let destination = p.tracks[0].instrument.as_ref().unwrap().main.id;
            p.edit(Edit::TransferInstrumentComponent {
                track_id: track,
                source: source.layer,
                destination,
                destination_index: None,
                component: InstrumentComponent::Envelope(source.unit),
                duplicate: false,
            })
            .unwrap();
            assert!(
                p.tracks[0].effects[0]
                    .unit
                    .modulations()
                    .iter()
                    .all(|r| r.source.unwrap().layer == destination)
            );
            p.validate().unwrap();
            // LFO and Macro metadata use the same shared scope validation as Envelopes.
            let instrument = p.tracks[0].instrument.as_mut().unwrap();
            let lfo = LfoInstance::default();
            let lfo_source = ModulationSourceId {
                layer: destination,
                unit: lfo.id,
            };
            instrument.main.lfos.push(lfo);
            instrument.main.macro_module.add();
            let macro_source = ModulationSourceId {
                layer: destination,
                unit: instrument.main.macro_module.controls[0].id,
            };
            for source in [lfo_source, macro_source] {
                let mut effect = p.tracks[0].effects[0].clone();
                for r in effect.unit.modulations_mut() {
                    r.source = Some(source);
                }
                p.edit(Edit::SetEffects {
                    track_id: track,
                    effects: vec![effect],
                    compact: false,
                })
                .unwrap();
            }
            let other_track = p.add_midi_track().unwrap();
            let mut foreign = p.tracks[0].effects[0].clone();
            foreign.id = Id::new_v4();
            assert!(
                p.edit(Edit::SetEffects {
                    track_id: other_track,
                    effects: vec![foreign],
                    compact: false
                })
                .is_err()
            );
        }
    }
}
