use super::*;

fn header_edge(app: &DawUi, id: Id, left: bool) -> Pos2 {
    let entry = app
        .selected_clip_snapshots()
        .into_iter()
        .find(|entry| entry.clip.id == id)
        .unwrap();
    let block = app.clip_block(
        app.lane_bounds[&entry.track],
        &entry.clip,
        entry.clip.start_frame,
    );
    Pos2::new(
        if left {
            block.left() + 1.0
        } else {
            block.right() - 1.0
        },
        block.top() + 12.0,
    )
}

fn begin_loop(
    app: &mut DawUi,
    ctx: &egui::Context,
    start: Pos2,
    end: Pos2,
    shift: bool,
) -> Vec<egui::epaint::ClippedShape> {
    frame(app, ctx, vec![]);
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame_shapes(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers {
                shift,
                ..Default::default()
            }),
            egui::Event::PointerMoved(end),
        ],
        Vec2::new(1280.0, 800.0),
    )
}

fn release(app: &mut DawUi, ctx: &egui::Context, end: Pos2, shift: bool) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        }],
    );
}

#[test]
fn selected_header_edges_loop_every_clip_with_shared_preview_and_repeat_snapping() {
    for left in [false, true] {
        for shift in [false, true] {
            let (mut app, _, ids) = multi_clip_fixture();
            let ctx = context();
            select_group(&mut app, &ctx, ids);
            let before = app.selected_clip_snapshots();
            let original = format!("{:?}", app.session.project);
            let start = header_edge(&app, ids[0], left);
            let end = start + Vec2::new(if left { -0.48 } else { 0.48 } * app.zoom, 0.0);
            let shapes = begin_loop(&mut app, &ctx, start, end, shift);
            let drag = app.drag.as_ref().unwrap();
            assert_eq!(drag.clips.len(), 3);
            let previews = app.clip_drag_previews(drag, end, shift);
            let amount = frames(if shift { 0.48 } else { 0.5 });
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            for (preview, original) in previews.iter().zip(&before) {
                assert!(preview.valid);
                assert_eq!(
                    preview.clip.length_frames,
                    original.clip.length_frames + amount
                );
                assert_eq!(
                    preview.clip.start_frame,
                    original.clip.start_frame - if left { amount } else { 0 }
                );
                assert_eq!(
                    preview.clip.source_offset_frame,
                    original.clip.source_offset_frame
                );
                assert_eq!(
                    preview.clip.repeat.unwrap().length_frames,
                    original
                        .clip
                        .repeat
                        .map_or(original.clip.length_frames, |repeat| repeat.length_frames)
                );
                for local in [
                    0,
                    original.clip.length_frames / 2,
                    original.clip.length_frames - 1,
                ] {
                    assert_eq!(
                        preview
                            .clip
                            .source_frame(local + if left { amount } else { 0 }),
                        original.clip.source_frame(local)
                    );
                }
                assert!(has_preview_outline(
                    &shapes,
                    app.clip_block(
                        app.lane_bounds[&preview.track],
                        &preview.clip,
                        preview.clip.start_frame
                    ),
                    theme::ACCENT
                ));
            }
            release(&mut app, &ctx, end, shift);
            assert!(app.error.is_none());
            assert!(app.dirty);
            assert_eq!(app.selected_clips, HashSet::from(ids));
            for (placed, preview) in app.selected_clip_snapshots().iter().zip(&previews) {
                assert_eq!(format!("{:?}", placed.clip), format!("{:?}", preview.clip));
            }
            // An inward group resize stops when any clip reaches its repeat base.
            let start = header_edge(&app, ids[0], left);
            let end = start + Vec2::new(if left { 1.0 } else { -1.0 } * app.zoom, 0.0);
            begin_loop(&mut app, &ctx, start, end, true);
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
            for (preview, original) in previews.iter().zip(&before) {
                assert_eq!(
                    format!("{:?}", preview.clip),
                    format!("{:?}", original.clip)
                );
            }
            release(&mut app, &ctx, end, true);
            assert!(app.error.is_none());
            assert_eq!(app.selected_clips, HashSet::from(ids));
        }
    }
}

#[test]
fn group_loop_overlap_trims_selected_and_unselected_neighbors() {
    for left in [false, true] {
        for selected_obstacle in [false, true] {
            let (mut app, _, ids) = multi_clip_fixture();
            if selected_obstacle {
                app.session.project.tracks[0].clips[0].start_frame = frames(4.0);
            } else {
                let mut obstacle = app.session.project.tracks[0].clips[1].clone();
                obstacle.id = Id::new_v4();
                obstacle.start_frame = frames(if left { 4.5 } else { 6.0 });
                obstacle.length_frames = frames(0.1);
                obstacle.repeat = None;
                app.session.project.tracks[0].clips.push(obstacle);
            }
            app.session.project.validate().unwrap();
            let ctx = context();
            select_group(&mut app, &ctx, ids);
            let original = format!("{:?}", app.session.project);
            let start = header_edge(&app, ids[0], left);
            let amount = if selected_obstacle { 1.0 } else { 0.5 };
            let end = start + Vec2::new(if left { -amount } else { amount } * app.zoom, 0.0);
            let shapes = begin_loop(&mut app, &ctx, start, end, true);
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
            assert_eq!(previews.len(), if selected_obstacle { 3 } else { 4 });
            for preview in &previews {
                assert!(preview.valid);
                if preview.removed {
                    continue;
                }
                assert!(has_preview_outline(
                    &shapes,
                    app.clip_block(
                        app.lane_bounds[&preview.track],
                        &preview.clip,
                        preview.clip.start_frame
                    ),
                    theme::ACCENT
                ));
            }
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            release(&mut app, &ctx, end, true);
            assert_ne!(format!("{:?}", app.session.project), original);
            assert!(app.dirty);
            assert_eq!(app.selected_clips, HashSet::from(ids));
            assert!(app.error.is_none());
            app.session.project.validate().unwrap();
            for preview in &previews {
                let placed = app.session.project.tracks[preview.track_index]
                    .clips
                    .iter()
                    .find(|clip| clip.id == preview.clip.id);
                if preview.removed {
                    assert!(placed.is_none());
                } else {
                    assert_eq!(
                        format!("{:?}", placed.unwrap()),
                        format!("{:?}", preview.clip)
                    );
                }
            }
        }
    }
}

#[test]
fn group_loop_bounds_no_ops_and_cancellation_preserve_selection() {
    for left in [false, true] {
        let (mut app, _, ids) = multi_clip_fixture();
        let ctx = context();
        select_group(&mut app, &ctx, ids);
        let original = format!("{:?}", app.session.project);
        let clips = app.selected_clip_snapshots();
        let drag = Drag {
            clip: clips[1].clip.clone(),
            track: clips[1].track,
            mode: if left {
                ClipDragMode::LoopLeft
            } else {
                ClipDragMode::LoopRight
            },
            origin: Pos2::new(500.0, 100.0),
            duplicate: false,
            linked_boundary: false,
            clips: clips.clone(),
        };
        for delta in [0.0, if left { 20.0 } else { -20.0 }] {
            app.finish_clip_resize(&drag, drag.origin + Vec2::new(delta, 0.0), true);
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
        if left {
            let previews =
                app.loop_group_preview(&drag, drag.origin - Vec2::new(10.0 * app.zoom, 0.0), true);
            for (preview, original) in previews.iter().zip(&clips) {
                assert_eq!(
                    preview.clip.start_frame,
                    original.clip.start_frame - frames(2.0)
                );
                assert_eq!(
                    preview.clip.length_frames,
                    original.clip.length_frames + frames(2.0)
                );
            }
        }
        for focused in [false, true] {
            let start = header_edge(&app, ids[0], left);
            let end = start + Vec2::new(if left { -0.5 } else { 0.5 } * app.zoom, 0.0);
            begin_loop(&mut app, &ctx, start, end, true);
            if focused {
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
            } else {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        focused: false,
                        events: vec![egui::Event::WindowFocused(false)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
            release(&mut app, &ctx, end, true);
            assert!(app.drag.is_none());
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            assert_eq!(app.selected_clips, HashSet::from(ids));
            frame(&mut app, &ctx, vec![egui::Event::WindowFocused(true)]);
        }
    }
}
