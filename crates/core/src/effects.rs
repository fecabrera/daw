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
