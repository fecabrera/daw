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
fn lfo_panel_uses_shared_height_power_and_seven_editable_controls_in_both_themes() {
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
                    bounds = crate::lfo_panel::unit(
                        ui,
                        lfo,
                        1,
                        panels::unit_height(compact),
                        compact,
                        Default::default(),
                    );
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
    app.instrument_catalog.expand_folders();
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

#[test]
fn lfo_modulation_expands_without_hiding_controls_and_markers_use_shared_interactions() {
    use daw_core::{
        EnvelopeMode, InstrumentSettings, Modulation, ModulationParameter as P, ModulationSourceId,
    };
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let ctx = context();
        let mut app = DawUi::default();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let mut i = InstrumentSettings::default();
        i.layers[0].envelopes[0].present = true;
        i.layers[0].envelopes[0].mode = EnvelopeMode::Legato;
        let source = ModulationSourceId {
            layer: i.layers[0].id,
            unit: i.layers[0].envelopes[0].id,
        };
        let mut lfo = LfoInstance::default();
        lfo.settings.depth = 0.3;
        lfo.settings.rate_mode = daw_core::LfoRateMode::Hz;
        for p in [P::LfoDepth, P::LfoRate] {
            lfo.modulations.push(Modulation {
                source: Some(source),
                amount: 0.5,
                ..Modulation::new(p)
            });
        }
        i.layers[0].lfos.push(lfo.clone());
        let sources = crate::modulation_panel::provider_sources(&i);
        let available = &sources[0].1;
        assert!(available.iter().all(|s| s.id.unit != lfo.id));
        let draw = |lfo: &mut LfoInstance, events: Vec<egui::Event>| {
            let mut bounds = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bounds = crate::lfo_panel::unit(
                        ui,
                        lfo,
                        1,
                        panels::unit_height(false),
                        false,
                        crate::lfo_panel::Controls {
                            sources: available,
                            tempo: 120.0,
                            live: Some([0.7, 3.0]),
                        },
                    );
                },
            );
            output.textures_delta.clear();
            bounds.unwrap()
        };
        let closed = draw(&mut lfo, vec![]);
        let regular = closed.inner.unwrap();
        let toggle = Pos2::new(
            closed.header.rect.right() - 13.5,
            closed.header.rect.center().y,
        );
        draw(
            &mut lfo,
            vec![egui::Event::PointerMoved(toggle), button(toggle, true)],
        );
        assert!(!lfo.modulation_open);
        draw(&mut lfo, vec![button(toggle, false)]);
        assert!(lfo.modulation_open && !lfo.collapsed && lfo.enabled);
        let expanded = draw(&mut lfo, vec![]);
        assert!(expanded.response.rect.width() > closed.response.rect.width() + 180.0);
        assert_eq!(expanded.inner.unwrap(), regular);
        let depth = regular[2][0].center();
        draw(
            &mut lfo,
            vec![egui::Event::PointerMoved(depth), button(depth, true)],
        );
        draw(&mut lfo, vec![button(depth, false)]);
        draw(
            &mut lfo,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!((lfo.settings.depth - 0.31).abs() < 0.0001);
        for rect in [regular[2][0], regular[1][1]] {
            lfo.modulation_open = false;
            draw(&mut lfo, vec![]);
            let marker = knobs::modulation_marker_rect(rect).center();
            let original = lfo.settings;
            draw(
                &mut lfo,
                vec![egui::Event::PointerMoved(marker), button(marker, true)],
            );
            draw(&mut lfo, vec![button(marker, false)]);
            assert!(lfo.modulation_open);
            assert_eq!(lfo.settings, original);
        }
        let expanded = draw(&mut lfo, vec![]);
        let add = Pos2::new(
            expanded.response.rect.right() - 20.0,
            expanded.response.rect.center().y,
        );
        draw(
            &mut lfo,
            vec![egui::Event::PointerMoved(add), button(add, true)],
        );
        draw(&mut lfo, vec![button(add, false)]);
        assert_eq!(lfo.modulations.len(), 3);
        assert_eq!(lfo.modulations[2].parameter, P::LfoDepth);
        assert_eq!(lfo.modulations[2].amount, 0.0);
        assert!(lfo.modulations[2].source.is_none());
    }
}
