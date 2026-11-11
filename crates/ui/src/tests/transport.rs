use super::*;

fn space_event(pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn play_pause_has_priority_over_focused_controls_but_not_text_editors() {
    for text_editor in [false, true] {
        let ctx = context();
        let mut text = String::new();
        let mut requests = 0;
        let mut clicks = 0;
        for step in 0..5 {
            let events = match step {
                1 | 2 | 4 => vec![
                    space_event(true, egui::Modifiers::NONE),
                    egui::Event::Text(" ".into()),
                ],
                3 => vec![space_event(false, egui::Modifiers::NONE)],
                _ => vec![],
            };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    requests += usize::from(transport_controls::take_play_pause(ui.ctx()));
                    let response = if text_editor {
                        ui.text_edit_singleline(&mut text)
                    } else {
                        let response = ui.button("Focused control");
                        clicks += usize::from(response.clicked());
                        response
                    };
                    if step == 0 {
                        response.request_focus();
                    }
                },
            );
            output.textures_delta.clear();
            assert!(ctx.egui_wants_keyboard_input());
            assert_eq!(ctx.text_edit_focused(), text_editor);
        }
        assert_eq!(requests, if text_editor { 0 } else { 2 });
        assert_eq!(text, if text_editor { "   " } else { "" });
        assert_eq!(clicks, 0, "Space must not also activate the focused button");
    }
}

#[test]
fn play_pause_preserves_modified_space_and_requires_window_focus() {
    for (focused, modifiers) in [
        (false, egui::Modifiers::NONE),
        (true, egui::Modifiers::COMMAND),
        (true, egui::Modifiers::CTRL),
        (true, egui::Modifiers::ALT),
    ] {
        let ctx = context();
        let mut output = ctx.run_ui(
            egui::RawInput {
                focused,
                events: vec![space_event(true, modifiers)],
                ..Default::default()
            },
            |ui| {
                assert!(!transport_controls::take_play_pause(ui.ctx()));
                assert!(ui.input(|input| input.key_pressed(egui::Key::Space)));
            },
        );
        output.textures_delta.clear();
    }
}

type IconColors = [Option<[u8; 3]>; 3];

fn controls_frame(
    ctx: &egui::Context,
    state: [bool; 4],
    events: Vec<egui::Event>,
    enabled: bool,
) -> (
    transport_controls::Buttons,
    Rect,
    Vec<egui::epaint::ClippedShape>,
    IconColors,
) {
    controls_frame_with_stop(
        ctx,
        state,
        transport_controls::StopAction::from_transport(state[0], false, 1),
        events,
        enabled,
    )
}

fn controls_frame_with_stop(
    ctx: &egui::Context,
    state: [bool; 4],
    stop_action: transport_controls::StopAction,
    events: Vec<egui::Event>,
    enabled: bool,
) -> (
    transport_controls::Buttons,
    Rect,
    Vec<egui::epaint::ClippedShape>,
    IconColors,
) {
    let mut controls = None;
    let mut bounds = Rect::NOTHING;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| {
            let row =
                toolbars::Toolbar::row_with_control_height(ui, icons::TOOLBAR_BUTTON_SIZE, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        transport_controls::show(
                            ui,
                            state[0],
                            stop_action,
                            state[1],
                            state[2],
                            state[3],
                        )
                    })
                    .inner
                });
            bounds = row.response.rect;
            controls = Some(row.inner);
        },
    );
    let buttons = controls.unwrap();
    let icon_colors = [buttons.play.rect, buttons.stop.rect, buttons.looping.rect].map(|rect| {
        let texture = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Rect(image)
                if image.rect == Rect::from_center_size(rect.center(), Vec2::splat(16.0)) =>
            {
                image.brush.as_ref().map(|brush| brush.fill_texture_id)
            }
            _ => None,
        })?;
        let delta = output.textures_delta.set.get(&texture)?.last()?;
        let egui::ImageData::Color(image) = &delta.image;
        let pixel = image.pixels.iter().max_by_key(|pixel| pixel.a())?;
        let rgba = pixel.to_srgba_unmultiplied();
        Some([rgba[0], rgba[1], rgba[2]])
    });
    output.textures_delta.clear();
    (buttons, bounds, output.shapes, icon_colors)
}

fn end_click_sequence(app: &mut DawUi, ctx: &egui::Context) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|input| input.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
}

#[test]
fn independent_transport_buttons_preserve_size_hover_and_semantic_colors_in_all_themes() {
    for color_theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let ctx = context();
        let mut app = DawUi::default();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                accent: settings::AccentColor::Red,
                ..Default::default()
            },
        );
        let play_fill = egui::Color32::from_rgb(51, 133, 85);
        let stop_fill = egui::Color32::from_rgb(197, 78, 88);
        let loop_fill = egui::Color32::from_rgb(175, 140, 62);
        let foreground = egui::Color32::WHITE;
        let mut checked_icons = [false; 3];
        for playing in [false, true] {
            for looping in [false, true] {
                let state = [playing, looping, true, true];
                let away = egui::Event::PointerMoved(Pos2::new(1000.0, 300.0));
                let (buttons, row, shapes, icon_colors) =
                    controls_frame(&ctx, state, vec![away], true);
                for (index, active, texture_index) in
                    [(0, playing, 0), (1, playing, 1), (2, looping, 2)]
                {
                    if active && let Some(color) = icon_colors[texture_index] {
                        assert_eq!(color, [foreground.r(), foreground.g(), foreground.b()]);
                        checked_icons[index] = true;
                    }
                }
                assert_eq!(row.height(), toolbars::HEIGHT);
                let rects = [buttons.play.rect, buttons.stop.rect, buttons.looping.rect];
                assert!(
                    rects
                        .iter()
                        .all(|rect| rect.size() == Vec2::new(30.0, 24.0))
                );
                for pair in rects.windows(2) {
                    assert_eq!(pair[1].left() - pair[0].right(), 7.0);
                    assert_eq!(pair[0].center().y, pair[1].center().y);
                }
                assert_eq!(rects[0].top() - row.top(), 7.0);
                assert_eq!(rects[0].left() - row.left(), 7.0);
                assert_eq!(row.bottom() - rects[0].bottom(), 7.0);
                let group = rects[0].union(rects[2]);
                assert_eq!(group.size(), Vec2::new(104.0, 24.0));
                assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(paint) if paint.rect == group
                )));
                assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, .. }
                        if points[0].y == group.top() && points[1].y == group.bottom()
                            && points[0].x > group.left() && points[0].x < group.right()
                )));
                let fills = [
                    if playing {
                        play_fill
                    } else {
                        theme::unit_separator(&ctx)
                    },
                    if playing {
                        stop_fill
                    } else {
                        theme::unit_separator(&ctx)
                    },
                    if looping {
                        loop_fill
                    } else {
                        theme::unit_separator(&ctx)
                    },
                ];
                for (rect, fill) in rects.into_iter().zip(fills) {
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(image)
                        if image.brush.is_some() && image.rect == Rect::from_center_size(rect.center(), Vec2::splat(16.0)))));
                    assert!(shapes.iter().any(
                        |shape| matches!(&shape.shape, egui::Shape::Rect(paint)
                        if paint.rect == rect && paint.fill == fill && paint.stroke == Stroke::NONE
                            && paint.corner_radius == egui::CornerRadius::same(5))
                    ));
                    assert!(
                        !shapes
                            .iter()
                            .any(|shape| matches!(&shape.shape, egui::Shape::Rect(paint)
                        if paint.rect == rect && paint.stroke.width > 0.0))
                    );
                    let (_, _, hovered, _) = controls_frame(
                        &ctx,
                        state,
                        vec![egui::Event::PointerMoved(rect.center())],
                        true,
                    );
                    assert_eq!(
                        hovered
                            .iter()
                            .any(|shape| matches!(&shape.shape, egui::Shape::Rect(paint)
                        if paint.rect == rect && paint.fill == theme::unit_header_fill(&ctx, false)
                            && paint.stroke == Stroke::NONE)),
                        fill == theme::unit_separator(&ctx)
                    );
                    assert!(hovered.iter().any(
                        |shape| matches!(&shape.shape, egui::Shape::Rect(paint)
                        if paint.rect == rect && paint.fill == fill && paint.stroke == Stroke::NONE
                            && paint.corner_radius == egui::CornerRadius::same(5))
                    ));
                }
            }
        }
        assert!(
            checked_icons.into_iter().all(|checked| checked),
            "Inspect both active icon textures"
        );
        // Inspect the shared ruler painter used by Timeline and Editor.
        app.zoom = 70.0;
        app.session.project.transport.r#loop.start_frame = frames(0.5);
        app.session.project.transport.r#loop.end_frame = frames(3.0);
        for looping in [false, true] {
            app.session.project.transport.r#loop.enabled = looping;
            let mut ruler = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| ruler = app.time_ruler(ui),
            );
            let (left, right) = app.loop_pixels(ruler);
            let band = Rect::from_min_max(
                Pos2::new(left, ruler.top()),
                Pos2::new(right, ruler.bottom() - 20.0),
            );
            let fill = if looping {
                egui::Color32::from_rgb(208, 172, 60)
            } else {
                theme::TOOLBAR_HOVER_FILL
            };
            let handle = if looping {
                loop_fill
            } else {
                theme::toggle_colors(&ctx, false).1
            };
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(paint) if paint.rect == band && paint.fill == fill)));
            for x in [left, right] {
                let bounds = Rect::from_min_max(
                    Pos2::new(x - 1.5, band.top() + 2.0),
                    Pos2::new(x + 1.5, band.bottom() - 2.0),
                );
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(paint) if paint.rect == bounds && paint.fill == handle)));
            }
            if looping {
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == "2"
                        && band.contains(text.pos)
                        && text.galley.job.sections.iter().all(|section|
                            section.format.color == egui::Color32::BLACK))));
            }
            output.textures_delta.clear();
        }
    }
}

#[test]
fn transport_controls_keep_pointer_keyboard_and_disabled_actions() {
    for enabled in [false, true] {
        for available in [false, true] {
            let ctx = context();
            let state = [false, false, available, available];
            let (buttons, _, _, _) = controls_frame(&ctx, state, vec![], enabled);
            let points = [
                buttons.play.rect.center(),
                buttons.stop.rect.center(),
                buttons.looping.rect.center(),
            ];
            for (index, point) in points.into_iter().enumerate() {
                controls_frame(
                    &ctx,
                    state,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                    enabled,
                );
                let (buttons, _, _, _) =
                    controls_frame(&ctx, state, vec![button(point, false)], enabled);
                let responses = [buttons.play, buttons.stop, buttons.looping];
                for (which, response) in responses.iter().enumerate() {
                    assert_eq!(
                        response.clicked(),
                        enabled && (available || index == 1) && which == index
                    );
                }
            }
        }
    }
    let ctx = context();
    let state = [true, true, true, true];
    let (buttons, _, _, _) = controls_frame(&ctx, state, vec![], true);
    buttons.play.request_focus();
    let (buttons, _, _, _) = controls_frame(
        &ctx,
        state,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        true,
    );
    assert!(buttons.play.clicked());
    assert!(!buttons.stop.clicked());
    assert!(!buttons.looping.clicked());
}

#[test]
fn transport_loop_button_keeps_validity_dirty_state_and_workspace_geometry() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let workspace = app.effects_bounds.toolbar;
    assert_eq!(
        app.transport_bounds[0].left(),
        app.effects_bounds.library_toggle.left()
    );
    assert_eq!(app.transport_bounds[0].width(), app.fit_all_bounds.width());
    let point = app.transport_bounds[2].center();
    app.session.project.transport.r#loop.start_frame = 0;
    app.session.project.transport.r#loop.end_frame = 0;
    app.dirty = false;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(!app.session.project.transport.r#loop.enabled);
    assert!(!app.dirty);
    assert!(app.error.is_some());
    app.error = None;
    end_click_sequence(&mut app, &ctx);
    app.session.project.transport.r#loop.end_frame = frames(1.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(app.session.project.transport.r#loop.enabled);
    assert!(app.dirty);
    assert_eq!(app.effects_bounds.toolbar, workspace);
    assert!(
        app.transport_bounds
            .iter()
            .all(|rect| rect.size() == Vec2::new(30.0, 24.0))
    );
    end_click_sequence(&mut app, &ctx);
    app.dirty = false;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(!app.session.project.transport.r#loop.enabled);
    assert!(app.dirty);
}

#[test]
fn loop_bar_clicks_toggle_on_release_in_both_views_without_changing_the_range() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_view_mode(view, &ctx);
        frame(&mut app, &ctx, vec![]);
        app.set_view_zoom(70.0);
        app.set_view_scroll(1.0);
        app.session.project.transport.playhead_frame = frames(8.0);
        app.session.project.transport.r#loop.start_frame = frames(2.0);
        app.session.project.transport.r#loop.end_frame = frames(5.0);
        frame(&mut app, &ctx, vec![]);
        let ruler = if view == ViewMode::Timeline {
            let lane = app.lane_bounds[&track];
            Rect::from_min_size(
                Pos2::new(lane.left(), lane.top() - RULER_HEIGHT),
                Vec2::new(lane.width(), RULER_HEIGHT),
            )
        } else {
            app.midi_editor.ruler_bounds
        };
        let point =
            |seconds: f32| Pos2::new(ruler.left() + (seconds - 1.0) * 70.0, ruler.top() + 9.0);
        // Click both handles and the body. Small pointer motion stays a click.
        for seconds in [1.98, 3.5, 5.01] {
            let start = point(seconds);
            let end = start + Vec2::new(0.5, 0.0);
            let before = app.session.project.transport.r#loop.enabled;
            app.dirty = false;
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            assert_eq!(app.session.project.transport.r#loop.enabled, before);
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            frame(&mut app, &ctx, vec![button(end, false)]);
            let region = &app.session.project.transport.r#loop;
            assert_eq!(region.enabled, !before);
            assert_eq!(
                (region.start_frame, region.end_frame),
                (frames(2.0), frames(5.0))
            );
            assert_eq!(app.session.project.transport.playhead_frame, frames(8.0));
            assert!(app.dirty);
            assert!(app.error.is_none());
        }
        let state = |app: &DawUi| {
            let region = &app.session.project.transport.r#loop;
            (region.enabled, region.start_frame, region.end_frame)
        };
        let before = state(&app);
        app.dirty = false;
        let blank = point(7.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(blank), button(blank, true)],
        );
        frame(&mut app, &ctx, vec![button(blank, false)]);
        assert_eq!(state(&app), before);
        assert!(!app.dirty);

        // A real drag with no horizontal displacement must not become a toggle.
        let start = point(3.5);
        let end = start + Vec2::new(0.0, 15.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app), before);
        assert!(!app.dirty);

        // Becoming busy between press and release cancels click activation.
        let (sender, receiver) = mpsc::channel();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        app.job = Some(receiver);
        frame(&mut app, &ctx, vec![button(start, false)]);
        assert_eq!(state(&app), before);
        assert!(!app.dirty);
        drop(sender);
    }
}

#[test]
fn stop_control_disables_origin_and_keeps_paused_stop_available_in_both_themes() {
    use transport_controls::StopAction;
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let ctx = context();
        let mut app = DawUi::default();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        for (playing, pending, position, expected) in [
            (true, true, 0, StopAction::Stop),
            (false, true, 400, StopAction::Stop), // Paused: still return to playback start.
            (false, true, 0, StopAction::Stop),   // Paused after seeking to timeline zero.
            (false, false, 400, StopAction::ReturnToOrigin),
            (false, false, 0, StopAction::AtOrigin),
        ] {
            let action = StopAction::from_transport(playing, pending, position);
            assert_eq!(action, expected);
            for enabled in [true, false] {
                let state = [playing, false, true, true];
                let (buttons, _, _, _) =
                    controls_frame_with_stop(&ctx, state, action, vec![], enabled);
                assert_eq!(
                    buttons.stop.enabled(),
                    enabled && action != StopAction::AtOrigin
                );
                assert_eq!(buttons.stop.rect.size(), Vec2::new(30.0, 24.0));
                let point = buttons.stop.rect.center();
                controls_frame_with_stop(
                    &ctx,
                    state,
                    action,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                    enabled,
                );
                let (buttons, _, _, _) = controls_frame_with_stop(
                    &ctx,
                    state,
                    action,
                    vec![button(point, false)],
                    enabled,
                );
                assert_eq!(
                    buttons.stop.clicked(),
                    enabled && action != StopAction::AtOrigin
                );
            }
        }
    }
}

#[test]
fn stopped_transport_returns_to_origin_once_without_an_audio_device_in_every_view() {
    for view in [
        ViewMode::Timeline,
        ViewMode::Instrument,
        ViewMode::Edit,
        ViewMode::Mixer,
    ] {
        let (mut app, _) = fixture();
        let ctx = context();
        app.set_view_mode(view, &ctx);
        app.session.project.transport.playhead_frame = frames(2.0);
        app.dirty = false;
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.stop_action(),
            transport_controls::StopAction::ReturnToOrigin
        );
        let point = app.transport_bounds[1].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert_eq!(app.session.project.transport.playhead_frame, frames(2.0));
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.session.project.transport.playhead_frame, 0);
        assert_eq!(app.stop_action(), transport_controls::StopAction::AtOrigin);
        assert!(app.dirty);
        assert!(app.output.is_none());
        assert!(app.error.is_none());
        end_click_sequence(&mut app, &ctx);
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.session.project.transport.playhead_frame, 0);
        assert!(!app.dirty);
    }
}

#[test]
fn timing_section_orders_metronome_signature_tempo_and_keeps_project_and_transport_unchanged() {
    for color_theme in settings::ColorTheme::ALL {
        let (mut app, _) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        assert!(!app.metronome_enabled);
        let rect = app.metronome_bounds;
        assert_eq!(rect.size(), Vec2::new(30.0, 24.0));
        assert!((rect.center().y - app.transport_bounds[0].center().y).abs() < 0.1);
        assert!((app.time_signature_control.bounds[0].left() - rect.right() - 7.0).abs() < 0.1);
        let idle = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(idle.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(surface) if surface.rect == rect && surface.fill == theme::unit_separator(&ctx)
                && surface.corner_radius == egui::CornerRadius::same(5) && surface.stroke.is_empty()
        )));
        let point = rect.center();
        let hover = frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point)],
            Vec2::new(1280.0, 800.0),
        );
        assert!(hover.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(surface) if surface.rect == rect && surface.fill == theme::unit_header_fill(&ctx, false)
                && surface.corner_radius == egui::CornerRadius::same(5) && surface.stroke.is_empty()
        )));
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(app.metronome_enabled);
        assert!(
            app.output.is_none(),
            "enabling a stopped metronome must not open a device"
        );
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        for view in ViewMode::ALL {
            app.set_view_mode(view, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            assert!(app.metronome_enabled);
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(surface) if surface.rect == app.metronome_bounds
                    && surface.fill == theme::LOOP_COLOR && surface.corner_radius == egui::CornerRadius::same(5)
                    && surface.stroke.is_empty()
            )));
            assert!(app.metronome_bounds.right() < app.time_signature_control.bounds[0].left());
            assert!(
                (app.tempo_bounds.left() - app.time_signature_control.bounds[1].right() - 7.0)
                    .abs()
                    < 0.1
            );
            let dividers = shapes
                .iter()
                .filter(|shape| {
                    matches!(&shape.shape,
                        egui::Shape::LineSegment { points, .. }
                        if points[0].x == points[1].x
                            && points[0].x > app.musical_bounds[1].right()
                            && points[0].x < app.metronome_bounds.left()
                            && points[0].y <= app.metronome_bounds.center().y
                            && points[1].y >= app.metronome_bounds.center().y
                    )
                })
                .count();
            assert_eq!(
                dividers, 1,
                "{view:?}: one divider before the timing section"
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
}

fn meter_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn time_signature_double_click_edits_each_number_and_preserves_tempo_and_frames() {
    for (part, text, expected) in [
        (0, "7", Some(7)),
        (1, "8", Some(8)),
        (1, "5", Some(5)),
        (0, "0", None),
        (1, "65536", None),
        (1, "x", None),
    ] {
        let mut app = DawUi::default();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let original = app.session.project.clone();
        assert!(
            (app.tempo_bounds.left() - app.time_signature_control.bounds[1].right() - 7.0).abs()
                < 0.1
        );
        let point = app.time_signature_control.bounds[part].center();
        for _ in 0..2 {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(&mut app, &ctx, vec![button(point, false)]);
        }
        frame(&mut app, &ctx, vec![]);
        assert!(app.time_signature_control.active());
        assert!(ctx.text_edit_focused());
        frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
        frame(&mut app, &ctx, vec![meter_key(egui::Key::Enter)]);
        assert!(!app.time_signature_control.active());
        if let Some(value) = expected {
            let mut expected = original;
            if part == 0 {
                expected.time_signature.numerator = value;
            } else {
                expected.time_signature.denominator = value;
            }
            assert_eq!(app.session.project, expected);
            assert!(app.dirty);
            assert!(app.error.is_none(), "{:?}", app.error);
        } else {
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
            assert!(app.error.is_some());
        }
    }
}

#[test]
fn time_signature_drags_commit_on_release_and_escape_cancels() {
    for (part, cancel, expected) in [(0, false, 7), (1, false, 12), (0, true, 4), (1, true, 4)] {
        let mut app = DawUi::default();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let start = app.time_signature_control.bounds[part].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let end = start - egui::vec2(0.0, 12.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            app.session.project.time_signature,
            daw_core::TimeSignature::default()
        );
        assert!(!app.dirty);
        let preview = app.display_time_signature();
        assert_eq!(
            if part == 0 {
                preview.numerator
            } else {
                preview.denominator
            },
            if part == 0 { 7 } else { 12 }
        );
        let bars: Vec<_> = app
            .musical_timeline()
            .ticks(1000.0)
            .filter(|tick| tick.bar)
            .map(|tick| tick.beat)
            .collect();
        assert!(
            bars.iter()
                .all(|beat| beat % f64::from(preview.numerator) == 0.0)
        );
        assert_eq!(
            app.musical_timeline().beats(24_000),
            f64::from(preview.denominator) / 4.0
        );
        if cancel {
            frame(&mut app, &ctx, vec![meter_key(egui::Key::Escape)]);
            assert_eq!(
                app.display_time_signature(),
                daw_core::TimeSignature::default()
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end - egui::vec2(0.0, 12.0))],
            );
            assert_eq!(
                app.display_time_signature(),
                daw_core::TimeSignature::default()
            );
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        let sig = app.session.project.time_signature;
        assert_eq!(
            if part == 0 {
                sig.numerator
            } else {
                sig.denominator
            },
            expected
        );
        assert_eq!(app.dirty, !cancel);
    }
}

#[test]
fn time_signature_editors_keep_display_width_and_scroll_long_input() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for view in ViewMode::ALL {
            for part in 0..2 {
                let mut app = DawUi::default();
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                app.set_view_mode(view, &ctx);
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let before = app.session.project.clone();
                let idle_tempo = app.tempo_bounds;
                let idle_numbers = app.time_signature_control.bounds;
                let point = app.time_signature_control.bounds[part].center();
                for _ in 0..2 {
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(point), button(point, true)],
                    );
                    frame(&mut app, &ctx, vec![button(point, false)]);
                }
                frame(&mut app, &ctx, vec![]);
                assert!(ctx.text_edit_focused());
                for text in [None, Some("65535")] {
                    let events = text
                        .map(|text| vec![egui::Event::Text(text.into())])
                        .unwrap_or_default();
                    frame(&mut app, &ctx, events);
                    let [editor, clip, painted_text] =
                        app.time_signature_control.editor_bounds.unwrap();
                    assert_eq!(app.time_signature_control.bounds, idle_numbers);
                    assert_eq!(app.tempo_bounds, idle_tempo);
                    assert!((editor.width() - idle_numbers[part].width()).abs() < 0.1);
                    if text.is_none() {
                        assert!(
                            clip.expand(0.5).contains_rect(painted_text),
                            "{view:?}, {part}: text {painted_text:?}, clip {clip:?}"
                        );
                    } else {
                        assert!(painted_text.width() > clip.width());
                    }
                    assert!(
                        app.time_signature_control.bounds[part]
                            .expand(0.5)
                            .contains_rect(editor)
                    );
                    assert!(
                        app.metronome_bounds.right() < app.time_signature_control.bounds[0].left()
                    );
                    assert!(
                        app.time_signature_control.bounds[0].right()
                            < app.time_signature_control.bounds[1].left()
                    );
                    assert!(app.time_signature_control.bounds[1].right() < app.tempo_bounds.left());
                }
                frame(&mut app, &ctx, vec![meter_key(egui::Key::Escape)]);
                assert!(!app.time_signature_control.active());
                assert!(app.time_signature_control.editor_bounds.is_none());
                assert_eq!(app.session.project, before);
                assert!(!app.dirty);
            }
        }
    }
}
