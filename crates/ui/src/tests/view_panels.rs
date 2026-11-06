use super::*;

fn menu_frame(app: &mut DawUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| app.view_menu(ui),
    );
    output.textures_delta.clear();
    output
}

fn label(output: &egui::FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}

#[test]
fn panel_menu_availability_and_checks_follow_the_current_view() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    for (view, enabled) in [
        (
            ViewMode::Timeline,
            [false, false, true, true, true, true, true, false, false],
        ),
        (
            ViewMode::Instrument,
            [true, true, true, false, false, false, true, false, false],
        ),
        (
            ViewMode::Edit,
            [true, false, true, true, false, false, true, false, false],
        ),
        (
            ViewMode::Mixer,
            [false, false, true, false, false, false, true, true, true],
        ),
    ] {
        app.set_view_mode(view, &ctx);
        for (panel, available) in ViewPanel::ALL.into_iter().zip(enabled) {
            assert_eq!(
                app.view_panel_enabled(panel, &ctx),
                available,
                "{view:?}: {panel:?}"
            );
            if !available {
                assert!(!app.view_panel_selected(panel));
                app.toggle_view_panel(panel, &ctx);
                assert!(!app.view_panel_selected(panel));
            }
        }
    }
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.set_effects_catalog_visible(true);
    app.set_view_mode(ViewMode::Mixer, &ctx);
    app.set_effects_catalog_visible(false);
    assert!(!app.view_panel_selected(ViewPanel::EffectsCatalog));
    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    app.settings_open = true;
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    for panel in ViewPanel::ALL {
        assert!(!app.view_panel_enabled(panel, &ctx));
        let selected = app.view_panel_selected(panel);
        app.toggle_view_panel(panel, &ctx);
        assert_eq!(app.view_panel_selected(panel), selected);
    }
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn panel_menu_clicks_share_catalog_exclusivity_and_toolbar_state() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.toggle_view_panel(ViewPanel::Components, &ctx);
    assert!(app.view_panel_selected(ViewPanel::Components));
    assert!(!app.view_panel_selected(ViewPanel::EffectsCatalog));
    let output = menu_frame(&mut app, &ctx, vec![]);
    let point = label(&output, "Effects Catalog");
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    assert!(!app.view_panel_selected(ViewPanel::Components));
    app.set_view_mode(ViewMode::Mixer, &ctx);
    let before = app.session.project.clone();
    let output = menu_frame(&mut app, &ctx, vec![]);
    let point = label(&output, "Returns");
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(!app.mixer_returns_visible);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn panel_items_have_outer_dividers_between_mixer_and_fullscreen() {
    let (mut app, _) = fixture();
    let ctx = context();
    menu_frame(&mut app, &ctx, vec![]);
    let output = menu_frame(&mut app, &ctx, vec![]);
    let mixer = label(&output, "Mixer");
    let first = label(&output, ViewPanel::ALL[0].label());
    let last = label(&output, ViewPanel::ALL.last().unwrap().label());
    let fullscreen = label(&output, "Enter Full Screen");
    let dividers: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, .. }
                if points[0].y == points[1].y && (points[1].x - points[0].x).abs() > 100.0 =>
            {
                Some(points[0].y)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dividers.len(), 2);
    assert!(mixer.y < dividers[0] && dividers[0] < first.y);
    assert!(last.y < dividers[1] && dividers[1] < fullscreen.y);
    let mut previous = mixer.y;
    for panel in ViewPanel::ALL {
        let point = label(&output, panel.label());
        assert!(point.y > previous);
        previous = point.y;
    }
    let point = fullscreen;
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    let output = menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Fullscreen(true)))
    );
}
