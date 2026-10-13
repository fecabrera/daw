use super::*;

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn menu_and_view_buttons_share_one_row_with_a_separator() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for mode in ViewMode::ALL {
            for width in [900.0, 1280.0] {
                let mut app = DawUi::default();
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                app.view_mode = mode;
                let size = Vec2::new(width, 800.0);
                frame_sized(&mut app, &ctx, vec![], size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let label_rect = |label: &str| {
                    shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => {
                                Some(Rect::from_min_size(text.pos, text.galley.size()))
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing label: {label}"))
                };
                let rects = ViewMode::ALL.map(|mode| label_rect(mode.label()));
                for pair in rects.windows(2) {
                    assert!(pair[0].right() < pair[1].left());
                    assert!((pair[0].center().y - pair[1].center().y).abs() < 0.1);
                }
                assert!(rects.last().unwrap().right() < width);
                assert!(rects[0].bottom() < app.time_bounds[0].top());
                assert!(!shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == app.window_title)
                }));
                assert!(!shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text)
                        if ["File", "Track", "View"].contains(&text.galley.text()))
                }));
                let timeline_index = 0;
                let menus_right = f32::from(theme::TOOLBAR_PADDING) + 28.0;
                let separators = shapes
                    .iter()
                    .filter(|shape| {
                        matches!(&shape.shape, egui::Shape::LineSegment { points, .. }
                        if (points[0].x - points[1].x).abs() < 0.1
                            && points[0].x > menus_right
                            && points[0].x < rects[timeline_index].left()
                            && points[0].y < rects[timeline_index].center().y
                            && points[1].y > rects[timeline_index].center().y)
                    })
                    .count();
                assert_eq!(separators, 1);

                let project = format!("{:?}", app.session.project);
                let (target, index) = if mode == ViewMode::Timeline {
                    (ViewMode::Mixer, rects.len() - 1)
                } else {
                    (ViewMode::Timeline, timeline_index)
                };
                let point = rects[index].center();
                frame_sized(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                    size,
                );
                assert_eq!(app.view_mode, mode);
                frame_sized(&mut app, &ctx, vec![button(point, false)], size);
                assert_eq!(app.view_mode, target);
                assert_eq!(format!("{:?}", app.session.project), project);
                assert!(!app.dirty);
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn native_toolbar_burger_opens_menus_and_settings_without_project_changes() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let mut app = DawUi::default();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                titlebar: settings::TitlebarStyle::Native,
                ..Default::default()
            },
        );
        let project = format!("{:?}", app.session.project);
        let timeline = label_point(&mut app, &ctx, "Timeline", None);
        let burger = Pos2::new(f32::from(theme::TOOLBAR_PADDING) + 14.0, timeline.y);
        click(&mut app, &ctx, burger);
        assert!(egui::Popup::is_any_open(&ctx));
        for label in ["File", "Edit", "Track", "View"] {
            label_point(&mut app, &ctx, label, None);
        }
        let file = label_point(&mut app, &ctx, "File", None);
        click(&mut app, &ctx, file);
        let settings = label_point(&mut app, &ctx, "Settings…", None);
        click(&mut app, &ctx, settings);
        assert!(app.settings_open);
        assert_eq!(format!("{:?}", app.session.project), project);
        assert!(!app.dirty);
    }
}

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
                        egui::Shape::Text(text) if text.galley.text() == "Audio 1" => Some(text),
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
fn view_menu_checks_one_mode_and_switches_on_release_without_project_changes() {
    fn menu_frame(
        app: &mut DawUi,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
                events,
                ..Default::default()
            },
            |ui| app.view_menu(ui),
        );
        output.textures_delta.clear();
        output
    }
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.zoom = 123.0;
        app.scroll = 7.0;
        let clip = app.session.project.tracks[0].clips[0].id;
        app.select_clip(clip, track, false);
        let project = format!("{:?}", app.session.project);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert_eq!(
            shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == "View"
            )),
            !cfg!(any(target_os = "windows", target_os = "linux"))
        );
        for mode in ViewMode::ALL.into_iter().chain([ViewMode::Timeline]) {
            let output = menu_frame(&mut app, &ctx, vec![]);
            let checks: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) if path.points.len() == 3 => Some(path),
                    _ => None,
                })
                .collect();
            assert_eq!(checks.len(), 1);
            let point = |label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let checked = point(app.view_mode().label());
            let check_rect = Rect::from_points(&checks[0].points);
            assert!((check_rect.center().y - checked.y).abs() < 1.0);
            assert!(check_rect.right() < checked.x);
            let destination = point(mode.label());
            let before = app.view_mode();
            menu_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(destination),
                    button(destination, true),
                ],
            );
            assert_eq!(app.view_mode(), before);
            menu_frame(&mut app, &ctx, vec![button(destination, false)]);
            assert_eq!(app.view_mode(), mode);
            assert_eq!(format!("{:?}", app.session.project), project);
            assert_eq!((app.zoom, app.scroll), (123.0, 7.0));
            assert_eq!(app.selected_clips, HashSet::from([clip]));
            assert!(app.track_selected(track));
            assert!(!app.dirty && !app.sync_needed);
        }
    }
}

#[test]
fn menu_and_native_view_dispatch_share_guards_and_draft_cleanup() {
    for busy in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.settings_open = !busy;
        let (_sender, receiver) = mpsc::channel();
        if busy {
            app.job = Some(receiver);
        }
        assert!(!app.view_switch_enabled());
        app.set_view_mode(ViewMode::Mixer, &ctx);
        assert_eq!(app.view_mode(), ViewMode::Timeline);
        app.settings_open = false;
        app.job = None;
        app.track_name_edit = Some(TrackNameEdit {
            track,
            text: "Uncommitted".into(),
            focus: true,
        });
        app.inputs.insert(
            track,
            Inputs {
                gain: "-12".into(),
                pan: "0.5".into(),
            },
        );
        assert!(app.view_switch_enabled());
        app.set_view_mode(ViewMode::Mixer, &ctx);
        assert_eq!(app.view_mode(), ViewMode::Mixer);
        assert!(app.track_name_edit.is_none() && app.inputs.is_empty());
        assert!(!app.dirty);
    }
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
    let point = label_point(&mut app, &ctx, "Audio 1", None);
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
            for mode in ViewMode::ALL {
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
                    let selected_label = mode.label();
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

#[test]
fn fader_scales_fit_the_strips_and_silence_survives_view_changes() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Mixer, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for id in [Some(track), None] {
            let rect = app.mixer_faders[&id];
            let strip = id.map_or(app.master_bounds, |id| app.mixer_bounds[&id]);
            let inset = rect.width() / 5.0;
            let mut labels = Vec::new();
            for (db, label) in [
                (0.0, "0"),
                (-3.0, "-3"),
                (-6.0, "-6"),
                (-10.0, "-10"),
                (-15.0, "-15"),
                (-20.0, "-20"),
                (-30.0, "-30"),
                (-40.0, "-40"),
                (daw_core::SILENT_GAIN_DB, "−∞"),
                (3.0, "+3"),
                (6.0, "+6"),
                (9.0, "+9"),
                (12.0, "+12"),
            ] {
                let y = egui::lerp(
                    (rect.bottom() - inset)..=(rect.top() + inset),
                    faders::position_for_gain(db),
                );
                let bounds = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => {
                            let bounds = text.galley.rect.translate(text.pos.to_vec2());
                            ((bounds.center().y - y).abs() < 0.6
                                && bounds.right() < rect.left()
                                && bounds.left() >= rect.left() - faders::SCALE_WIDTH - 0.1)
                                .then_some(bounds)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing fader label {label} on {id:?}"));
                assert!(
                    strip.contains_rect(bounds),
                    "{label}: {bounds:?}, strip: {strip:?}"
                );
                labels.push(bounds);
            }
            labels.sort_by(|a, b| a.top().total_cmp(&b.top()));
            for pair in labels.windows(2) {
                assert!(pair[0].bottom() < pair[1].top());
            }
            let subdivisions = shapes
                .iter()
                .filter(|shape| {
                    matches!(&shape.shape,
                        egui::Shape::LineSegment { points, .. } if
                            (points[1].x - points[0].x - 3.0).abs() < 0.1
                            && (points[1].x - (rect.left() - 3.0)).abs() < 0.1
                            && points[0].y >= rect.top() && points[0].y <= rect.bottom()
                    )
                })
                .count();
            assert!(subdivisions >= 10);
            for db in [1.0, 2.0, 4.0, 5.0, 7.0, 8.0, 10.0, 11.0] {
                let y = egui::lerp(
                    (rect.bottom() - inset)..=(rect.top() + inset),
                    faders::position_for_gain(db),
                );
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, .. } if
                        (points[1].x - points[0].x - 3.0).abs() < 0.1
                        && (points[1].x - (rect.left() - 3.0)).abs() < 0.1
                        && (points[0].y - y).abs() < 0.1
                )));
            }
            let rails: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(shape)
                        if shape.fill == theme::palette(&ctx).input
                            && (shape.rect.width() - 8.0).abs() < 0.1
                            && (shape.rect.height() - (rect.height() - 10.0)).abs() < 0.1
                            && strip.contains_rect(shape.rect) =>
                    {
                        Some(shape.rect)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(rails.len(), 2);
            let rail = rails[0];
            for db in [
                0, -3, -6, -9, -12, -15, -18, -21, -24, -30, -35, -40, -45, -50, -60,
            ] {
                let y = egui::lerp(rail.bottom()..=rail.top(), (db as f32 + 60.0) / 60.0);
                assert!(
                    shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Text(text) if text.galley.text() == db.to_string() && {
                            let bounds = text.galley.rect.translate(text.pos.to_vec2());
                            strip.contains_rect(bounds)
                                && bounds.left() > rect.right()
                                && bounds.right() < rail.left()
                                && (bounds.center().y - y).abs() < 0.6
                        }
                    )),
                    "Missing meter label {db} on {id:?}"
                );
            }
            click(&mut app, &ctx, Pos2::new(rect.center().x, rect.bottom()));
            let db = id.map_or(app.session.project.master.gain_db, |_| {
                app.session.project.tracks[0].gain_db
            });
            assert_eq!(db, daw_core::SILENT_GAIN_DB);
            assert_eq!(daw_core::linear_gain(db), 0.0);
        }
        assert!(!app.session.project.tracks[0].muted);
        assert!(!app.session.project.tracks[0].soloed);
        app.set_view_mode(ViewMode::Timeline, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let lane = app.lane_bounds[&track];
        let track_panel = Rect::from_min_size(
            Pos2::new(lane.left() - TRACK_WIDTH, lane.top()),
            Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
        );
        for panel in [track_panel, app.master_bounds] {
            let rails: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(shape)
                        if shape.fill == theme::palette(&ctx).input
                            && (shape.rect.width() - 8.0).abs() < 0.1
                            && shape.rect.height() > 30.0
                            && panel.contains_rect(shape.rect) =>
                    {
                        Some(shape.rect)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(rails.len(), 2);
            assert!((rails[1].right() - rails[0].left() - 20.0).abs() < 0.1);
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if ["0", "-15", "-30", "-45", "-60"].contains(&text.galley.text())
                    && panel.contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
            )));
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, .. } if panel.contains(points[0])
                    && (points[1].x - (rails[0].left() - 2.0)).abs() < 0.1
                    && (points[0].y - points[1].y).abs() < 0.1
                    && (points[1].x - points[0].x).abs() <= 8.0
            )));
        }
        assert_eq!(app.inputs[&track].gain, "−∞");
        assert_eq!(app.master_gain_input, "−∞");
        assert!(app.error.is_none());
    }
}

#[test]
fn gain_and_peak_fields_align_with_marked_columns_in_tracks_and_master() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Mixer, &ctx);
        let project = format!("{:?}", app.session.project);
        for size in [
            Vec2::new(1280.0, 800.0),
            Vec2::new(700.0, 420.0),
            Vec2::new(500.0, 300.0),
        ] {
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            for id in [Some(track), None] {
                let fader = app.mixer_faders[&id];
                let strip = id.map_or(app.master_bounds, |id| app.mixer_bounds[&id]);
                assert!((strip.width() - 120.0).abs() < 0.1);
                let rails: Vec<_> = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(shape)
                            if shape.fill == theme::palette(&ctx).input
                                && (shape.rect.width() - 8.0).abs() < 0.1
                                && (shape.rect.height() - fader.height() + 10.0).abs() < 0.1
                                && strip.contains_rect(shape.rect) =>
                        {
                            Some(shape.rect)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(rails.len(), 2);
                assert!((rails[0].top() - fader.top() - 5.0).abs() < 0.1);
                let fader_center = (fader.left() - faders::SCALE_WIDTH + fader.right()) / 2.0;
                let meter_center = (rails[0].left() - meters::SCALE_WIDTH + rails[1].right()) / 2.0;
                let field_bounds = |label: &str, x: f32| {
                    shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => {
                                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                                (strip.contains_rect(bounds)
                                    && bounds.bottom() < fader.top()
                                    && fader.top() - bounds.bottom() < 15.0
                                    && (bounds.center().x - x).abs() < 0.6)
                                    .then_some(bounds)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing {label} above {id:?} at {size:?}"))
                };
                let gain = field_bounds("0.0", fader_center);
                let peak = field_bounds("−∞", meter_center);
                assert!((gain.center().y - peak.center().y).abs() < 0.1);
                assert!(gain.right() < peak.left());
                for bounds in [gain, peak] {
                    let field = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(shape)
                                if shape.rect.contains_rect(bounds)
                                    && shape.rect.width() < 50.0 =>
                            {
                                Some(shape.rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert!(strip.contains_rect(field));
                    assert!((field.width() - NUMERIC_WIDTH).abs() < 0.1);
                }
                frame_sized(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(peak.center()),
                        button(peak.center(), true),
                    ],
                    size,
                );
                frame_sized(
                    &mut app,
                    &ctx,
                    vec![button(peak.center(), false), egui::Event::Text("20".into())],
                    size,
                );
                assert!(!ctx.egui_wants_keyboard_input());
                assert_eq!(format!("{:?}", app.session.project), project);
                assert!(!app.dirty);
            }
        }
    }
}

#[test]
fn instrument_and_effects_views_use_their_track_scope_without_project_changes() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, audio) = fixture();
        app.session.project.tracks[0].name = "Audio only".into();
        let midi = app.session.project.add_midi_track().unwrap();
        app.session.project.tracks[1].name = "MIDI only".into();
        let clip = app.session.project.tracks[0].clips[0].id;
        app.select_clip(clip, audio, false);
        let project = format!("{:?}", app.session.project);
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Instrument, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let has = |shapes: &[egui::epaint::ClippedShape], name: &str| {
            shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == name))
        };
        assert!(has(
            &shapes,
            "Select a MIDI track to configure its instrument."
        ));
        assert!(!has(&shapes, "Add source"));
        let point = label_point(&mut app, &ctx, "Select MIDI track", None);
        click(&mut app, &ctx, point);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(has(&shapes, "MIDI only"));
        assert!(!has(&shapes, "Audio only"));
        let point = label_point(&mut app, &ctx, "MIDI only", None);
        click(&mut app, &ctx, point);
        assert_eq!(app.selected_track, Some(midi));
        assert_eq!(app.selected_clip, Some(clip));
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(has(&shapes, "Layers"));
        assert!(has(&shapes, "Add source"));
        assert!(!has(&shapes, "Units"));
        app.set_view_mode(ViewMode::Effects, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(!has(&shapes, "Track effects are not available yet."));
        assert!(app.effects_bounds.strip.is_positive());
        assert_eq!(app.effects_bounds.library.width(), TRACK_WIDTH);
        assert!(!has(&shapes, "Layers"));
        assert!(!has(&shapes, "Add source"));
        let point = label_point(&mut app, &ctx, "MIDI only", None);
        click(&mut app, &ctx, point);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(has(&shapes, "Audio only"));
        assert!(has(&shapes, "MIDI only"));
        let point = label_point(&mut app, &ctx, "Audio only", None);
        click(&mut app, &ctx, point);
        assert_eq!(app.selected_track, Some(audio));
        assert_eq!(app.selected_clip, Some(clip));
        assert_eq!(format!("{:?}", app.session.project), project);
        assert!(!app.dirty);
    }
}

#[test]
fn processing_view_names_and_empty_states_keep_their_responsibilities_separate() {
    assert_eq!(
        ViewMode::ALL.map(ViewMode::label),
        ["Timeline", "Instrument", "Effects", "Editor", "Mixer"]
    );
    let mut app = DawUi::default();
    let ctx = context();
    for (mode, prompt) in [
        (
            ViewMode::Instrument,
            "Select a MIDI track to configure its instrument.",
        ),
        (
            ViewMode::Effects,
            "Select a track to configure its effects.",
        ),
    ] {
        app.set_view_mode(mode, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == prompt))
        );
        assert!(!app.dirty);
    }
}
