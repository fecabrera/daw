use super::*;

fn boundary_fixture() -> (DawUi, Id, [Clip; 2]) {
    let (mut app, track) = fixture();
    let left = &mut app.session.project.tracks[0].clips[0];
    left.start_frame = frames(2.0);
    left.length_frames = frames(1.0);
    left.source_offset_frame = frames(1.0);
    let left = left.clone();
    let right = Clip {
        id: Id::new_v4(),
        start_frame: left.end(),
        source_offset_frame: frames(3.0),
        ..left.clone()
    };
    app.session.project.tracks[0].clips.push(right.clone());
    app.session.project.validate().unwrap();
    (app, track, [left, right])
}

fn begin_boundary(
    app: &mut DawUi,
    ctx: &egui::Context,
    track: Id,
    clip: &Clip,
    left_edge: bool,
    delta: f32,
    shift: bool,
) -> (Pos2, Pos2, Vec<egui::epaint::ClippedShape>) {
    let block = app.clip_block(app.lane_bounds[&track], clip, clip.start_frame);
    let start = Pos2::new(
        if left_edge {
            block.left() + 0.2
        } else {
            block.right() - 0.2
        },
        block.top() + 40.0,
    );
    let end = start + Vec2::new(delta * app.zoom, 0.0);
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
    (start, end, shapes)
}

fn release_boundary(app: &mut DawUi, ctx: &egui::Context, end: Pos2, shift: bool) {
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
fn either_waveform_edge_previews_the_pair_and_commits_the_same_snapped_boundary() {
    for left_edge in [false, true] {
        for shift in [false, true] {
            for sign in [-1.0, 1.0] {
                let (mut app, track, clips) = boundary_fixture();
                let ctx = context();
                select_group(&mut app, &ctx, clips.clone().map(|clip| clip.id));
                let original = format!("{:?}", app.session.project);
                let primary = &clips[usize::from(left_edge)];
                let (_, end, shapes) = begin_boundary(
                    &mut app,
                    &ctx,
                    track,
                    primary,
                    left_edge,
                    sign * 0.237,
                    shift,
                );
                let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, shift);
                assert_eq!(previews.len(), 2);
                assert!(
                    previews
                        .iter()
                        .all(|preview| preview.valid && !preview.removed)
                );
                let boundary = frames(3.0 + f64::from(sign) * if shift { 0.237 } else { 0.25 });
                assert_eq!(previews[0].clip.end(), boundary);
                assert_eq!(previews[1].clip.start_frame, boundary);
                assert_eq!(previews[0].clip.start_frame, clips[0].start_frame);
                assert_eq!(previews[1].clip.end(), clips[1].end());
                for preview in &previews {
                    assert!(has_preview_outline(
                        &shapes,
                        app.clip_block(
                            app.lane_bounds[&track],
                            &preview.clip,
                            preview.clip.start_frame
                        ),
                        theme::ACCENT
                    ));
                }
                assert_eq!(format!("{:?}", app.session.project), original);
                assert!(!app.dirty);
                release_boundary(&mut app, &ctx, end, shift);
                assert!(app.error.is_none());
                assert!(app.dirty);
                for preview in previews {
                    let placed = app.session.project.tracks[0]
                        .clips
                        .iter()
                        .find(|clip| clip.id == preview.clip.id)
                        .unwrap();
                    assert_eq!(format!("{placed:?}"), format!("{:?}", preview.clip));
                }
            }
        }
    }
}

#[test]
fn shared_boundary_clamps_both_source_limits_and_removes_either_covered_clip_on_release() {
    for direction in [-1.0, 1.0] {
        for limited in [false, true] {
            let (mut app, track, _) = boundary_fixture();
            if limited {
                app.session.project.tracks[0].clips[0].source_offset_frame = frames(8.8);
                app.session.project.tracks[0].clips[1].source_offset_frame = frames(0.2);
            }
            let clips = app.session.project.tracks[0].clips.clone();
            let ctx = context();
            select_group(&mut app, &ctx, [clips[0].id, clips[1].id]);
            let before = format!("{:?}", app.session.project);
            let (start, end, _) = begin_boundary(
                &mut app,
                &ctx,
                track,
                &clips[0],
                false,
                direction * 2.0,
                true,
            );
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
            if limited {
                assert!(previews.iter().all(|preview| !preview.removed));
                assert_eq!(
                    previews[0].clip.end(),
                    frames(3.0 + f64::from(direction) * 0.2)
                );
            } else {
                let covered = clips[usize::from(direction > 0.0)].id;
                assert!(
                    previews
                        .iter()
                        .find(|preview| preview.clip.id == covered)
                        .unwrap()
                        .removed
                );
            }
            assert_eq!(format!("{:?}", app.session.project), before);
            // Returning to the press point restores both clips before release.
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
            assert!(
                app.clip_drag_previews(app.drag.as_ref().unwrap(), start, true)
                    .iter()
                    .all(|preview| !preview.removed)
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            release_boundary(&mut app, &ctx, end, true);
            assert!(app.error.is_none());
            assert_eq!(
                app.session.project.tracks[0].clips.len(),
                if limited { 2 } else { 1 }
            );
            assert_eq!(app.selected_clips.len(), if limited { 2 } else { 1 });
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn shared_boundary_cancels_on_escape_or_focus_loss_and_clamped_releases_stay_clean() {
    for case in 0..3 {
        let (mut app, track, clips) = boundary_fixture();
        if case == 2 {
            app.session.project.tracks[0].clips[1].source_offset_frame = 0;
        }
        let ctx = context();
        select_group(&mut app, &ctx, [clips[0].id]);
        let before = format!("{:?}", app.session.project);
        let (_, end, _) = begin_boundary(&mut app, &ctx, track, &clips[0], false, -0.5, true);
        if case == 0 {
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
        } else if case == 1 {
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
        release_boundary(&mut app, &ctx, end, true);
        assert!(app.drag.is_none());
        assert_eq!(format!("{:?}", app.session.project), before);
        assert!(!app.dirty);
        assert!(app.error.is_none());
    }
}

#[test]
fn header_looping_and_modifier_stretching_do_not_move_the_neighbor_on_shortening() {
    let (app, track, clips) = boundary_fixture();
    for mode in [ClipDragMode::LoopRight, ClipDragMode::StretchRight] {
        let mut clip = clips[0].clone();
        if mode == ClipDragMode::LoopRight {
            clip.repeat = Some(daw_core::ClipLoop {
                length_frames: frames(0.5),
                phase_frame: 0,
            });
        }
        let mut copy = DawUi::default();
        copy.session.project = app.session.project.clone();
        let mut app = copy;
        app.session.project.tracks[0].clips[0] = clip.clone();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let drag = Drag {
            clip: clip.clone(),
            track,
            mode,
            origin: Pos2::ZERO,
            duplicate: false,
            linked_boundary: false,
            clips: vec![SelectedClip {
                clip,
                track,
                track_index: 0,
            }],
        };
        let previews = app.clip_drag_previews(&drag, Pos2::new(-0.25 * app.zoom, 0.0), true);
        assert!(
            previews
                .iter()
                .all(|preview| preview.clip.id != clips[1].id)
        );
        assert!(previews[0].clip.end() < clips[1].start_frame);
    }
}

#[test]
fn trimming_inside_either_clip_keeps_the_original_behavior() {
    for left_edge in [false, true] {
        let (mut app, track, clips) = boundary_fixture();
        let ctx = context();
        let clip = &clips[usize::from(left_edge)];
        select_group(&mut app, &ctx, [clip.id]);
        let block = app.clip_block(app.lane_bounds[&track], clip, clip.start_frame);
        let start = Pos2::new(
            if left_edge {
                block.left() + 5.0
            } else {
                block.right() - 5.0
            },
            block.top() + 40.0,
        );
        let end = start + Vec2::new(if left_edge { 0.25 } else { -0.25 } * app.zoom, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let drag = app.drag.as_ref().unwrap();
        assert!(!drag.linked_boundary);
        let previews = app.clip_drag_previews(drag, end, false);
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].clip.id, clip.id);
        release_boundary(&mut app, &ctx, end, false);
        let neighbor = &clips[usize::from(!left_edge)];
        let retained = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|other| other.id == neighbor.id)
            .unwrap();
        assert_eq!(format!("{retained:?}"), format!("{neighbor:?}"));
        assert!(
            app.session.project.tracks[0].clips[0].end()
                < app.session.project.tracks[0].clips[1].start_frame
        );
    }
}

#[test]
fn linked_pair_and_other_selected_clips_reject_shortening_as_one_transaction() {
    let (mut app, track, clips) = boundary_fixture();
    let other_track = app.session.project.add_track().unwrap();
    let other = Clip {
        id: Id::new_v4(),
        length_frames: frames(0.25),
        ..clips[0].clone()
    };
    app.session.project.tracks[1].clips.push(other.clone());
    let ctx = context();
    select_group(&mut app, &ctx, [clips[0].id, other.id]);
    let before = format!("{:?}", app.session.project);
    let (_, end, _) = begin_boundary(&mut app, &ctx, track, &clips[0], false, -0.5, true);
    let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
    assert!(previews.iter().all(|preview| !preview.valid));
    assert_eq!(format!("{:?}", app.session.project), before);
    release_boundary(&mut app, &ctx, end, true);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert_eq!(app.session.project.tracks[1].id, other_track);
    assert!(!app.dirty);
    assert!(app.error.as_ref().unwrap().contains("remove"));
}

#[test]
fn the_shared_line_works_at_multiple_display_scales_and_uses_a_narrow_hit_area() {
    for scale in [1.0, 2.0] {
        for offset in [-3.5_f32, -2.5, 0.0, 2.5, 3.5] {
            let (mut app, track, clips) = boundary_fixture();
            let ctx = context();
            ctx.set_pixels_per_point(scale);
            frame(&mut app, &ctx, vec![]);
            let block = app.clip_block(app.lane_bounds[&track], &clips[1], clips[1].start_frame);
            let start = Pos2::new(block.left() + offset, block.top() + 40.0);
            let end = start + Vec2::new(if offset < 0.0 { -0.25 } else { 0.25 } * app.zoom, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            let drag = app.drag.as_ref().unwrap();
            assert_eq!(drag.linked_boundary, offset.abs() <= 3.0);
            let previews = app.clip_drag_previews(drag, end, false);
            assert_eq!(previews.len(), if offset.abs() <= 3.0 { 2 } else { 1 });
            release_boundary(&mut app, &ctx, end, false);
            let clips = &app.session.project.tracks[0].clips;
            if offset.abs() <= 3.0 {
                assert_eq!(clips[0].end(), clips[1].start_frame);
            } else {
                assert!(clips[0].end() < clips[1].start_frame);
            }
        }
    }
}
