use super::*;

#[test]
fn duplicate_into_new_track_uses_defaults_and_commits_only_on_release() {
    for midi in [false, true] {
        for cancel in [false, true] {
            let (mut app, track) = if midi {
                let mut app = DawUi::default();
                let track = app.session.project.add_midi_track().unwrap();
                app.session.project.add_midi_pattern(track, 0).unwrap();
                app.session.project.tracks[0].instrument =
                    Some(daw_core::OscillatorSettings::default().into());
                (app, track)
            } else {
                fixture()
            };
            let source = &mut app.session.project.tracks[0];
            source.name = "Custom sound".into();
            source.gain_db = -9.0;
            source.pan = 0.5;
            source.muted = true;
            source.soloed = true;
            source.effects_compact = true;
            let original = source.clips[0].clone();
            let before = format!("{:?}", app.session.project);
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let block = app.clip_block(lane, &original, original.start_frame);
            let start = Pos2::new(block.center().x, block.top() + 12.0);
            let end = Pos2::new(start.x, lane.bottom() + 12.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let modifiers = egui::Modifiers {
                alt: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Default::default()
            };
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerMoved(end),
                ],
            );
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, false);
            assert!(previews.iter().all(|p| p.valid));
            assert_eq!(previews[0].track_index, 1);
            assert_eq!(format!("{:?}", app.session.project), before);
            assert!(!app.dirty);
            if cancel {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                );
            }
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                }],
            );
            assert!(app.error.is_none(), "{:?}", app.error);
            if cancel {
                assert_eq!(format!("{:?}", app.session.project), before);
                assert!(!app.dirty);
                continue;
            }
            assert_eq!(app.session.project.tracks.len(), 2);
            let fresh = &app.session.project.tracks[1];
            assert_ne!(fresh.id, track);
            assert_eq!(fresh.name, if midi { "MIDI 2" } else { "Audio 2" });
            assert_eq!(fresh.color, daw_core::default_track_color(1));
            assert_eq!((fresh.gain_db, fresh.pan), (0.0, 0.0));
            assert!(!fresh.muted && !fresh.soloed && !fresh.effects_compact);
            assert!(fresh.effects.is_empty());
            if midi {
                let instrument = fresh.instrument.as_ref().unwrap();
                assert_eq!(instrument.layers.len(), 1);
                assert!(instrument.layers[0].oscillators.is_empty());
                assert!(instrument.layers[0].sampler.is_none());
            } else {
                assert!(fresh.instrument.is_none());
            }
            let copy = &fresh.clips[0];
            assert_ne!(copy.id, original.id);
            let mut expected = original.clone();
            expected.id = copy.id;
            expected.start_frame = previews[0].clip.start_frame;
            assert_eq!(format!("{copy:?}"), format!("{expected:?}"));
            assert_eq!(
                format!("{:?}", app.session.project.tracks[0].clips[0]),
                format!("{original:?}")
            );
            assert_eq!(app.selected_track, Some(fresh.id));
            assert_eq!(app.selected_clip, Some(copy.id));
            assert!(app.dirty);
            app.session.project.validate().unwrap();
        }
    }
}

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
