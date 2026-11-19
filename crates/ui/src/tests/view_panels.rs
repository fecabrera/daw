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
fn instrument_tabs_partition_units_and_keep_the_mixer_independent() {
    use crate::instrument_workspace::InstrumentSubview;
    use daw_core::InstrumentComponent;
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].amplitude.present = true;
    instrument.layers[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    instrument.layers[0].macro_module.add();
    instrument
        .main
        .effects
        .push(daw_core::EffectInstance::drive());
    instrument.main.macro_module.add();
    let [lfo, envelope] = [instrument.lfo_lane.id, instrument.envelope_lane.id];
    let [layer, output] = [instrument.layers[0].id, instrument.main.id];
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_effects_lane_visible(false);
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    assert_eq!(app.instrument_subview, InstrumentSubview::Sources);
    let expanded_height = panels::unit_height(false);
    assert_eq!(
        app.midi_editor.layer_bounds[&layer].row.height(),
        expanded_height
    );
    assert!(app.instrument_ui.modulation_lane_bounds.is_empty());
    assert!(app.instrument_ui.main_bounds.row.is_negative());
    assert!(app.instrument_ui.effect_slots.is_empty());
    assert_eq!(
        app.instrument_ui
            .component_slots
            .iter()
            .map(|(owner, component, _)| (*owner, *component))
            .collect::<Vec<_>>(),
        [
            (layer, InstrumentComponent::Oscillators),
            (layer, InstrumentComponent::Amplitude)
        ]
    );

    let click = |app: &mut DawUi, rect: Rect| {
        super::instrument_catalog::click(app, &ctx, rect.center(), false);
        frame(app, &ctx, vec![]);
    };
    let mixer_toggle = app.effects_bounds.track_mixer_toggle;
    click(&mut app, mixer_toggle);
    let panel = app.effects_bounds.track_mixer;
    assert_eq!(panel.width(), 2.0 * crate::mixer::STRIP_WIDTH);
    assert!(app.midi_editor.layer_bounds[&layer].toolbar.right() <= panel.left());

    let [sources_tab, effects_tab, modulation_tab] = app.instrument_catalog_bounds.subviews;
    for (left, right) in [(sources_tab, effects_tab), (effects_tab, modulation_tab)] {
        assert_eq!(left.y_range(), right.y_range());
        assert_eq!(
            right.left() - left.right(),
            crate::rows::GROUP_DIVIDER_WIDTH
        );
    }
    let layers_header = app.midi_editor.instrument_header_bounds;
    assert!(app.midi_editor.add_layer_bounds.is_positive());

    click(&mut app, effects_tab);
    assert_eq!(app.instrument_subview, InstrumentSubview::Effects);
    assert_eq!(app.midi_editor.instrument_header_bounds, layers_header);
    assert!(app.midi_editor.add_layer_bounds.is_positive());
    assert!(app.instrument_ui.modulation_lane_bounds.is_empty());
    assert!(!app.instrument_ui.main_bounds.row.is_negative());
    assert_eq!(app.instrument_ui.main_bounds.row.height(), expanded_height);
    assert_eq!(
        app.instrument_ui
            .effect_slots
            .iter()
            .map(|(owner, _, _)| *owner)
            .collect::<Vec<_>>(),
        [layer, output]
    );
    assert!(app.instrument_ui.component_slots.is_empty());
    assert!(app.instrument_ui.source_drop_rows.is_empty());

    click(&mut app, modulation_tab);
    assert_eq!(app.instrument_subview, InstrumentSubview::Modulation);
    assert_eq!(app.midi_editor.instrument_header_bounds, layers_header);
    assert_eq!(app.midi_editor.add_layer_bounds, Rect::NOTHING);
    let lanes = &app.instrument_ui.modulation_lane_bounds;
    assert!(lanes[&lfo].row.top() < lanes[&envelope].row.top());
    assert_eq!(lanes[&lfo].row.height(), expanded_height);
    assert_eq!(lanes[&envelope].row.height(), expanded_height);
    assert_eq!(app.instrument_ui.macro_bounds.row.height(), expanded_height);
    let macros = &app.instrument_ui.component_slots;
    assert_eq!(macros.len(), 2);
    assert_eq!(
        (macros[0].0, macros[0].1),
        (output, InstrumentComponent::Macros)
    );
    assert_eq!(
        (macros[1].0, macros[1].1),
        (layer, InstrumentComponent::Macros)
    );
    assert!(lanes[&envelope].row.top() < macros[0].2.top());
    assert!(app.instrument_ui.main_bounds.row.is_negative());
    assert!(app.instrument_ui.effect_slots.is_empty());
    click(&mut app, sources_tab);
    assert_eq!(app.instrument_subview, InstrumentSubview::Sources);

    app.toggle_view_panel(ViewPanel::EffectsLane, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.strip.is_positive());
    assert_eq!(app.effects_bounds.track_mixer_toggle, mixer_toggle);

    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert!(!app.view_panel_selected(ViewPanel::TrackMixer));
    app.set_view_mode(ViewMode::Instrument, &ctx);
    assert!(app.view_panel_selected(ViewPanel::TrackMixer));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn beat_parent_output_uses_effects_and_macros_use_modulation() {
    use crate::instrument_workspace::InstrumentSubview;
    let mut app = DawUi::default();
    let beat = app.session.project.add_beat_track().unwrap();
    let output = app.session.project.beat(beat).unwrap().output.id;
    let ctx = context();
    app.select_track(beat, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_none());
    assert!(app.instrument_ui.component_viewports.is_empty());
    app.set_instrument_subview(InstrumentSubview::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_some());
    app.set_instrument_subview(InstrumentSubview::Modulation, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.final_output.is_none());
    assert_eq!(app.selected_instrument_owner(), Some(output));
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
            [false, false, true, true, true, true, true, true, true],
        ),
        (
            ViewMode::Instrument,
            [true, true, true, false, false, true, true, false, false],
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
