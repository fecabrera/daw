use super::*;

#[test]
fn fade_handles_preview_commit_cancel_and_shape_without_moving_clips() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let clip = app.session.project.tracks[0].clips[0].clone();
    let lane = app.lane_bounds[&track];
    let block = app.clip_block(lane, &clip, clip.start_frame);
    let origin = Pos2::new(block.left() + 1.0, block.top() + 26.0);
    let target = origin + Vec2::new(40.0, 0.0);
    let before = app.session.project.clone();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    assert!(matches!(
        app.drag.as_ref().map(|d| d.mode),
        Some(ClipDragMode::Fade {
            left: true,
            shape: false,
            ..
        })
    ));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
    assert_eq!(app.session.project, before, "drag must remain a preview");
    frame(&mut app, &ctx, vec![button(target, false)]);
    let fade = app.session.project.tracks[0].clips[0].fades.fade_in;
    assert!(fade.length_frames > 0 && !fade.crossfade);
    assert_eq!(
        app.session.project.tracks[0].clips[0].start_frame,
        clip.start_frame
    );
    assert_eq!(
        app.session.project.tracks[0].clips[0].length_frames,
        clip.length_frames
    );
    let width = app.session.project.seconds(fade.length_frames) as f32 * app.view_zoom();
    let shape = Pos2::new(
        block.left() + width * 0.5,
        block.bottom() - 0.5 * (block.height() - crate::clip_fades::HANDLE_TOP),
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(shape), button(shape, true)],
    );
    assert!(matches!(
        app.drag.as_ref().map(|d| d.mode),
        Some(ClipDragMode::Fade { shape: true, .. })
    ));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(shape + Vec2::new(8.0, -4.0))],
    );
    frame(
        &mut app,
        &ctx,
        vec![button(shape + Vec2::new(8.0, -4.0), false)],
    );
    assert!(
        app.session.project.tracks[0].clips[0]
            .fades
            .fade_in
            .midpoint
            > 0.5
    );
    let shaped = app.session.project.tracks[0].clips[0].fades.fade_in;
    assert!(shaped.midpoint_position > 0.5);
    assert_eq!(shaped.length_frames, fade.length_frames);
    assert!((shaped.gain(shaped.midpoint_position) - shaped.midpoint).abs() < 0.00001);
    let committed = app.session.project.clone();
    let endpoint = Pos2::new(block.left() + width, origin.y);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(endpoint), button(endpoint, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(endpoint + Vec2::new(20.0, 0.0))],
    );
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
    frame(&mut app, &ctx, vec![button(endpoint, false)]);
    assert_eq!(app.session.project, committed);
}

#[test]
fn outward_handle_creates_crossfade_without_overlapping_clip_ranges() {
    let (mut app, track) = fixture();
    let ctx = context();
    let mut first = app.session.project.tracks[0].clips[0].clone();
    first.length_frames = 96_000;
    let mut second = first.clone();
    second.id = Id::new_v4();
    second.start_frame = first.end();
    second.source_offset_frame = 96_000;
    app.session.project.tracks[0].clips = vec![first.clone(), second.clone()];
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let block = app.clip_block(app.lane_bounds[&track], &first, 0);
    let origin = Pos2::new(block.right() - 1.0, block.top() + 26.0);
    let target = origin + Vec2::new(20.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
    assert!(
        !app.session.project.tracks[0].clips[0]
            .fades
            .fade_out
            .crossfade
    );
    frame(&mut app, &ctx, vec![button(target, false)]);
    let fade = app.session.project.tracks[0].clips[0].fades.fade_out;
    assert!(fade.crossfade && fade.length_frames > 0);
    assert_eq!(
        app.session.project.tracks[0].clips[0].end(),
        second.start_frame
    );
    assert_eq!(
        app.session.project.tracks[0].clips[1].start_frame,
        second.start_frame
    );
    app.session.project.validate().unwrap();
}

#[test]
fn clip_settings_shape_edits_the_saved_envelope_and_painting_is_read_only() {
    let (mut app, track) = fixture();
    let ctx = context();
    let id = app.session.project.tracks[0].clips[0].id;
    app.session.project.tracks[0].clips[0]
        .fades
        .fade_in
        .length_frames = 48_000;
    app.select_clip(id, track, false);
    app.clip_settings_visible = true;
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert_eq!(app.session.project, before);
    for (before, after) in [("Transpose", "Fade in"), ("Position", "Fade out")] {
        let text_rect = |name: &str| {
            shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == name => {
                        Some(Rect::from_min_size(t.pos, t.galley.size()))
                    }
                    _ => None,
                })
                .unwrap()
        };
        let previous = text_rect(before);
        let next = text_rect(after);
        let line = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::LineSegment { points, .. }
                    if points[0].y == points[1].y
                        && points[0].y > previous.bottom()
                        && points[0].y < next.top() =>
                {
                    Some(points[0].y)
                }
                _ => None,
            })
            .expect("section divider");
        assert!(
            ((line - previous.bottom()) - (next.top() - line)).abs() <= 1.5,
            "divider spacing must match above and below {after}"
        );
    }
    let label = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Shape" => Some(t.pos),
            _ => None,
        })
        .expect("Shape label");
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.pos.x > label.x && (t.pos.y - label.y).abs() < 4.0 => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Shape value in the right column");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point + Vec2::new(0.0, -2000.0))],
    );
    frame(
        &mut app,
        &ctx,
        vec![button(point + Vec2::new(0.0, -2000.0), false)],
    );
    assert_eq!(
        app.session.project.tracks[0].clips[0]
            .fades
            .fade_in
            .midpoint,
        daw_core::ClipFade::MAX_LEVEL
    );
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let label = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Position" => Some(t.pos),
            _ => None,
        })
        .expect("Position label");
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.pos.x > label.x && (t.pos.y - label.y).abs() < 4.0 => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Position value in the right column");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point + Vec2::new(0.0, -2000.0))],
    );
    frame(
        &mut app,
        &ctx,
        vec![button(point + Vec2::new(0.0, -2000.0), false)],
    );
    let fade = app.session.project.tracks[0].clips[0].fades.fade_in;
    assert_eq!(fade.midpoint_position, daw_core::ClipFade::MAX_POSITION);
    assert!((fade.gain(fade.midpoint_position) - fade.midpoint).abs() < 0.00001);
    // The plain value keeps Gain's double-click text editing behavior.
    for _ in 0..2 {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
    }
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![egui::Event::Text("25".into())]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(
        app.session.project.tracks[0].clips[0]
            .fades
            .fade_in
            .midpoint_position,
        0.25
    );
}

#[test]
fn lower_editor_fades_keep_their_drag_clock_when_clip_options_read_the_preview() {
    let (mut app, ctx, _, id) = super::audio_editor::audio_fixture();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let color = theme::clip_colors(app.session.project.tracks[0].color, None).body;
    let block = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill == color
                    && rect.rect.top() >= app.midi_editor.track_bounds.bottom() =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .unwrap();
    let origin = Pos2::new(block.left() + 1.0, block.top() + 26.0);
    let target = origin + Vec2::new(40.0, 0.0);
    let expected = (40.0 / app.midi_editor.content.zoom * app.session.project.sample_rate_hz as f32)
        .round() as u64;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
    assert!(app.content_clip_drag);
    let drag = app.drag.as_ref().unwrap();
    assert_eq!(
        app.fade_drag_value(drag, &ctx).fade_in.length_frames,
        expected
    );
    frame(&mut app, &ctx, vec![button(target, false)]);
    let clip = app.session.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap();
    assert_eq!(clip.fades.fade_in.length_frames, expected);
}

#[test]
fn inspector_painting_preserves_boundary_control_points_and_inactive_crossfades() {
    for crossfade in [false, true] {
        for position in [0.15, 0.85] {
            for level in [0.125, 0.875] {
                let (mut app, track) = fixture();
                let ctx = context();
                let clip = &mut app.session.project.tracks[0].clips[0];
                clip.fades.fade_in = daw_core::ClipFade {
                    length_frames: 1000,
                    midpoint: level,
                    midpoint_position: position,
                    crossfade,
                };
                let id = clip.id;
                app.select_clip(id, track, false);
                app.clip_settings_visible = true;
                let before = app.session.project.clone();
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                assert_eq!(
                    app.session.project, before,
                    "opening settings must not commit clamped display values"
                );
            }
        }
    }
}

#[test]
fn crossfade_shading_is_applied_once_regardless_of_saved_clip_order() {
    for left in [false, true] {
        for reverse_order in [false, true] {
            let (mut app, track) = fixture();
            let ctx = context();
            let mut first = app.session.project.tracks[0].clips[0].clone();
            first.length_frames = 96_000;
            let mut second = first.clone();
            second.id = Id::new_v4();
            second.start_frame = first.end();
            second.source_offset_frame = 192_000;
            let fade = daw_core::ClipFade {
                length_frames: 24_000,
                midpoint: 0.7,
                midpoint_position: 0.35,
                crossfade: true,
            };
            let owner = if left {
                second.fades.fade_in = fade;
                second.clone()
            } else {
                first.fades.fade_out = fade;
                first.clone()
            };
            let asset = &app.session.project.assets[0];
            app.session.audio.insert(
                asset.id,
                daw_media::AudioData {
                    disk: None,
                    sample_rate_hz: app.session.project.sample_rate_hz,
                    samples: std::sync::Arc::new(
                        (0..asset.decoded_frame_count)
                            .map(|frame| {
                                let value = if frame < 96_000 {
                                    0.2
                                } else if frame < 192_000 {
                                    0.8
                                } else {
                                    0.4
                                };
                                [value, -value]
                            })
                            .collect(),
                    ),
                    peaks: std::sync::Arc::new(
                        (0..asset.decoded_frame_count.div_ceil(256))
                            .map(|block| {
                                let value = if block * 256 < 96_000 {
                                    0.2
                                } else if block * 256 < 192_000 {
                                    0.8
                                } else {
                                    0.4
                                };
                                [[0.0, value], [-value, 0.0]]
                            })
                            .collect(),
                    ),
                    metadata: asset.source_metadata.clone(),
                },
            );
            app.session.project.tracks[0].clips = if reverse_order {
                vec![second, first]
            } else {
                vec![first, second]
            };
            app.select_clip(owner.id, track, false);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let block = app.clip_block(app.lane_bounds[&track], &owner, owner.start_frame);
            assert!(
                !shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.stroke.color == theme::palette(&ctx).divider
                    && rect.rect.height() >= 22.0 && rect.rect.top() >= block.top()
                    && rect.rect.bottom() <= block.bottom() + 0.1)),
                "audio crossfade headers and bodies must not have neutral border rectangles"
            );
            let width = app.session.project.seconds(fade.length_frames) as f32 * app.view_zoom();
            let probe = Pos2::new(
                if left {
                    block.left() - width * 0.5
                } else {
                    block.right() + width * 0.5
                },
                block.top() + crate::clip_fades::HANDLE_CENTER + 1.0,
            );
            let grey = crate::clip_fades::FADE_SHADE;
            let layers = shapes
                .iter()
                .filter(|shape| {
                    shape.clip_rect.contains(probe)
                        && matches!(&shape.shape, egui::Shape::Mesh(mesh)
                    if !mesh.vertices.is_empty()
                        && mesh.vertices.iter().all(|v| v.color == grey)
                        && mesh.calc_bounds().contains(probe))
                })
                .count();
            assert_eq!(
                layers, 1,
                "crossfade must not receive duplicate background fill"
            );
            let header = app.clip_fade_header_rect(block, &owner, &ctx);
            let visual_outline =
                Rect::from_min_max(header.min, Pos2::new(header.right(), block.bottom()));
            let outline_probe = Pos2::new(probe.x, block.top() + 1.0);
            assert!(shapes.iter().any(|shape| shape.clip_rect.contains(outline_probe)
                && matches!(&shape.shape, egui::Shape::Rect(rect)
                    if rect.rect == visual_outline && rect.stroke == theme::selection_outline(&ctx))),
                "the owner selection outline must span the crossover in the neighbor's paint pass");
            if left {
                assert_eq!(header.left(), block.left() - width);
            } else {
                assert_eq!(header.right(), block.right() + width);
            }
            let shade_index = shapes
                .iter()
                .position(|shape| {
                    shape.clip_rect.contains(probe)
                        && matches!(&shape.shape,
                    egui::Shape::Mesh(mesh) if mesh.vertices.iter().all(|v| v.color == grey)
                        && mesh.calc_bounds().contains(probe))
                })
                .unwrap();
            let egui::Shape::Mesh(shade_mesh) = &shapes[shade_index].shape else {
                unreachable!()
            };
            assert_eq!(
                shade_mesh.calc_bounds().top(),
                block.top() + crate::clip_paint::HEADER_HEIGHT,
                "crossover shading must meet the header without a bright strip"
            );
            let waveform_color =
                theme::clip_colors(app.session.project.tracks[0].color, None).waveform;
            let waveform_index = shapes
                .iter()
                .enumerate()
                .find_map(|(index, shape)| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == waveform_color
                            && (points[0].x - probe.x).abs() < 2.0
                            && shape.clip_rect.contains(points[0]) =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
                .expect("neighbor waveform remains visible in its original color");
            assert!(
                shade_index < waveform_index,
                "shade must be behind the waveform"
            );
            let pixel = probe.x.floor();
            let heights: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == waveform_color
                            && points[0].x == pixel
                            && points[1].x == pixel
                            && shape.clip_rect.contains(points[0]) =>
                    {
                        Some((points[1].y - points[0].y).abs())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                heights.len(),
                4,
                "both stereo source waveforms must extend through the overlap"
            );
            let scale = (block.height() - crate::clip_fades::WAVEFORM_TOP) * 0.5 * 0.42;
            let frames_per_pixel =
                f64::from(app.session.project.sample_rate_hz) / f64::from(app.view_zoom());
            let extended_left = if left {
                block.left() - width
            } else {
                block.left()
            };
            let local =
                ((f64::from(pixel) - f64::from(extended_left)) * frames_per_pixel).max(0.0) as u64;
            let position = if left {
                local
            } else {
                owner.length_frames + fade.length_frames - 1 - local
            };
            let weight = fade.gain(position as f32 / (fade.length_frames - 1) as f32);
            for amplitude in [
                0.8 * weight,
                (if left { 0.2 } else { 0.4 }) * (1.0 - weight),
            ] {
                assert!(
                    heights
                        .iter()
                        .any(|height| (height - amplitude * scale).abs() < 0.01),
                    "both source waveforms must follow their complementary transition gains"
                );
            }
            let guide_count = |shapes: &[egui::epaint::ClippedShape]| {
                shapes.iter().filter(|shape|
                shape.clip_rect.contains(probe) && matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == egui::Color32::from_rgb(32, 32, 32)
                            && stroke.width == 1.0 && points[0].x == points[1].x
                            && (points[0].y - block.top() - crate::clip_paint::HEADER_HEIGHT).abs() < 0.001
                            && (points[1].y - block.bottom()).abs() < 0.001)
            ).count()
            };
            assert_eq!(
                guide_count(&shapes),
                0,
                "crossover guides stay hidden without hover"
            );
            let curve_count = |shapes: &[egui::epaint::ClippedShape]| {
                shapes
                    .iter()
                    .filter(|shape| {
                        shape.clip_rect.contains(probe)
                            && matches!(&shape.shape,
                    egui::Shape::Path(path) if path.points.len() == 33 && path.stroke.width == 1.0)
                    })
                    .count()
            };
            assert_eq!(
                curve_count(&shapes),
                0,
                "crossfade curves stay hidden without hover"
            );
            let hovered = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(probe)],
                Vec2::new(1280.0, 800.0),
            );
            assert_eq!(
                curve_count(&hovered),
                2,
                "hover reveals both crossover curves"
            );
            assert_eq!(
                guide_count(&hovered),
                2,
                "hover shows both full-height crossover guides"
            );
            let unhovered = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerGone],
                Vec2::new(1280.0, 800.0),
            );
            assert_eq!(
                curve_count(&unhovered),
                0,
                "leaving the crossover hides the curves again"
            );
            assert_eq!(
                guide_count(&unhovered),
                0,
                "leaving the crossover hides both guides"
            );
            let receiver = app.session.project.tracks[0]
                .clips
                .iter()
                .find(|clip| clip.id != owner.id)
                .unwrap()
                .id;
            app.select_clip(receiver, track, false);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(probe), button(probe, true)],
            );
            frame(&mut app, &ctx, vec![button(probe, false)]);
            assert_eq!(
                app.selected_clip,
                Some(owner.id),
                "clicking the crossover selects its visual owner"
            );
        }
    }
}

#[test]
fn midpoint_drags_stop_at_position_and_shape_limits_in_both_directions_and_modes() {
    for left in [false, true] {
        for crossfade in [false, true] {
            for delta in [-1000.0, 1000.0] {
                let (mut app, track) = fixture();
                let ctx = context();
                let mut first = app.session.project.tracks[0].clips[0].clone();
                first.length_frames = 96_000;
                let mut second = first.clone();
                second.id = Id::new_v4();
                second.start_frame = first.end();
                second.source_offset_frame = first.length_frames;
                let fade = daw_core::ClipFade {
                    length_frames: 24_000,
                    crossfade,
                    ..Default::default()
                };
                let owner = if left {
                    second.fades.fade_in = fade;
                    second.clone()
                } else {
                    first.fades.fade_out = fade;
                    first.clone()
                };
                app.session.project.tracks[0].clips = vec![first, second];
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let block = app.clip_block(app.lane_bounds[&track], &owner, owner.start_frame);
                let width =
                    app.session.project.seconds(fade.length_frames) as f32 * app.view_zoom();
                let anchor = if left { block.left() } else { block.right() };
                let direction = if left { 1.0 } else { -1.0 } * if crossfade { -1.0 } else { 1.0 };
                let origin = Pos2::new(
                    anchor + direction * width * 0.5,
                    block.bottom() - 0.5 * (block.height() - crate::clip_fades::HANDLE_TOP),
                );
                let target = origin + Vec2::splat(delta);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(origin), button(origin, true)],
                );
                assert!(matches!(
                    app.drag.as_ref().map(|d| d.mode),
                    Some(ClipDragMode::Fade { shape: true, .. })
                ));
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
                frame(&mut app, &ctx, vec![button(target, false)]);
                let clip = app.session.project.tracks[0]
                    .clips
                    .iter()
                    .find(|c| c.id == owner.id)
                    .unwrap();
                let result = if left {
                    clip.fades.fade_in
                } else {
                    clip.fades.fade_out
                };
                assert!(
                    result.midpoint_position == daw_core::ClipFade::MIN_POSITION
                        || result.midpoint_position == daw_core::ClipFade::MAX_POSITION
                );
                assert!(
                    result.midpoint == daw_core::ClipFade::MIN_LEVEL
                        || result.midpoint == daw_core::ClipFade::MAX_LEVEL
                );
                assert_eq!(result.length_frames, fade.length_frames);
                assert!(result.valid());
            }
        }
    }
}

#[test]
fn fade_controls_are_hover_only_and_whole_at_shared_boundaries() {
    let (mut app, track) = fixture();
    let ctx = context();
    let mut first = app.session.project.tracks[0].clips[0].clone();
    first.length_frames = 96_000;
    first.fades.fade_out = daw_core::ClipFade {
        length_frames: 24_000,
        crossfade: true,
        ..Default::default()
    };
    let mut second = first.clone();
    second.id = Id::new_v4();
    second.start_frame = first.end();
    second.source_offset_frame = 192_000;
    second.fades = Default::default();
    app.session.project.tracks[0].clips = vec![first.clone(), second.clone()];
    frame(&mut app, &ctx, vec![]);
    let markers = |shapes: &[egui::epaint::ClippedShape]| {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.rect.size() == Vec2::splat(6.0) => {
                    Some((shape.clip_rect, rect.rect))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let absent = frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerGone],
        Vec2::new(1280.0, 800.0),
    );
    assert!(markers(&absent).is_empty());
    let block = app.clip_block(app.lane_bounds[&track], &second, second.start_frame);
    let hovered = frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(block.center())],
        Vec2::new(1280.0, 800.0),
    );
    let handles = markers(&hovered);
    let at_boundary: Vec<_> = handles
        .iter()
        .filter(|(_, rect)| (rect.center().x - block.left()).abs() < 0.001)
        .collect();
    assert!(!at_boundary.is_empty());
    assert!(
        at_boundary
            .iter()
            .all(|(clip, rect)| clip.contains_rect(*rect))
    );
    let absent = frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerGone],
        Vec2::new(1280.0, 800.0),
    );
    assert!(markers(&absent).is_empty());
}

#[test]
fn waveform_fade_envelope_tracks_both_edges_and_shape() {
    let (mut app, _) = fixture();
    let ctx = context();
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.fades.fade_in = daw_core::ClipFade {
        length_frames: 1000,
        midpoint: 0.8,
        midpoint_position: 0.25,
        crossfade: false,
    };
    clip.fades.fade_out = clip.fades.fade_in;
    let clip = clip.clone();
    let gain = app.waveform_fade_gain(&clip, &ctx);
    assert_eq!(gain(0), 0.0);
    assert!((gain(250) - 0.8).abs() < 0.00001);
    assert_eq!(gain(1000), 1.0);
    assert!((gain(clip.length_frames - 251) - 0.8).abs() < 0.00001);
    assert_eq!(gain(clip.length_frames - 1), 0.0);
}
