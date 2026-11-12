use super::*;

#[test]
fn cached_markers_paint_only_in_lower_editor_without_project_edits() {
    for appearance in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, _, id) = super::audio_editor::audio_fixture();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: appearance,
                ..Default::default()
            },
        );
        let clip = app.session.project.tracks[0].clips[0].clone();
        let audio = app.session.audio_for_clip(&clip).unwrap();
        app.editor_transients.seed(
            app.session.project.project_id,
            audio,
            vec![clip.source_offset_frame + 600],
        );
        let before = app.session.project.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let color = theme::DARK.grid.gamma_multiply(0.45);
        assert!(color.a() > theme::DARK.grid.gamma_multiply(0.1).a());
        let lines: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape.shape {
                egui::Shape::LineSegment { points, stroke } if stroke.color == color => {
                    Some(points)
                }
                _ => None,
            })
            .collect();
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|points| points[0].x == points[1].x
            && points[0].y > app.midi_editor.track_bounds.bottom()));
        let colors = theme::clip_colors(app.session.project.tracks[0].color, None);
        let block = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == colors.body
                        && rect.rect.top() > app.midi_editor.track_bounds.bottom() =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("lower clip body");
        assert!(lines.iter().all(|line| {
            (line[0].y - (block.top() + crate::clip_paint::HEADER_HEIGHT)).abs() < 0.01
                && (line[1].y - (block.bottom() - 0.5)).abs() < 0.01
        }));
        assert_eq!(app.selected_clip, Some(id));
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        app.editor_transients.enabled = false;
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(!shapes.iter().any(|shape| matches!(shape.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == color)));
    }
}

#[test]
fn transient_toggle_does_not_edit_the_project() {
    let (mut app, ctx, _, _) = super::audio_editor::audio_fixture();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let point = app.editor_transients.toggle_bounds.unwrap().center();
    assert!(!app.editor_transients.enabled);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(app.editor_transients.enabled);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    frame(&mut app, &ctx, vec![button(point, true)]);
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(!app.editor_transients.enabled);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}
