use super::*;

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn effects_lane_spans_the_window_with_one_status_separator_and_an_aligned_overview() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for size in [
            Vec2::new(640.0, 260.0),
            Vec2::new(640.0, 480.0),
            Vec2::new(1280.0, 800.0),
        ] {
            let (mut app, track) = fixture();
            let clip = app.session.project.tracks[0].clips[0].id;
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_clip(clip, track, false);
            app.set_view_mode(ViewMode::Edit, &ctx);
            frame_sized(&mut app, &ctx, vec![], size);
            let editor_ruler = app.midi_editor.ruler_bounds;
            let editor_lane = app.lane_bounds[&track];
            let original = format!("{:?}", app.session.project);
            app.set_view_mode(ViewMode::Effects, &ctx);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let catalog = app.effects_bounds.library;
            let header = app.effects_bounds.header;
            let rack = app.effects_bounds.strip;
            let scrollbar = app.effects_bounds.scrollbar;
            let ruler = app.midi_editor.ruler_bounds;
            let lane = app.lane_bounds[&track];
            assert_eq!(catalog.width(), TRACK_WIDTH);
            assert!((catalog.top() - header.top()).abs() < 0.1);
            assert!((header.bottom() - ruler.bottom()).abs() < 0.1);
            assert!(
                (catalog.bottom() - rack.top()).abs() < 0.1,
                "size={size:?} catalog={catalog:?} rack={rack:?} scrollbar={scrollbar:?}"
            );
            assert_eq!(rack.left(), catalog.left());
            assert_eq!(rack.width(), size.x);
            assert_eq!(rack.right(), editor_lane.right());
            assert!(rack.top() >= lane.bottom());
            if size.y >= 480.0 {
                assert_eq!(rack.height(), panels::unit_height(false));
                assert!(rack.top() > lane.bottom() + 20.0);
            } else {
                assert!(rack.height() > 0.0 && rack.height() < panels::unit_height(false));
            }
            assert_eq!(ruler.height(), editor_ruler.height());
            assert_eq!(lane.height(), editor_lane.height());
            assert!((ruler.left() - editor_ruler.left() - TRACK_WIDTH).abs() < 0.1);
            assert_eq!(ruler.left(), lane.left());
            assert_eq!(ruler.bottom(), lane.top());
            assert_eq!(scrollbar.left(), ruler.left());
            assert_eq!(scrollbar.right(), ruler.right());
            assert_eq!(scrollbar.height(), SCROLLBAR_HEIGHT);
            assert_eq!(scrollbar.bottom(), rack.top());
            assert!(scrollbar.top() >= lane.bottom());
            if scrollbar.top() > lane.bottom() {
                for tick in app.musical_timeline().ticks(lane.width()) {
                    let x = lane.left() + tick.x;
                    if x < lane.left() || x > lane.right() {
                        continue;
                    }
                    let line = [Pos2::new(x, lane.bottom()), Pos2::new(x, scrollbar.top())];
                    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, .. } if *points == line)));
                }
            }
            let palette = theme::palette(&ctx);
            let line = [
                Pos2::new(rack.left(), rack.top() + 0.5),
                Pos2::new(rack.right(), rack.top() + 0.5),
            ];
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment {points, stroke} if *points == line && *stroke == Stroke::new(1.0, palette.border))));
            let bottom_lines = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if points[0].x == rack.left()
                            && points[1].x == rack.right()
                            && points[0].y == points[1].y
                            && (points[0].y - rack.bottom()).abs() <= 1.0 =>
                    {
                        Some((points, stroke))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                bottom_lines.len(),
                1,
                "one separator between the lane and status bar"
            );
            assert_eq!(*bottom_lines[0].1, Stroke::new(1.0, palette.border));
            assert_eq!(bottom_lines[0].0[0].y, rack.bottom() + 0.5);
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rack && r.fill == palette.background)));
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == catalog && r.fill == palette.panel)));
            let border = [
                Pos2::new(catalog.right() - 0.5, catalog.top()),
                Pos2::new(catalog.right() - 0.5, catalog.bottom()),
            ];
            assert_eq!(shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke } if *points == border && *stroke == Stroke::new(1.0, palette.border))).count(), 1);
            assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Track effects are not available yet." || t.galley.text() == "Oscillator" || t.galley.text() == "Envelope")));
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn effects_overview_follows_the_processing_track_and_clears_missing_selection() {
    let (mut app, audio) = fixture();
    let audio_clip = app.session.project.tracks[0].clips[0].id;
    let midi = app.session.project.add_midi_track().unwrap();
    let midi_clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
    let ctx = context();
    app.select_clip(audio_clip, audio, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    app.select_track(midi, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(audio_clip));
    assert_eq!(app.lane_bounds.len(), 1);
    assert!(app.lane_bounds.contains_key(&midi));
    assert!(app.midi_editor.clip_bounds.contains_key(&midi_clip));
    assert!(!app.midi_editor.clip_bounds.contains_key(&audio_clip));
    app.selected_track = None;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(app.lane_bounds.is_empty());
    assert_eq!(app.timeline_bounds, Rect::NOTHING);
    assert!(app.midi_editor.clip_bounds.is_empty());
    assert!(app.effects_bounds.strip.is_positive());
    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Select a track to configure its effects.")));
    assert!(!app.dirty);
}

#[test]
fn effects_ruler_seek_loop_and_zoom_use_the_overview_viewport() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.zoom = 240.0;
    app.midi_editor.scroll = 0.5;
    let timeline = (app.zoom, app.scroll);
    frame(&mut app, &ctx, vec![]);
    let ruler = app.midi_editor.ruler_bounds;
    click(
        &mut app,
        &ctx,
        Pos2::new(ruler.left() + 180.0, ruler.bottom() - 6.0),
    );
    assert_eq!(app.session.project.transport.playhead_frame, 60_000);
    let start = Pos2::new(ruler.left() + 240.0, ruler.top() + 5.0);
    let end = start + Vec2::new(240.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let region = &app.session.project.transport.r#loop;
    assert_eq!((region.start_frame, region.end_frame), (72_000, 120_000));
    assert!(app.dirty);
    let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
    let point = app.effects_bounds.strip.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
    let point = app.lane_bounds[&track].left_center() + Vec2::new(120.0, 0.0);
    let anchor = app.view_scroll() + 120.0 / f64::from(app.view_zoom());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!(app.view_zoom(), 480.0);
    assert!((app.view_scroll() + 120.0 / f64::from(app.view_zoom()) - anchor).abs() < 0.001);
    let grid_point = Pos2::new(
        point.x,
        (app.lane_bounds[&track].bottom() + app.effects_bounds.scrollbar.top()) * 0.5,
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(grid_point),
            egui::Event::Zoom(1.25),
        ],
    );
    assert_eq!(app.view_zoom(), 600.0);
    app.set_view_scroll(0.0);
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let project = format!("{:?}", app.session.project);
    let start = app.effects_bounds.scrollbar.left_center() + Vec2::new(30.0, -1.0);
    let end = start + Vec2::new(80.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(app.view_scroll() > 0.0);
    assert_eq!(app.lane_bounds[&track], lane);
    assert_eq!(format!("{:?}", app.session.project), project);
    assert!(!app.dirty);
    assert_eq!((app.zoom, app.scroll), timeline);
}
