use super::*;

#[test]
fn mixer_headers_match_inherited_clip_headers_in_both_themes_and_selection_states() {
    let (mut app, track) = fixture();
    let next_track = app.session.project.add_track().unwrap();
    app.view_mode = crate::mixer::ViewMode::Mixer;
    // A clip override must not replace the track's default clip color.
    app.session.project.tracks[0].clips[0].color = Some(daw_core::DEFAULT_TRACK_COLORS[0]);
    let ctx = context();
    for color in [
        daw_core::DEFAULT_TRACK_COLORS[3],
        daw_core::DEFAULT_TRACK_COLORS[11],
        daw_core::LEGACY_CLIP_COLOR,
    ] {
        app.session.project.tracks[0].color = color;
        let colors = theme::clip_colors(color, None);
        for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            for selected in [false, true] {
                app.selected_track = None;
                app.selected_tracks.clear();
                if selected {
                    app.select_track(track, false);
                }
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let strip = app.mixer_bounds[&track];
                let name = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == "Track 1" => Some(text),
                        _ => None,
                    })
                    .unwrap();
                let point = name.pos + name.galley.rect.center().to_vec2();
                assert!((point.x - strip.center().x).abs() < 1.0);
                assert!((point.y - (strip.top() + 12.0)).abs() < 1.0);
                assert!(
                    name.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.color == colors.text
                            && section.format.font_id.size == 11.0)
                );
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.fill == colors.header
                        && rect.rect.top() == strip.top()
                        && rect.rect.width() == strip.width()
                        && rect.rect.height() == 24.0
                        && rect.rect.contains(point))));
                for bounds in [strip, app.master_bounds] {
                    let points = [
                        Pos2::new(bounds.left() + 0.5, bounds.top()),
                        Pos2::new(bounds.left() + 0.5, bounds.bottom()),
                    ];
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::LineSegment { points: line, stroke }
                                if *line == points && *stroke == egui::Stroke::new(1.0, theme::palette(&ctx).border))));
                }
                assert!(
                    shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.fill == theme::palette(&ctx).panel
                            && rect.rect.width() == 1280.0 && rect.rect.bottom() == strip.top()
                    )),
                    "Mixer must meet the toolbar without a gap"
                );
                assert_eq!(shapes.iter().filter(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if points[0].y == points[1].y && (points[0].y - strip.top()).abs() <= 1.0
                            && *stroke == egui::Stroke::new(1.0, theme::palette(&ctx).border)
                )).count(), 1, "One shared thin top border");
                if selected {
                    let outline = theme::selection_outline(&ctx);
                    let path = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Path(path)
                                if path.points[1] == strip.shrink(2.0).left_top()
                                    && path.stroke.color
                                        == egui::epaint::ColorMode::Solid(outline.color)
                                    && path.stroke.width == outline.width =>
                            {
                                Some(path)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(
                        path.points[2].x + path.stroke.width / 2.0,
                        app.mixer_bounds[&next_track].left(),
                        "Selected right outline must meet the next strip without a gap"
                    );
                }
                assert_eq!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if points[0] == Pos2::new(strip.left() + 1.0, strip.bottom() - theme::selection_outline(&ctx).width / 2.0)
                            && points[1] == Pos2::new(strip.right() - 1.0, strip.bottom() - theme::selection_outline(&ctx).width / 2.0)
                            && *stroke == theme::selection_outline(&ctx)
                )), selected);
                assert!(!app.dirty);
            }
        }
    }
}

fn label_point(app: &mut DawUi, ctx: &egui::Context, label: &str, bounds: Option<Rect>) -> Pos2 {
    frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let point = text.pos + text.galley.rect.center().to_vec2();
                bounds
                    .is_none_or(|rect| rect.contains(point))
                    .then_some(point)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing label: {label}"))
}

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

fn replace_text(app: &mut DawUi, ctx: &egui::Context, text: &str) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text(text.into()),
        ],
    );
}

#[test]
fn mixer_numeric_controls_commit_cancel_and_reject_invalid_values() {
    let (mut app, track) = fixture();
    app.view_mode = crate::mixer::ViewMode::Mixer;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let strip = app.mixer_bounds[&track];
    let pan = label_point(&mut app, &ctx, "0.00", Some(strip));
    assert!(
        (pan.x - strip.center().x).abs() < 1.0,
        "pan {pan:?} strip {strip:?}"
    );
    click(&mut app, &ctx, pan);
    replace_text(&mut app, &ctx, "0.75");
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    assert_eq!(app.session.project.tracks[0].pan, 0.75);
    let gain = label_point(&mut app, &ctx, "0.0", Some(strip));
    click(&mut app, &ctx, gain);
    replace_text(&mut app, &ctx, "-9.0");
    click(&mut app, &ctx, strip.center());
    assert_eq!(app.session.project.tracks[0].gain_db, -9.0);
    let gain = label_point(&mut app, &ctx, "-9.0", Some(strip));
    click(&mut app, &ctx, gain);
    replace_text(&mut app, &ctx, "-20.0");
    frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
    assert_eq!(app.session.project.tracks[0].gain_db, -9.0);
    click(&mut app, &ctx, gain);
    replace_text(&mut app, &ctx, "NaN");
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    assert_eq!(app.session.project.tracks[0].gain_db, -9.0);
    assert!(app.error.is_some());
}

#[test]
fn switches_views_without_changing_the_project_or_timeline_position() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.zoom = 123.0;
    app.scroll = 7.0;
    app.select_track(track, false);
    app.selected_clip = Some(app.session.project.tracks[0].clips[0].id);
    let project = format!("{:?}", app.session.project);
    let point = label_point(&mut app, &ctx, "Mixer", None);
    click(&mut app, &ctx, point);
    assert!(app.view_mode == crate::mixer::ViewMode::Mixer);
    assert!(!app.dirty);
    assert!(!app.sync_needed);
    assert!(app.timeline_bounds == Rect::NOTHING && app.lane_bounds.is_empty());
    assert!(app.file_drop_target.is_none());
    assert_eq!(format!("{:?}", app.session.project), project);
    assert!(app.track_selected(track));
    let point = label_point(&mut app, &ctx, "Timeline", None);
    click(&mut app, &ctx, point);
    assert!(app.view_mode == crate::mixer::ViewMode::Timeline);
    assert_eq!((app.zoom, app.scroll), (123.0, 7.0));
    assert!(app.lane_bounds.contains_key(&track));
    assert!(app.selected_clip.is_some());
}

#[test]
fn strips_scroll_in_project_order_with_master_pinned_during_resize() {
    let mut app = DawUi::default();
    let tracks: Vec<_> = (0..20)
        .map(|_| app.session.project.add_track().unwrap())
        .collect();
    app.view_mode = crate::mixer::ViewMode::Mixer;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let master = app.master_bounds;
    assert_eq!(master.right(), 1280.0);
    for pair in tracks.windows(2) {
        assert_eq!(
            app.mixer_bounds[&pair[0]].right(),
            app.mixer_bounds[&pair[1]].left()
        );
    }
    let first = app.mixer_bounds[&tracks[0]];
    for _ in 0..3 {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(first.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-200.0, 0.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
        );
    }
    assert!(app.mixer_bounds[&tracks[0]].left() < first.left());
    assert_eq!(app.master_bounds, master);
    for size in [Vec2::new(760.0, 500.0), Vec2::new(1400.0, 900.0)] {
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        for pair in tracks.windows(2) {
            let x = app.mixer_bounds[&pair[1]].left();
            if x < 1.0 || x + 1.0 >= app.master_bounds.left() {
                continue;
            }
            let dividers = shapes
                .iter()
                .filter(|shape| {
                    matches!(&shape.shape,
                        egui::Shape::LineSegment { points, stroke }
                            if points[0].x == points[1].x && (points[0].x - x).abs() <= 1.0
                                && *stroke == egui::Stroke::new(1.0, theme::palette(&ctx).border)
                    )
                })
                .count();
            assert_eq!(dividers, 1, "Stacked divider at {x}");
        }
        let status = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == theme::palette(&ctx).panel
                        && rect.rect.width() == size.x
                        && rect.rect.bottom() == size.y =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("Status bar background");
        assert_eq!(app.master_bounds.bottom(), status.top());
        assert_eq!(
            app.mixer_bounds[&tracks[0]].bottom(),
            app.master_bounds.bottom()
        );
        let separators = shapes
            .iter()
            .filter(|shape| {
                matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if points[0] == Pos2::new(0.0, status.top() - 0.5)
                            && points[1] == Pos2::new(size.x, status.top() - 0.5)
                            && *stroke == egui::Stroke::new(1.0, theme::palette(&ctx).border)
                )
            })
            .count();
        assert_eq!(separators, 1);
        assert_eq!(app.master_bounds.right(), size.x);
        let master_fader = app.mixer_faders[&None];
        let track_fader = app.mixer_faders[&Some(tracks[0])];
        assert!(
            (master_fader.bottom() - track_fader.bottom()).abs() < 1.0,
            "master {master_fader:?}, track {track_fader:?}"
        );
        assert!(app.master_bounds.contains_rect(master_fader));
        assert!(app.mixer_bounds[&tracks[0]].contains_rect(track_fader));
    }
    assert!(!app.dirty);
}

#[test]
fn mixer_faders_and_buttons_change_shared_values_and_preserve_group_selection() {
    let (mut app, track) = fixture();
    let second = app.session.project.add_track().unwrap();
    app.view_mode = crate::mixer::ViewMode::Mixer;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let first_rect = app.mixer_bounds[&track];
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for label in ["M", "S"] {
        let center = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    assert!(text.galley.job.sections.iter().all(|section| {
                        section.format.font_id == egui::FontId::proportional(11.0)
                    }));
                    let center = text.pos + text.galley.size() * 0.5;
                    first_rect.contains(center).then_some(center)
                }
                _ => None,
            })
            .unwrap();
        let rect = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.contains(center) && rect.rect.width() < 30.0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(rect.size(), Vec2::splat(toolbars::CONTROL_HEIGHT));
        assert!(first_rect.contains_rect(rect));
    }
    click(&mut app, &ctx, first_rect.center());
    let second_rect = app.mixer_bounds[&second];
    let point = second_rect.center();
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![button(point, false)], true);
    assert!(app.track_selected(track) && app.track_selected(second));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::ModifiersChanged(Default::default())],
    );
    frame(&mut app, &ctx, vec![key(egui::Key::M)]);
    assert!(app.session.project.tracks.iter().all(|t| t.muted));
    let point = label_point(&mut app, &ctx, "S", Some(first_rect));
    click(&mut app, &ctx, point);
    assert!(app.session.project.tracks[0].soloed);
    assert!(!app.session.project.tracks[1].soloed);
    assert_eq!(app.selected_tracks.len(), 1);
    for id in [Some(track), None] {
        let rect = app.mixer_faders[&id];
        click(
            &mut app,
            &ctx,
            Pos2::new(rect.center().x, rect.top() + 12.0),
        );
    }
    assert!(app.session.project.tracks[0].gain_db > 0.0);
    assert!(app.session.project.master.gain_db > 0.0);
    assert!(app.dirty);
    let gain = app.session.project.tracks[0].gain_db;
    app.view_mode = crate::mixer::ViewMode::Timeline;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.tracks[0].gain_db, gain);
}

#[test]
fn mixer_name_editing_and_out_of_range_gain_survive_display() {
    let (mut app, track) = fixture();
    app.view_mode = crate::mixer::ViewMode::Mixer;
    app.session.project.tracks[0].gain_db = 20.0;
    let ctx = context();
    let point = label_point(&mut app, &ctx, "Track 1", None);
    assert_eq!(app.session.project.tracks[0].gain_db, 20.0);
    assert!(!app.dirty);
    click(&mut app, &ctx, point);
    click(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.track_name_edit
            .as_ref()
            .is_some_and(|e| e.track == track)
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::Text("Mixer name".into()),
            key(egui::Key::Enter),
        ],
    );
    assert_eq!(app.session.project.tracks[0].name, "Mixer name");
    assert!(app.track_drag.is_none());
}

#[test]
fn view_buttons_keep_the_same_style_across_titlebars_and_pointer_states() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Yellow,
        ] {
            for mode in [
                crate::mixer::ViewMode::Timeline,
                crate::mixer::ViewMode::Mixer,
            ] {
                let mut reference_size = None;
                for titlebar in [
                    settings::TitlebarStyle::Custom,
                    settings::TitlebarStyle::Native,
                ] {
                    let mut app = DawUi::default();
                    let ctx = context();
                    app.set_settings(
                        &ctx,
                        settings::AppSettings {
                            theme: color_theme,
                            accent,
                            titlebar,
                        },
                    );
                    app.view_mode = mode;
                    if cfg!(target_os = "macos") {
                        app.use_native_menu();
                    }
                    let selected_label = if mode == crate::mixer::ViewMode::Timeline {
                        "Timeline"
                    } else {
                        "Mixer"
                    };
                    let point = label_point(&mut app, &ctx, selected_label, None);
                    let unselected_label = if selected_label == "Timeline" {
                        "Mixer"
                    } else {
                        "Timeline"
                    };
                    let unselected_point = label_point(&mut app, &ctx, unselected_label, None);
                    for (phase, (events, hovered)) in [
                        (vec![egui::Event::PointerGone], None),
                        (vec![egui::Event::PointerMoved(point)], Some(selected_label)),
                        (vec![button(point, true)], Some(selected_label)),
                        (vec![button(point, false)], Some(selected_label)),
                        (
                            vec![egui::Event::PointerMoved(unselected_point)],
                            Some(unselected_label),
                        ),
                        (
                            vec![egui::Event::PointerMoved(Pos2::new(1000.0, 500.0))],
                            None,
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let shapes = frame_shapes(&mut app, &ctx, events, Vec2::new(1280.0, 800.0));
                        let fill = ctx.global_style().visuals.selection.bg_fill;
                        for label in ["Timeline", "Mixer"] {
                            let text = shapes
                                .iter()
                                .find_map(|shape| match &shape.shape {
                                    egui::Shape::Text(text) if text.galley.text() == label => {
                                        Some(text)
                                    }
                                    _ => None,
                                })
                                .unwrap();
                            let expected = if label == selected_label || hovered == Some(label) {
                                match color_theme {
                                    settings::ColorTheme::Dark => egui::Color32::WHITE,
                                    settings::ColorTheme::Light => egui::Color32::BLACK,
                                }
                            } else {
                                theme::palette(&ctx).secondary
                            };
                            assert!(
                                text.galley.job.sections.iter().all(|section| {
                                    let color = text.override_text_color.unwrap_or_else(|| {
                                        if section.format.color == egui::Color32::PLACEHOLDER {
                                            text.fallback_color
                                        } else {
                                            section.format.color
                                        }
                                    });
                                    color == expected
                                }),
                                "{label}: phase={phase}, hovered={hovered:?}, expected={expected:?}, fallback={:?}",
                                text.fallback_color
                            );
                            if label == selected_label {
                                let center = text.pos + text.galley.rect.center().to_vec2();
                                let background = shapes
                                    .iter()
                                    .find_map(|shape| match &shape.shape {
                                        egui::Shape::Rect(rect)
                                            if rect.fill == fill && rect.rect.contains(center) =>
                                        {
                                            Some(rect)
                                        }
                                        _ => None,
                                    })
                                    .expect("Selected view background");
                                assert_eq!(background.stroke, egui::Stroke::NONE);
                                assert_eq!(
                                    background.corner_radius,
                                    ctx.global_style().visuals.widgets.inactive.corner_radius
                                );
                                if let Some(size) = reference_size {
                                    assert_eq!(background.rect.size(), size);
                                } else {
                                    reference_size = Some(background.rect.size());
                                }
                            } else {
                                let center = text.pos + text.galley.rect.center().to_vec2();
                                assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                                    egui::Shape::Rect(rect) if rect.fill == fill && rect.rect.contains(center)
                                )), "Hover must not add the selected background");
                            }
                        }
                        assert!(app.view_mode == mode);
                        assert!(!app.dirty);
                    }
                }
            }
        }
    }
}

#[test]
fn mode_toolbar_uses_platform_titlebar_placement_and_modal_guards() {
    for titlebar in [
        settings::TitlebarStyle::Custom,
        settings::TitlebarStyle::Native,
    ] {
        let mut app = DawUi::default();
        app.settings.titlebar = titlebar;
        if cfg!(target_os = "macos") {
            app.use_native_menu();
        }
        let ctx = context();
        let point = label_point(&mut app, &ctx, "Mixer", None);
        assert!(point.y < app.time_bounds[0].center().y);
        if cfg!(target_os = "macos") && titlebar == settings::TitlebarStyle::Custom {
            let title = app.window_title.clone();
            let title_point = label_point(&mut app, &ctx, &title, None);
            assert!((point.y - title_point.y).abs() < 1.0);
            assert!(point.x > title_point.x);
        }
        app.settings_open = true;
        click(&mut app, &ctx, point);
        assert!(app.view_mode == crate::mixer::ViewMode::Timeline);
    }
}
