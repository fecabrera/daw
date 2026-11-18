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
fn instrument_right_toolbar_controls_the_mixer_and_lane_visibility_independently() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    let modulation = [instrument.lfo_lane.id, instrument.envelope_lane.id];
    let layer = instrument.layers[0].id;
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_effects_lane_visible(false);
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let selection = app.instrument_ui.selected_layer;
    let mixer_toggle = app.effects_bounds.track_mixer_toggle;
    let modulation_toggle = app.instrument_catalog_bounds.modulation_toggle;
    let output_toggle = app.instrument_catalog_bounds.output_toggle;
    assert_eq!(mixer_toggle.center().x, modulation_toggle.center().x);
    assert_eq!(modulation_toggle.center().x, output_toggle.center().x);
    assert!(mixer_toggle.bottom() < modulation_toggle.top());
    assert!(modulation_toggle.bottom() < output_toggle.top());
    assert!(app.effects_bounds.track_mixer.is_negative());
    assert!(
        modulation
            .iter()
            .all(|id| app.instrument_ui.modulation_lane_bounds.contains_key(id))
    );
    let visible_viewport = app.instrument_catalog_bounds.layer_viewport;

    let click = |app: &mut DawUi, rect: Rect| {
        let point = rect.center();
        frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(app, &ctx, vec![button(point, false)]);
    };
    click(&mut app, mixer_toggle);
    let panel = app.effects_bounds.track_mixer;
    assert!(app.view_panel_selected(ViewPanel::TrackMixer));
    assert_eq!(panel.width(), 2.0 * crate::mixer::STRIP_WIDTH);
    assert_eq!(app.mixer_bounds[&track].right(), app.master_bounds.left());
    assert!(app.midi_editor.layer_bounds[&layer].toolbar.right() <= panel.left());
    assert!(
        app.instrument_ui
            .component_viewports
            .iter()
            .all(|(_, rect)| rect.right() <= panel.left())
    );

    click(&mut app, modulation_toggle);
    assert!(!app.view_panel_selected(ViewPanel::ModulationLanes));
    assert!(app.instrument_ui.modulation_lane_bounds.is_empty());
    assert!(!app.instrument_ui.main_bounds.row.is_negative());
    click(&mut app, output_toggle);
    assert!(!app.view_panel_selected(ViewPanel::InstrumentOutput));
    assert!(app.instrument_ui.main_bounds.row.is_negative());
    assert!(app.instrument_catalog_bounds.layer_viewport.bottom() > visible_viewport.bottom());
    assert!(app.view_panel_selected(ViewPanel::TrackMixer));

    app.toggle_view_panel(ViewPanel::EffectsLane, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.strip.is_positive());
    assert!(app.instrument_catalog_bounds.output_toggle.bottom() < app.effects_bounds.strip.top());
    assert_eq!(app.effects_bounds.track_mixer_toggle, mixer_toggle);

    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert!(!app.view_panel_selected(ViewPanel::TrackMixer));
    app.toggle_view_panel(ViewPanel::ClipSettings, &ctx);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    assert!(app.view_panel_selected(ViewPanel::TrackMixer));
    assert!(!app.view_panel_selected(ViewPanel::ModulationLanes));
    assert!(!app.view_panel_selected(ViewPanel::InstrumentOutput));
    app.toggle_view_panel(ViewPanel::ModulationLanes, &ctx);
    app.toggle_view_panel(ViewPanel::InstrumentOutput, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(
        modulation
            .iter()
            .all(|id| app.instrument_ui.modulation_lane_bounds.contains_key(id))
    );
    assert!(!app.instrument_ui.main_bounds.row.is_negative());
    assert_eq!(app.instrument_ui.selected_layer, selection);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn instrument_output_toggle_hides_the_beat_parent_lane() {
    let mut app = DawUi::default();
    let beat = app.session.project.add_beat_track().unwrap();
    let output = app.session.project.beat(beat).unwrap().output.id;
    let ctx = context();
    app.select_track(beat, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_some());
    let before = app.session.project.clone();
    app.toggle_view_panel(ViewPanel::InstrumentOutput, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_none());
    assert!(app.instrument_ui.component_viewports.is_empty());
    assert_eq!(app.selected_instrument_owner(), Some(output));
    app.toggle_view_panel(ViewPanel::InstrumentOutput, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_some());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
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
            [
                false, false, true, true, true, true, true, true, true, false, false,
            ],
        ),
        (
            ViewMode::Instrument,
            [
                true, true, true, false, false, true, true, false, false, true, true,
            ],
        ),
        (
            ViewMode::Edit,
            [
                true, false, true, true, false, false, true, false, false, false, false,
            ],
        ),
        (
            ViewMode::Mixer,
            [
                false, false, true, false, false, false, true, true, true, false, false,
            ],
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
