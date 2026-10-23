use daw_core::{Edit, Id, MidiOptions, MidiScale, Project};

#[test]
fn midi_options_round_trip_and_missing_fields_keep_legacy_defaults() {
    let mut project = Project::default();
    let midi = project.add_midi_track().unwrap();
    project.add_track().unwrap();
    for scale in MidiScale::ALL {
        let options = MidiOptions {
            root: Some(11),
            scale,
            snap: true,
        };
        project
            .edit(Edit::SetMidiOptions {
                track_id: midi,
                options,
            })
            .unwrap();
        assert!(
            serde_json::to_value(&project).unwrap()["tracks"][1]
                .get("midi_options")
                .is_none()
        );
        let bytes = serde_json::to_vec(&project).unwrap();
        let reopened: Project = serde_json::from_slice(&bytes).unwrap();
        reopened.validate().unwrap();
        assert_eq!(reopened, project);
        let mut legacy = serde_json::to_value(&project).unwrap();
        legacy["tracks"][0]
            .as_object_mut()
            .unwrap()
            .remove("midi_options");
        let legacy: Project = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy.tracks[0].midi_options, MidiOptions::default());
        legacy.validate().unwrap();
    }
    let partial: MidiOptions = serde_json::from_str(r#"{"root":7}"#).unwrap();
    assert_eq!(
        partial,
        MidiOptions {
            root: Some(7),
            ..Default::default()
        }
    );
    assert!(serde_json::from_str::<MidiOptions>(r#"{"scale":"unknown"}"#).is_err());
}

#[test]
fn options_are_independent_copy_with_tracks_and_never_modify_patterns() {
    let mut project = Project::default();
    let first = project.add_midi_track().unwrap();
    let second = project.add_midi_track().unwrap();
    project.add_midi_pattern(first, 0).unwrap();
    let assets = project.assets.clone();
    let options = MidiOptions {
        root: Some(9),
        scale: MidiScale::Minor,
        snap: true,
    };
    project
        .edit(Edit::SetMidiOptions {
            track_id: first,
            options,
        })
        .unwrap();
    assert_eq!(project.tracks[1].midi_options, MidiOptions::default());
    let copy = Id::new_v4();
    project
        .edit(Edit::DuplicateTrack {
            track_id: first,
            new_track_id: copy,
        })
        .unwrap();
    assert_eq!(
        project
            .tracks
            .iter()
            .find(|t| t.id == copy)
            .unwrap()
            .midi_options,
        options
    );
    project
        .edit(Edit::SetMidiOptions {
            track_id: first,
            options: MidiOptions::default(),
        })
        .unwrap();
    assert_eq!(
        project
            .tracks
            .iter()
            .find(|t| t.id == copy)
            .unwrap()
            .midi_options,
        options
    );
    assert_eq!(
        project
            .tracks
            .iter()
            .find(|t| t.id == second)
            .unwrap()
            .midi_options,
        MidiOptions::default()
    );
    assert_eq!(project.assets, assets);
}

#[test]
fn invalid_roots_tracks_and_batches_reject_options_atomically() {
    let mut project = Project::default();
    let midi = project.add_midi_track().unwrap();
    let audio = project.add_track().unwrap();
    let original = project.clone();
    for root in [12, 127, 255] {
        let options = MidiOptions {
            root: Some(root),
            ..Default::default()
        };
        assert!(
            project
                .edit(Edit::SetMidiOptions {
                    track_id: midi,
                    options
                })
                .is_err()
        );
        assert_eq!(project, original);
        let mut invalid = original.clone();
        invalid.tracks[0].midi_options = options;
        assert!(invalid.validate().is_err());
    }
    let options = MidiOptions {
        root: Some(7),
        ..Default::default()
    };
    for track_id in [audio, Id::new_v4()] {
        assert!(
            project
                .edit(Edit::SetMidiOptions { track_id, options })
                .is_err()
        );
        assert_eq!(project, original);
    }
    assert!(
        project
            .edit(Edit::Batch(vec![
                Edit::SetMidiOptions {
                    track_id: midi,
                    options
                },
                Edit::SetMidiOptions {
                    track_id: audio,
                    options
                },
            ]))
            .is_err()
    );
    assert_eq!(project, original);
    project.tracks[1].midi_options = options;
    assert!(project.validate().is_err());
}
