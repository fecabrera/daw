use super::*;
use crate::mixer::ViewMode;

fn double_click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    // Separate gestures so egui does not classify their clicks as a triple-click.
    let time = ctx.input(|input| input.time) + 1.0;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(time),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    for _ in 0..2 {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(app, ctx, vec![button(point, false)]);
    }
}

fn header_point(app: &DawUi, track: Id) -> Pos2 {
    if app.view_mode == ViewMode::Mixer {
        let strip = app.mixer_bounds[&track];
        Pos2::new(strip.left() + 10.0, strip.top() + 12.0)
    } else {
        Pos2::new(110.0, app.lane_bounds[&track].top() + 18.0)
    }
}

#[test]
fn blank_track_headers_select_only_their_clips_and_allow_copy() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, tracks, ids) = multi_clip_fixture();
        let ctx = context();
        app.view_mode = mode;
        app.select_clip(ids[2], tracks[1], false);
        let before = format!("{:?}", app.session.project);
        frame(&mut app, &ctx, vec![]);
        let point = header_point(&app, tracks[0]);
        // A single click selects the track without replacing the clip selection.
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.selected_track, Some(tracks[0]));
        assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
        double_click(&mut app, &ctx, point);
        assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[1]]));
        assert_eq!(app.selected_clip, Some(ids[0]));
        assert!(app.track_name_edit.is_none());
        app.perform_clip_action(ClipAction::Copy, &ctx);
        assert_eq!(app.clip_clipboard.as_ref().unwrap().clips.len(), 2);
        let point = header_point(&app, tracks[2]);
        double_click(&mut app, &ctx, point);
        assert_eq!(app.selected_track, Some(tracks[2]));
        assert!(app.selected_clips.is_empty());
        assert!(app.selected_clip.is_none());
        assert!(app.track_name_edit.is_none());
        assert!(!app.clip_action_enabled(ClipAction::Copy, &ctx));
        assert!(!app.dirty);
        assert_eq!(format!("{:?}", app.session.project), before);
    }
}

#[test]
fn header_spacing_selects_clips_but_names_and_controls_keep_their_actions() {
    let (mut app, tracks, ids) = multi_clip_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let y = app.lane_bounds[&tracks[0]].top() + 18.0;
    double_click(&mut app, &ctx, Pos2::new(32.0, y)); // Gap after the grip.
    assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[1]]));
    app.select_clip(ids[2], tracks[1], false);
    double_click(&mut app, &ctx, Pos2::new(178.0, y)); // Mute button.
    assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
    assert!(!app.session.project.tracks[0].muted);
    double_click(&mut app, &ctx, Pos2::new(50.0, y)); // Visible name text.
    assert_eq!(app.track_name_edit.as_ref().unwrap().track, tracks[0]);
    assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
}

#[test]
fn blank_header_selection_is_blocked_during_jobs_and_dialogs() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        for dialog in [false, true] {
            let (mut app, tracks, ids) = multi_clip_fixture();
            let ctx = context();
            app.view_mode = mode;
            app.select_clip(ids[2], tracks[1], false);
            frame(&mut app, &ctx, vec![]);
            let point = header_point(&app, tracks[0]);
            let (_sender, receiver) = mpsc::channel();
            if dialog {
                app.settings_open = true;
            } else {
                app.job = Some(receiver);
            }
            double_click(&mut app, &ctx, point);
            assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
            assert_eq!(app.selected_track, Some(tracks[1]));
            assert!(app.track_name_edit.is_none());
            assert!(!app.dirty);
        }
    }
}

#[test]
fn tracks_and_layers_share_the_control_panel_surface_header_and_final_border() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for height in [400.0, 1300.0] {
            let mut app = DawUi::default();
            let midi = app.session.project.add_midi_track().unwrap();
            let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
            for index in 2..=4 {
                instrument.layers.push(daw_core::InstrumentLayerSettings {
                    name: format!("Layer {index}"),
                    ..Default::default()
                });
            }
            for _ in 0..8 {
                app.session.project.add_track().unwrap();
            }
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_track(midi, false);
            app.selected_tracks.insert(app.session.project.tracks[1].id);
            for view in [ViewMode::Timeline, ViewMode::Instrument] {
                app.set_view_mode(view, &ctx);
                let size = Vec2::new(1280.0, height);
                frame_shapes(&mut app, &ctx, vec![], size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let palette = theme::palette(&ctx);
                let column = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.fill == palette.panel
                                && rect.rect.width() == TRACK_WIDTH
                                && shape.clip_rect.contains_rect(rect.rect) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .max_by(|a, b| a.height().total_cmp(&b.height()))
                    .unwrap();
                let x = column.right() - 0.5;
                let borders = shapes.iter().enumerate().filter(|(_, shape)| matches!(
                    &shape.shape, egui::Shape::LineSegment { points, stroke }
                        if *points == [Pos2::new(x, column.top()), Pos2::new(x, column.bottom())]
                        && *stroke == Stroke::new(1.0, palette.border)
                )).collect::<Vec<_>>();
                assert_eq!(borders.len(), 1, "one continuous outer border in {view:?}");
                let border_index = borders[0].0;
                assert!(
                    shapes
                        .iter()
                        .enumerate()
                        .filter(|(_, shape)| matches!(
                            &shape.shape, egui::Shape::Rect(rect)
                                if rect.rect.left() <= x && rect.rect.right() >= x
                                    && rect.rect.top() < column.bottom()
                                    && rect.rect.bottom() > column.top()
                        ))
                        .all(|(index, _)| index < border_index),
                    "row fills and selection outlines cannot cover the column border"
                );
                let title = if view == ViewMode::Timeline {
                    "Tracks"
                } else {
                    "Layers"
                };
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == title
                        && (text.pos.x - column.left() - f32::from(theme::TOOLBAR_PADDING)).abs() < 0.1)));
                if view == ViewMode::Timeline {
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.stroke == theme::selection_outline(&ctx)
                            && rect.rect.right() == column.right())));
                } else {
                    assert_eq!(
                        app.midi_editor.instrument_header_bounds.width(),
                        column.width()
                    );
                    assert!(
                        app.midi_editor
                            .instrument_header_bounds
                            .contains_rect(app.midi_editor.add_layer_bounds)
                    );
                }
            }
            assert!(!app.dirty);
        }
    }
}

#[test]
fn timeline_row_dividers_share_track_panel_bounds_at_different_scales_and_scroll_offsets() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for scale in [1.0, 1.25, 2.0] {
            let mut app = DawUi::default();
            let tracks = (0..12)
                .map(|_| app.session.project.add_track().unwrap())
                .collect::<Vec<_>>();
            app.selected_track = Some(tracks[0]);
            app.selected_tracks = tracks.iter().copied().collect();
            let ctx = context();
            ctx.set_pixels_per_point(scale);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            let original = format!("{:?}", app.session.project);
            let size = Vec2::new(900.0, 600.0);
            frame_shapes(&mut app, &ctx, vec![], size);
            let first = app.lane_bounds[&tracks[0]];
            for scroll in [false, true] {
                let events = if scroll {
                    vec![
                        egui::Event::PointerMoved(first.min + Vec2::splat(20.0)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -114.25),
                            modifiers: Default::default(),
                            phase: egui::TouchPhase::Move,
                        },
                    ]
                } else {
                    vec![]
                };
                frame_shapes(&mut app, &ctx, events, size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                if scroll {
                    assert!(app.lane_bounds[&tracks[0]].top() < first.top());
                }
                let mut visible_rows = 0;
                for (index, track) in tracks.iter().enumerate() {
                    let lane = app.lane_bounds[track];
                    if lane.top() < app.timeline_bounds.top()
                        || lane.bottom() > app.master_bounds.top()
                    {
                        continue;
                    }
                    visible_rows += 1;
                    let panel = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.stroke == theme::selection_outline(&ctx)
                                    && rect.rect.right() == lane.left()
                                    && rect.rect.center().y == lane.center().y =>
                            {
                                Some(rect.rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(panel.y_range(), lane.y_range());
                    if index > 0 {
                        let y = panel.top() + 0.5;
                        let lines = shapes.iter().filter(|shape| matches!(
                            &shape.shape, egui::Shape::LineSegment { points, stroke }
                                if *points == [Pos2::new(panel.left(), y), Pos2::new(lane.right(), y)]
                                    && *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                        )).count();
                        assert_eq!(lines, 1, "one aligned separator across panel and lane");
                    }
                    assert!(
                        !shapes.iter().any(|shape| matches!(
                            &shape.shape, egui::Shape::LineSegment { points, stroke }
                                if *points == [lane.left_bottom(), lane.right_bottom()]
                                    && *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                        )),
                        "no separately positioned lane border"
                    );
                }
                assert!(visible_rows > 1);
            }
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}
