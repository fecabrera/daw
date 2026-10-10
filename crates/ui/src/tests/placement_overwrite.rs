use super::*;

fn move_fixture() -> (DawUi, Id, Id, Clip, Clip) {
    let (mut app, target) = fixture();
    let source = app.session.project.add_track().unwrap();
    let neighbor = app.session.project.tracks[0].clips[0].clone();
    let moving = Clip {
        id: Id::new_v4(),
        name: "Moved".into(),
        start_frame: frames(1.0),
        source_offset_frame: frames(1.0),
        length_frames: frames(1.0),
        ..neighbor.clone()
    };
    app.session.project.tracks[1].clips.push(moving.clone());
    (app, target, source, neighbor, moving)
}

fn begin_move(
    app: &mut DawUi,
    ctx: &egui::Context,
    moving: &Clip,
    source: Id,
    target: Id,
    delta: f32,
    shift: bool,
) -> (Pos2, Vec<egui::epaint::ClippedShape>) {
    frame(app, ctx, vec![]);
    let block = app.clip_block(app.lane_bounds[&source], moving, moving.start_frame);
    let start = Pos2::new(block.left() + 17.0, block.top() + 12.0);
    let end = Pos2::new(
        start.x + delta * app.zoom,
        app.lane_bounds[&target].top() + 12.0,
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    let shapes = frame_shapes(
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
    );
    (end, shapes)
}

#[test]
fn moving_into_a_clip_previews_both_survivors_and_commits_only_on_release() {
    for shift in [false, true] {
        let (mut app, target, source, neighbor, moving) = move_fixture();
        let ctx = context();
        select_group(&mut app, &ctx, [moving.id]);
        let original = format!("{:?}", app.session.project);
        let (end, shapes) = begin_move(&mut app, &ctx, &moving, source, target, 3.012, shift);
        let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, shift);
        assert_eq!(previews.len(), 3);
        assert!(previews.iter().all(|preview| preview.valid));
        let placed = &previews[0].clip;
        assert_eq!(placed.start_frame, frames(if shift { 4.012 } else { 4.0 }));
        let pieces: Vec<_> = previews
            .iter()
            .filter(|preview| preview.clip.id != moving.id)
            .collect();
        assert_eq!(pieces[0].clip.id, neighbor.id);
        assert_eq!(pieces[0].clip.end(), placed.start_frame);
        assert_eq!(pieces[1].clip.start_frame, placed.end());
        for preview in &previews {
            assert!(has_preview_outline(
                &shapes,
                app.clip_block(
                    app.lane_bounds[&target],
                    &preview.clip,
                    preview.clip.start_frame
                ),
                theme::ACCENT
            ));
        }
        assert_eq!(format!("{:?}", app.session.project), original);
        assert!(!app.dirty);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: end,
                pressed: false,
                button: egui::PointerButton::Primary,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            }],
        );
        assert!(app.error.is_none());
        assert!(app.dirty);
        assert!(app.session.project.tracks[1].clips.is_empty());
        let clips = &app.session.project.tracks[0].clips;
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].id, neighbor.id);
        assert_eq!(clips[1].id, moving.id);
        assert_ne!(clips[2].id, neighbor.id);
        for (result, preview) in clips.iter().zip(&[
            pieces[0].clip.clone(),
            placed.clone(),
            pieces[1].clip.clone(),
        ]) {
            assert_eq!(
                (
                    result.start_frame,
                    result.length_frames,
                    result.source_offset_frame
                ),
                (
                    preview.start_frame,
                    preview.length_frames,
                    preview.source_offset_frame
                )
            );
        }
        assert_eq!(app.selected_clips, HashSet::from([moving.id]));
        app.session.project.validate().unwrap();
    }
}

#[test]
fn group_move_keeps_gaps_and_restores_neighbor_previews_when_moved_back_or_cancelled() {
    for action in ["release", "escape", "focus"] {
        let (mut app, target, source, neighbor, moving) = move_fixture();
        let second = Clip {
            id: Id::new_v4(),
            start_frame: frames(3.0),
            length_frames: frames(0.5),
            ..moving.clone()
        };
        app.session.project.tracks[1].clips.push(second.clone());
        let ctx = context();
        select_group(&mut app, &ctx, [moving.id, second.id]);
        let before = format!("{:?}", app.session.project);
        let (end, _) = begin_move(&mut app, &ctx, &moving, source, target, 1.0, true);
        let drag = app.drag.as_ref().unwrap();
        let origin = drag.origin;
        let previews = app.clip_drag_previews(drag, end, true);
        assert_eq!(previews.len(), 5);
        assert!(previews.iter().all(|preview| preview.valid));
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(origin)]);
        let restored = app.clip_drag_previews(app.drag.as_ref().unwrap(), origin, true);
        assert_eq!(restored.len(), 2);
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        if action == "escape" {
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
        } else if action == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    events: vec![egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        if action == "release" {
            assert_eq!(app.session.project.tracks[0].clips.len(), 5);
            assert_eq!(app.session.project.tracks[0].clips[0].id, neighbor.id);
            assert_eq!(app.selected_clips, HashSet::from([moving.id, second.id]));
            assert!(app.dirty);
        } else {
            assert_eq!(format!("{:?}", app.session.project), before);
            assert!(!app.dirty);
        }
        assert!(app.error.is_none());
    }
}

#[test]
fn file_drop_previews_middle_split_and_publishes_all_ranges_after_import_succeeds() {
    let source = TestWav::new();
    let (mut app, track) = fixture();
    let neighbor = app.session.project.tracks[0].clips[0].clone();
    let before = format!("{:?}", app.session.project);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let point = lane.min + Vec2::new(4.012 * app.zoom, 12.0);
    let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
    let target = app.file_drop_target.as_ref().unwrap();
    assert_eq!(target.start, frames(4.0));
    let mut left = neighbor.clone();
    left.length_frames = target.start;
    let mut right = neighbor.clone();
    right.start_frame = target.start + frames(1.0);
    right.length_frames = neighbor.end() - right.start_frame;
    for piece in [&left, &right] {
        assert!(has_preview_outline(
            &shapes,
            app.clip_block(lane, piece, piece.start_frame),
            theme::ACCENT
        ));
    }
    assert_eq!(format!("{:?}", app.session.project), before);
    let prepared_samples = app
        .file_hover
        .as_ref()
        .unwrap()
        .audio
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
        .samples
        .clone();
    file_frame_with_shift(&mut app, &ctx, &source.0, point, false, true);
    let start = app.file_drop_target.as_ref().unwrap().start;
    assert_eq!(start, frames(4.012));
    assert_eq!(format!("{:?}", app.session.project), before);
    file_frame_with_shift(&mut app, &ctx, &source.0, point, true, true);
    assert!(app.job.is_some());
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
    finish_import(&mut app, &ctx);
    assert!(app.error.is_none());
    let clips = &app.session.project.tracks[0].clips;
    assert_eq!(clips.len(), 3);
    assert_eq!(clips[0].id, neighbor.id);
    assert_eq!(clips[0].end(), start);
    assert_eq!(clips[1].start_frame, start);
    assert_eq!(clips[2].start_frame, start + frames(1.0));
    assert_eq!(clips[2].source_offset_frame, clips[2].start_frame);
    assert!(std::sync::Arc::ptr_eq(
        &prepared_samples,
        &app.session.audio[&clips[1].asset_id].samples
    ));
    assert!(app.dirty);
}

#[test]
fn file_drop_full_coverage_removes_neighbor_and_failed_import_keeps_all_ranges() {
    for fail in [false, true] {
        let source = TestWav::new();
        let (mut app, track) = fixture();
        app.session.project.tracks[0].clips[0].length_frames = frames(0.5);
        let neighbor = app.session.project.tracks[0].clips[0].clone();
        let before = format!("{:?}", app.session.project);
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let point = app.lane_bounds[&track].min + Vec2::new(0.0, 12.0);
        ready_file_hover(&mut app, &ctx, &source.0, point);
        if fail {
            app.file_hover.as_mut().unwrap().audio =
                Some(Err("Cannot prepare dropped audio".into()));
        }
        file_frame(&mut app, &ctx, &source.0, point, true);
        assert_eq!(format!("{:?}", app.session.project), before);
        finish_import(&mut app, &ctx);
        if fail {
            assert!(
                app.error
                    .as_ref()
                    .unwrap()
                    .contains("Cannot prepare dropped audio")
            );
            assert_eq!(format!("{:?}", app.session.project), before);
            assert!(!app.dirty);
        } else {
            let clips = &app.session.project.tracks[0].clips;
            assert_eq!(clips.len(), 1);
            assert_ne!(clips[0].id, neighbor.id);
            assert_eq!(clips[0].length_frames, frames(1.0));
            assert!(app.error.is_none());
            assert!(app.dirty);
        }
    }
}
