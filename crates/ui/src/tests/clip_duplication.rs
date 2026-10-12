use super::*;

#[test]
fn duplicate_previews_and_release_trim_split_or_remove_neighbors() {
    for (start, length, expected) in [
        (8.0, 4.0, vec![(12.0, 20.0)]),
        (12.0, 4.0, vec![(10.0, 12.0), (16.0, 20.0)]),
        (18.0, 4.0, vec![(10.0, 18.0)]),
        (8.0, 14.0, vec![]),
    ] {
        let (mut app, track) = fixture();
        let source = &mut app.session.project.tracks[0].clips[0];
        source.start_frame = frames(30.0);
        source.length_frames = frames(length);
        source.repeat = Some(daw_core::ClipLoop {
            length_frames: frames(1.0),
            phase_frame: frames(0.25),
        });
        let source = source.clone();
        let neighbor = Clip {
            id: Id::new_v4(),
            name: "Neighbor".into(),
            start_frame: frames(10.0),
            length_frames: frames(10.0),
            ..source.clone()
        };
        app.session.project.tracks[0].clips.push(neighbor.clone());
        app.session.project.validate().unwrap();
        let before = format!("{:?}", app.session.project);
        let preview = ClipPreview {
            track,
            track_index: 0,
            valid: true,
            removed: false,
            clip: Clip {
                start_frame: frames(start),
                ..source.clone()
            },
        };
        let previews = app.resolve_move_preview(vec![preview.clone()], true);
        assert!(previews.iter().all(|preview| preview.valid));
        let neighbors: Vec<_> = previews
            .iter()
            .skip(1)
            .filter(|preview| !preview.removed)
            .collect();
        assert_eq!(
            neighbors
                .iter()
                .map(|preview| (preview.clip.start_frame, preview.clip.end()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(start, end)| (frames(*start), frames(*end)))
                .collect::<Vec<_>>()
        );
        if expected.is_empty() {
            assert!(
                previews
                    .iter()
                    .any(|preview| preview.clip.id == neighbor.id && preview.removed)
            );
        }
        assert_eq!(format!("{:?}", app.session.project), before);
        assert!(!app.dirty);
        let drag = Drag {
            clip: source.clone(),
            track,
            mode: ClipDragMode::Move,
            origin: Pos2::ZERO,
            duplicate: true,
            linked_boundary: false,
            clips: Vec::new(),
        };
        app.finish_clip_move(&drag, vec![preview]);
        assert!(app.error.is_none());
        assert!(app.dirty);
        let copy_id = app.selected_clip.unwrap();
        assert_ne!(copy_id, source.id);
        let clips = &app.session.project.tracks[0].clips;
        let original = clips.iter().find(|clip| clip.id == source.id).unwrap();
        assert_eq!(format!("{original:?}"), format!("{source:?}"));
        let copy = clips.iter().find(|clip| clip.id == copy_id).unwrap();
        assert_eq!(copy.length_frames, source.length_frames);
        assert_eq!(copy.repeat, source.repeat);
        assert_eq!(copy.asset_id, source.asset_id);
        assert_eq!(copy.source_offset_frame, source.source_offset_frame);
        let survivors: Vec<_> = clips
            .iter()
            .filter(|clip| clip.name == "Neighbor")
            .collect();
        assert_eq!(survivors.len(), expected.len());
        for (retained, (start, end)) in survivors.iter().zip(&expected) {
            assert_eq!(
                (retained.start_frame, retained.end()),
                (frames(*start), frames(*end))
            );
            for local in [0, retained.length_frames / 2, retained.length_frames - 1] {
                assert_eq!(
                    retained.source_frame(local),
                    neighbor.source_frame(local + retained.start_frame - neighbor.start_frame)
                );
            }
        }
        app.session.project.validate().unwrap();
    }
}

#[test]
fn invalid_copy_group_rolls_back_neighbor_edits_and_new_tracks() {
    let (mut app, track) = fixture();
    let source = app.session.project.tracks[0].clips[0].clone();
    app.selected_clip = Some(source.id);
    let before = format!("{:?}", app.session.project);
    let snapshot = |track_index, clip| SelectedClip {
        track,
        track_index,
        clip,
    };
    let mut next = app.session.project.clone();
    let new_track = next.add_track().unwrap();
    let mut invalid = snapshot(
        1,
        Clip {
            length_frames: 0,
            ..source.clone()
        },
    );
    invalid.track = new_track;
    app.insert_clips(next, vec![snapshot(0, source.clone()), invalid], 0);
    assert!(app.error.is_some());
    assert_eq!(format!("{:?}", app.session.project), before);
    assert_eq!(app.selected_clip, Some(source.id));
    assert!(!app.dirty);
}
