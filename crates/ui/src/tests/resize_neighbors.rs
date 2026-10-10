use super::*;

#[test]
fn selected_neighbor_removal_is_only_committed_on_release() {
    for cancel in [false, true] {
        let (mut app, tracks, ids) = multi_clip_fixture();
        // B has no source left to expand; A can expand through all of B.
        app.session.project.tracks[0].clips[1].source_offset_frame = frames(9.25);
        let ctx = context();
        select_group(&mut app, &ctx, ids);
        let original = format!("{:?}", app.session.project);
        let primary = app.session.project.tracks[0].clips[0].clone();
        let block = app.clip_block(app.lane_bounds[&tracks[0]], &primary, primary.start_frame);
        let start = Pos2::new(block.right() - 1.0, block.top() + 40.0);
        let end = start + Vec2::new(6.0 * app.zoom, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, false);
        assert!(previews.iter().all(|preview| preview.valid));
        assert!(
            previews
                .iter()
                .find(|preview| preview.clip.id == ids[1])
                .unwrap()
                .removed
        );
        assert_eq!(format!("{:?}", app.session.project), original);
        assert!(!app.dirty);
        assert_eq!(app.selected_clips, HashSet::from(ids));
        // Moving back restores the neighbor preview without touching the project.
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
        assert!(
            app.clip_drag_previews(app.drag.as_ref().unwrap(), start, false)
                .iter()
                .all(|preview| !preview.removed)
        );
        assert_eq!(format!("{:?}", app.session.project), original);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        if cancel {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        if cancel {
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            assert_eq!(app.selected_clips, HashSet::from(ids));
        } else {
            assert!(
                !app.session.project.tracks[0]
                    .clips
                    .iter()
                    .any(|clip| clip.id == ids[1])
            );
            assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[2]]));
            assert!(app.dirty);
            assert!(app.error.is_none());
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn neighbor_trim_and_removal_previews_cancel_on_focus_loss_for_both_edges() {
    for left in [false, true] {
        for header in [false, true] {
            let (mut app, track) = fixture();
            let primary = &mut app.session.project.tracks[0].clips[0];
            primary.start_frame = frames(3.0);
            primary.source_offset_frame = frames(3.0);
            primary.length_frames = frames(1.0);
            let primary = primary.clone();
            let partial = Clip {
                id: Id::new_v4(),
                start_frame: frames(if left { 0.0 } else { 6.0 }),
                length_frames: frames(2.0),
                source_offset_frame: 0,
                ..primary.clone()
            };
            let covered = Clip {
                id: Id::new_v4(),
                start_frame: frames(if left { 2.5 } else { 5.0 }),
                length_frames: frames(0.25),
                source_offset_frame: 0,
                ..primary.clone()
            };
            app.session.project.tracks[0]
                .clips
                .extend([partial.clone(), covered.clone()]);
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let original = format!("{:?}", app.session.project);
            let block = app.clip_block(app.lane_bounds[&track], &primary, primary.start_frame);
            let start = Pos2::new(
                if left {
                    block.left() + 1.0
                } else {
                    block.right() - 1.0
                },
                block.top() + if header { 12.0 } else { 40.0 },
            );
            let end = start + Vec2::new(if left { -1.5 } else { 3.0 } * app.zoom, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, false);
            let trimmed = previews
                .iter()
                .find(|preview| preview.clip.id == partial.id)
                .unwrap();
            assert!(!trimmed.removed);
            assert!(trimmed.clip.length_frames < partial.length_frames);
            assert!(
                previews
                    .iter()
                    .find(|preview| preview.clip.id == covered.id)
                    .unwrap()
                    .removed
            );
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    events: vec![egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.drag.is_none());
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}
