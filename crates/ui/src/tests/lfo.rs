use super::instrument_catalog::{add_component, click, key};
use super::*;
use daw_unit_core::{LfoInstance, LfoPolarity};
#[test]
fn lfo_catalog_adds_independent_saved_sources_without_adding_routes_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let mut app = DawUi::default();
        let track = app.session.project.add_midi_track().unwrap();
        app.select_track(track, false);
        let layer = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .id;
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        add_component(&mut app, &ctx, layer, 2);
        add_component(&mut app, &ctx, layer, 2);
        let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(instrument.layers[0].lfos.len(), 2);
        assert_ne!(
            instrument.layers[0].lfos[0].id,
            instrument.layers[0].lfos[1].id
        );
        assert!(instrument.layers[0].amplitude.modulations.is_empty());
        assert!(app.session.project.tracks[0].effects.is_empty());
        let sources = crate::modulation_panel::sources(Some(instrument));
        assert_eq!(sources.len(), 2);
        assert!(
            sources
                .iter()
                .all(|s| s.scope == daw_unit_core::ModulationScope::Shared)
        );
        assert_eq!(sources[0].label, "Layer 1 · LFO 1");
        let old = instrument.clone();
        let saved = serde_json::to_value(&app.session.project).unwrap();
        let loaded: daw_core::Project = serde_json::from_value(saved).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.tracks[0].instrument.as_ref().unwrap(), &old);
        assert!(app.dirty);
    }
}
#[test]
fn lfo_panel_uses_shared_height_power_and_six_editable_controls_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let ctx = context();
        let mut app = DawUi::default();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        let mut lfo = LfoInstance::default();
        let mut bounds = None;
        let mut draw = |lfo: &mut LfoInstance, compact: bool, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bounds =
                        crate::lfo_panel::unit(ui, lfo, 1, panels::unit_height(compact), compact);
                },
            );
            out.textures_delta.clear();
            bounds.take().unwrap()
        };
        let unit = draw(&mut lfo, false, vec![]);
        assert_eq!(unit.response.rect.height(), panels::unit_height(false));
        let knobs = unit.inner.unwrap();
        assert!(knobs.iter().flatten().all(|r| r.is_positive()));
        assert_eq!(knobs[0][0].center().y, knobs[1][0].center().y);
        let phase = knobs[0][1].center();
        draw(
            &mut lfo,
            false,
            vec![egui::Event::PointerMoved(phase), button(phase, true)],
        );
        draw(&mut lfo, false, vec![button(phase, false)]);
        draw(
            &mut lfo,
            false,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!((lfo.settings.phase_degrees - 1.0).abs() < 0.0001);
        let power = unit.power.rect.center();
        draw(
            &mut lfo,
            false,
            vec![egui::Event::PointerMoved(power), button(power, true)],
        );
        draw(&mut lfo, false, vec![button(power, false)]);
        assert!(!lfo.enabled);
        let compact = draw(&mut lfo, true, vec![]);
        assert_eq!(compact.response.rect.height(), panels::unit_height(true));
        assert!(
            compact
                .inner
                .unwrap()
                .iter()
                .flatten()
                .all(|r| *r == Rect::NOTHING)
        );
        lfo.collapsed = true;
        let collapsed = draw(&mut lfo, false, vec![]);
        assert!(collapsed.inner.is_none());
        assert_eq!(collapsed.response.rect.height(), panels::unit_height(false));
        lfo.settings.polarity = LfoPolarity::Bipolar;
        assert!(lfo.settings.valid());
    }
}
#[test]
fn lfo_catalog_enter_and_drag_share_the_existing_component_actions() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.select_track(track, false);
    let ctx = context();
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let point = app.instrument_catalog_bounds.toggle.center();
    click(&mut app, &ctx, point, false);
    let row = app.instrument_catalog_bounds.entries[2].center();
    click(&mut app, &ctx, row, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .lfos
            .len(),
        1
    );
    // Same shared catalog press/drag protocol as all other instrument components.
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(row), button(row, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(row + Vec2::new(20.0, 0.0))],
    );
    let layer = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    let end = app.midi_editor.layer_bounds[&layer].label.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .lfos
            .len(),
        2
    );
}
