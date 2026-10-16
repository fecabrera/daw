use super::instrument_catalog::{add_component, click, key};
use super::*;
fn fixture(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.selected_track = Some(track);
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
    if app.timeline_catalog_visible {
        let point = app.effects_bounds.library_toggle.center();
        click(&mut app, &ctx, point, false);
    }
    (app, ctx, layer)
}
fn label(shapes: &[egui::epaint::ClippedShape], text: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing label {text}"))
}
#[test]
fn generic_envelopes_add_independent_instances_and_keep_settings_power_and_collapse() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        add_component(&mut app, &ctx, layer, 1);
        let bounds = &app.midi_editor.layer_bounds[&layer];
        assert_eq!(
            bounds.envelope_panel.size(),
            bounds.filter_envelope_panel.size()
        );
        assert_eq!(
            bounds.envelope_panel.right() - 1.0,
            bounds.filter_envelope_panel.left()
        );
        assert_eq!(bounds.amplitude_panel, Rect::NOTHING);
        let strip = bounds.filter_envelope_stages[0];
        let point = Pos2::new(
            strip.center().x,
            strip.top() + 10.0 + knobs::CHOICE_SIZE * 0.5,
        );
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        let layer_state = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert!((layer_state.envelopes[1].settings.attack.time_ms - 6.0).abs() < 1e-6);
        assert_eq!(layer_state.envelopes[0].settings.attack.time_ms, 5.0);
        let point = app.midi_editor.layer_bounds[&layer]
            .filter_envelope_power
            .center();
        click(&mut app, &ctx, point, false);
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .envelopes[1]
                .enabled
        );
        let point = app.midi_editor.layer_bounds[&layer]
            .filter_envelope_header
            .center();
        super::midi::double_click_unit_header(&mut app, &ctx, point);
        let state = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert!(state.envelopes[1].collapsed && !state.envelopes[0].collapsed);
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let mut reopened = DawUi::default();
        reopened.session.project = serde_json::from_slice(&saved).unwrap();
        reopened.selected_track = app.selected_track;
        let fresh = context();
        reopened.set_view_mode(ViewMode::Instrument, &fresh);
        frame(&mut reopened, &fresh, vec![]);
        assert_eq!(
            reopened.session.project.tracks[0].instrument,
            app.session.project.tracks[0].instrument
        );
        assert!(!reopened.dirty);
    }
}
#[test]
fn amplitude_expands_shared_modulation_strips_without_hiding_regular_controls() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        add_component(&mut app, &ctx, layer, 2);
        app.dirty = false;
        let bounds = &app.midi_editor.layer_bounds[&layer];
        let header = bounds.amplitude_header;
        let height = bounds.amplitude_panel.height();
        assert_eq!(height, panels::unit_height(false));
        let point = Pos2::new(
            header.right() - 13.0,
            header.top() + panels::HEADER_HEIGHT * 0.5,
        );
        click(&mut app, &ctx, point, false);
        assert!(app.dirty);
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .amplitude
                .collapsed
        );
        let expanded = app.midi_editor.layer_bounds[&layer].amplitude_header;
        assert!(expanded.width() > header.width());
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        label(&shapes, "Level");
        label(&shapes, "0.0 dB");
        let add = Pos2::new(header.right() + 18.0, header.bottom() + 109.0);
        click(&mut app, &ctx, add, false);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .amplitude
                .modulations
                .len(),
            1
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let none = label(&shapes, "None");
        click(&mut app, &ctx, none, false);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let source = label(&shapes, "Layer 1 · Envelope 1");
        click(&mut app, &ctx, source, false);
        let settings = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert_eq!(
            settings.amplitude.modulations[0].source,
            Some(daw_core::EnvelopeSource {
                layer,
                envelope: settings.envelopes[0].id
            })
        );
        assert_eq!(
            app.midi_editor.layer_bounds[&layer]
                .amplitude_panel
                .height(),
            height
        );
        assert_eq!(settings.amplitude.modulations[0].amount, 0.0);
        // Destination controls keep independent route state and use the shared knob/power gestures.
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let amount = label(&shapes, "+0.00") + Vec2::new(0.0, 20.0);
        click(&mut app, &ctx, amount, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        assert!(
            (app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .amplitude
                .modulations[0]
                .amount
                - 0.01)
                .abs()
                < 1e-6
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let footer = label(&shapes, "Mod 1");
        click(&mut app, &ctx, footer - Vec2::new(32.0, 0.0), false);
        let route = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .amplitude
            .modulations[0];
        assert!(!route.enabled);
        let expanded = app.midi_editor.layer_bounds[&layer].amplitude_header;
        click(
            &mut app,
            &ctx,
            Pos2::new(expanded.right() - 20.0, add.y),
            false,
        );
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .amplitude
                .modulations
                .len(),
            2
        );
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .amplitude
                .modulations[0],
            route
        );
        let padding = footer;
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(padding),
                    egui::Event::PointerButton {
                        pos: padding,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let remove = label(&shapes, "Remove modulation");
        click(&mut app, &ctx, remove, false);
        let routes = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .amplitude
            .modulations;
        assert_eq!(routes.len(), 1);
        assert_ne!(routes[0].id, route.id);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let footer = label(&shapes, "Mod 1");
        let level = label(&shapes, "Level");
        assert!(
            (footer.y - level.y).abs() < 0.1,
            "Modulation and regular footers must align: mod {footer:?}, level {level:?}"
        );
        let compact = app.session.project.tracks[0].instrument.as_mut().unwrap();
        compact.layers[0].collapsed = true;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.midi_editor.layer_bounds[&layer]
                .amplitude_panel
                .height(),
            panels::unit_height(true)
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        label(&shapes, "Mod 1");
        label(&shapes, "Level");
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .collapsed = false;
        frame(&mut app, &ctx, vec![]);
        let saved = serde_json::to_value(&app.session.project).unwrap();
        let restored: daw_core::Project = serde_json::from_value(saved).unwrap();
        restored.validate().unwrap();
        app.dirty = false;
        let expanded = app.midi_editor.layer_bounds[&layer].amplitude_header;
        click(
            &mut app,
            &ctx,
            Pos2::new(expanded.right() - 13.0, point.y),
            false,
        );
        assert!(app.dirty);
        assert_eq!(
            app.midi_editor.layer_bounds[&layer]
                .amplitude_header
                .width(),
            header.width()
        );
        assert_eq!(
            app.midi_editor.layer_bounds[&layer]
                .amplitude_panel
                .height(),
            height
        );
    }
}
#[test]
fn auto_filter_expansion_keeps_regular_controls_and_lists_only_shared_envelopes() {
    let (mut app, ctx, layer) = fixture(settings::ColorTheme::Dark);
    add_component(&mut app, &ctx, layer, 1);
    add_component(&mut app, &ctx, layer, 1);
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .envelopes[0]
        .mode = daw_core::EnvelopeMode::Retrigger;
    let effect = daw_core::EffectInstance::auto_filter();
    app.session.project.tracks[0].effects.push(effect);
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let header = app.effects_bounds.auto_filter_units[0].header;
    let point = Pos2::new(
        header.right() - 13.0,
        header.top() + panels::HEADER_HEIGHT * 0.5,
    );
    click(&mut app, &ctx, point, false);
    assert!(app.dirty);
    assert!(app.effects_drag.is_none());
    let expanded = app.effects_bounds.auto_filter_units[0].header;
    assert!(expanded.width() > header.width());
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for text in ["Cutoff", "LFO", "Out"] {
        label(&shapes, text);
    }
    let point = Pos2::new(header.right() + 18.0, header.bottom() + 109.0);
    click(&mut app, &ctx, point, false);
    let daw_core::EffectSettings::AutoFilter(f) = &app.session.project.tracks[0].effects[0].unit
    else {
        panic!()
    };
    assert_eq!(f.modulations.len(), 1, "Add strip must create a route");
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let point = label(&shapes, "None");
    click(&mut app, &ctx, point, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(!shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Layer 1 · Envelope 2")
    ));
    let point = label(&shapes, "Layer 1 · Envelope 1");
    click(&mut app, &ctx, point, false);
    let daw_core::EffectSettings::AutoFilter(filter) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        panic!()
    };
    assert_eq!(filter.modulations.len(), 1);
    assert_eq!(filter.modulations[0].source.unwrap().layer, layer);
    app.session.project.validate().unwrap();
}

#[test]
fn modulation_visibility_is_saved_per_unit_and_follows_replacement_project_values() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 2);
        app.session.project.tracks[0].effects = vec![
            daw_core::EffectInstance::auto_filter(),
            daw_core::EffectInstance::auto_filter(),
        ];
        frame(&mut app, &ctx, vec![]);
        let closed = app.session.project.clone();
        let amplitude_width = app.midi_editor.layer_bounds[&layer]
            .amplitude_header
            .width();
        let filter_width = app.effects_bounds.auto_filter_units[0].header.width();
        for amplitude in [true, false] {
            app.dirty = false;
            let header = if amplitude {
                app.midi_editor.layer_bounds[&layer].amplitude_header
            } else {
                app.effects_bounds.auto_filter_units[0].header
            };
            click(
                &mut app,
                &ctx,
                Pos2::new(
                    header.right() - 13.0,
                    header.top() + panels::HEADER_HEIGHT * 0.5,
                ),
                false,
            );
            assert!(app.dirty);
        }
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let mut reopened = DawUi::default();
        reopened.session.project = serde_json::from_slice(&saved).unwrap();
        reopened.selected_track = app.selected_track;
        let fresh = context();
        reopened.set_settings(&fresh, app.settings);
        reopened.set_view_mode(ViewMode::Instrument, &fresh);
        frame(&mut reopened, &fresh, vec![]);
        assert!(
            reopened.midi_editor.layer_bounds[&layer]
                .amplitude_header
                .width()
                > amplitude_width
        );
        assert!(reopened.effects_bounds.auto_filter_units[0].header.width() > filter_width);
        assert_eq!(
            reopened.effects_bounds.auto_filter_units[1].header.width(),
            filter_width
        );
        assert!(!reopened.dirty);
        // A replacement with the same IDs must override any prior UI expansion state.
        reopened.session.project = closed;
        frame(&mut reopened, &fresh, vec![]);
        assert_eq!(
            reopened.midi_editor.layer_bounds[&layer]
                .amplitude_header
                .width(),
            amplitude_width
        );
        assert_eq!(
            reopened.effects_bounds.auto_filter_units[0].header.width(),
            filter_width
        );
        assert!(!reopened.dirty);
    }
}

#[test]
fn parameter_markers_open_their_own_modulation_controls_without_editing_base_values() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        add_component(&mut app, &ctx, layer, 2);
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let l = &mut instrument.layers[0];
        l.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::EnvelopeSource {
            layer,
            envelope: l.envelopes[0].id,
        };
        let mut connection = daw_core::Modulation::new(daw_core::ModulationParameter::Amplitude);
        connection.source = Some(source);
        connection.amount = 1.0;
        l.amplitude.modulations.push(connection);
        l.amplitude.level_db = -6.0;
        let mut effect = daw_core::EffectInstance::auto_filter();
        let daw_core::EffectSettings::AutoFilter(f) = &mut effect.unit else {
            panic!()
        };
        f.lfo_enabled = false;
        for parameter in daw_core::ModulationParameter::FILTER {
            let mut connection = daw_core::Modulation::new(parameter);
            connection.source = Some(source);
            connection.amount = 0.5;
            f.modulations.push(connection);
        }
        let original = f.clone();
        app.session.project.tracks[0].effects.push(effect);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let panel = app.midi_editor.layer_bounds[&layer].amplitude_panel;
        let point = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Circle(c)
                    if (c.radius - 1.5).abs() < 1e-4 && panel.contains(c.center) =>
                {
                    Some(c.center)
                }
                _ => None,
            })
            .expect("Amplitude parameter marker");
        app.dirty = false;
        click(&mut app, &ctx, point, false);
        frame(&mut app, &ctx, vec![]);
        let amp = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .amplitude;
        assert!(amp.modulation_open && !amp.collapsed && amp.enabled);
        assert_eq!(amp.level_db, -6.0);
        assert!(app.dirty);
        for (strip, row) in [(0, 1), (0, 2), (3, 0), (3, 1)] {
            let daw_core::EffectSettings::AutoFilter(f) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            f.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let knob = app.effects_bounds.auto_filter_units[0].knobs[strip][row];
            let point = knobs::modulation_marker_rect(knob).center();
            app.dirty = false;
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::AutoFilter(f) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            assert!(f.modulation_open, "marker {strip}/{row}");
            let mut expected = original.clone();
            expected.modulation_open = true;
            assert_eq!(*f, expected);
            assert!(app.dirty);
            assert!(app.effects_drag.is_none());
        }
    }
}

#[test]
fn drive_modulation_reuses_strips_markers_and_saved_visibility_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        let state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        state.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::EnvelopeSource {
            layer,
            envelope: state.envelopes[0].id,
        };
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::drive());
        frame(&mut app, &ctx, vec![]);
        let header = app.effects_bounds.drive_units[0].header;
        let height = app.effects_bounds.drive_units[0].labels[0].bottom() - header.top();
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.drive_units[0].header.width() > header.width());
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let daw_core::EffectSettings::Drive(d) = &mut app.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open);
        assert_eq!(d.modulations.len(), 1);
        assert_eq!(
            d.modulations[0].parameter,
            daw_core::ModulationParameter::Volume
        );
        assert!(d.modulations[0].source.is_none() && d.modulations[0].amount == 0.0);
        d.color_shape = daw_core::DriveColorShape::MidHump;
        d.modulations = daw_core::ModulationParameter::DRIVE
            .into_iter()
            .map(|p| {
                let mut r = daw_core::Modulation::new(p);
                r.source = Some(source);
                r.amount = p.limit() * 0.5;
                r
            })
            .collect();
        d.modulation_open = false;
        let base = d.clone();
        for (strip, row) in [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2)] {
            let daw_core::EffectSettings::Drive(d) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            d.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point =
                knobs::modulation_marker_rect(app.effects_bounds.drive_units[0].knobs[strip][row])
                    .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Drive(d) = &app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            let mut expected = base.clone();
            expected.modulation_open = true;
            assert_eq!(*d, expected);
            assert!(app.dirty && app.effects_drag.is_none());
            assert_eq!(
                app.effects_bounds.drive_units[0].labels[0].bottom() - header.top(),
                height
            );
        }
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let mut reopened = DawUi::default();
        reopened.session.project = serde_json::from_slice(&saved).unwrap();
        reopened.selected_track = app.selected_track;
        reopened.set_settings(&ctx, app.settings);
        reopened.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut reopened, &ctx, vec![]);
        let daw_core::EffectSettings::Drive(d) =
            &reopened.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open && !reopened.dirty);
        assert!(reopened.effects_bounds.drive_units[0].header.width() > header.width());
    }
}
