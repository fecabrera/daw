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
