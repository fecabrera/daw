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
pub(super) fn label(shapes: &[egui::epaint::ClippedShape], text: &str) -> Pos2 {
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
        add_component(&mut app, &ctx, layer, 3);
        app.dirty = false;
        let bounds = &app.midi_editor.layer_bounds[&layer];
        let header = bounds.amplitude_header;
        let height = bounds.amplitude_panel.height();
        assert_eq!(header.width(), midi_editor::UNIT_STRIP_WIDTH);
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
            Some(daw_core::ModulationSourceId {
                layer,
                unit: settings.envelopes[0].id
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
        let amount = label(&shapes, "+0.0%") + Vec2::new(0.0, 20.0);
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
                - (1.0 - 10.0_f32.powf(-0.1 / 20.0)))
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
        assert!((level.x - header.left() - midi_editor::UNIT_STRIP_WIDTH * 0.5).abs() < 1.0);
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
        let level = label(&shapes, "Level");
        assert!((level.x - header.left() - midi_editor::UNIT_STRIP_WIDTH * 0.5).abs() < 1.0);
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
        add_component(&mut app, &ctx, layer, 3);
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
        add_component(&mut app, &ctx, layer, 3);
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let l = &mut instrument.layers[0];
        l.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::ModulationSourceId {
            layer,
            unit: l.envelopes[0].id,
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
        for parameter in daw_core::ModulationParameter::AUTO_FILTER {
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
        for (strip, row) in [(0, 1), (0, 2), (3, 0), (3, 1), (2, 0), (2, 1), (2, 2)] {
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
        let source = daw_core::ModulationSourceId {
            layer,
            unit: state.envelopes[0].id,
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

#[test]
fn oscillator_pitch_gain_and_level_use_shared_modulation_controls_markers_and_saved_expansion() {
    for (theme, parameter, whole_unit) in [settings::ColorTheme::Dark, settings::ColorTheme::Light]
        .into_iter()
        .flat_map(|theme| {
            [
                (daw_core::ModulationParameter::Pitch, false),
                (daw_core::ModulationParameter::OscillatorGain, false),
                (daw_core::ModulationParameter::Level, true),
                (daw_core::ModulationParameter::Pitch, true),
            ]
            .map(|(parameter, whole_unit)| (theme, parameter, whole_unit))
        })
    {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 0);
        add_component(&mut app, &ctx, layer, 0);
        add_component(&mut app, &ctx, layer, 1);
        let header = app.midi_editor.layer_bounds[&layer].oscillator_header;
        let regular = app.midi_editor.oscillator_strip_bounds.clone();
        app.dirty = false;
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        assert!(app.dirty);
        let l = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert!(l.oscillator_modulation_open && l.oscillator_enabled && !l.oscillator_collapsed);
        assert_eq!(app.midi_editor.oscillator_strip_bounds, regular);
        assert!(
            app.midi_editor.layer_bounds[&layer]
                .oscillator_header
                .width()
                > header.width()
        );
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let l = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert_eq!(l.oscillator_modulations.len(), 1);
        assert_eq!(
            l.oscillator_modulations[0].parameter,
            daw_core::ModulationParameter::Pitch
        );
        assert_eq!(
            l.oscillator_modulations[0].target,
            Some(l.oscillators[0].id)
        );
        // The same parameter picker presents concrete instances by stable ID.
        click(
            &mut app,
            &ctx,
            Pos2::new(
                header.right() + 45.0,
                header.bottom()
                    + 10.0
                    + knobs::CHOICE_SIZE * 1.5
                    + f32::from(theme::TOOLBAR_PADDING),
            ),
            false,
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let second = label(
            &shapes,
            &if whole_unit {
                parameter.label().into()
            } else {
                format!("Osc 2 · {}", parameter.label())
            },
        );
        click(&mut app, &ctx, second, false);
        let l = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        assert_eq!(
            l.oscillator_modulations[0].target,
            if whole_unit {
                None
            } else {
                Some(l.oscillators[1].id)
            }
        );
        assert_eq!(l.oscillator_modulations[0].parameter, parameter);
        l.oscillator_modulations[0].source = Some(daw_core::ModulationSourceId {
            layer: l.id,
            unit: l.envelopes[0].id,
        });
        l.oscillator_modulations[0].amount = 0.25;
        l.oscillator_modulation_open = false;
        let original = l.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let second = if whole_unit {
            app.midi_editor.oscillator_output_bounds
        } else {
            app.midi_editor.oscillator_strip_bounds[1]
        };
        let marker = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Circle(c)
                    if (c.radius - 1.5).abs() < 1e-4 && second.contains(c.center) =>
                {
                    Some(c.center)
                }
                _ => None,
            })
            .expect("Oscillator assignment marker");
        click(&mut app, &ctx, marker, false);
        let l = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        let mut expected = original;
        expected.oscillator_modulation_open = true;
        assert_eq!(l, &expected);
        let saved = serde_json::to_value(&app.session.project).unwrap();
        let restored: daw_core::Project = serde_json::from_value(saved).unwrap();
        restored.validate().unwrap();
        assert_eq!(
            restored.tracks[0].instrument,
            app.session.project.tracks[0].instrument
        );
    }
}

#[test]
fn envelope_first_column_timing_knob_and_mode_dropdown_save_time_and_note_values() {
    use daw_core::{EnvelopeMode, EnvelopeTiming};
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        let stages = app.midi_editor.layer_bounds[&layer].envelope_stages;
        let y = stages[0].top() + 10.0 + knobs::CHOICE_SIZE * 0.5;
        let timing = Pos2::new(stages[0].left() - 0.5 * midi_editor::UNIT_STRIP_WIDTH, y);
        let height = app.midi_editor.layer_bounds[&layer].envelope_panel.height();
        click(&mut app, &ctx, timing, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let mode = label(&shapes, "Per note");
        assert!((mode.x - timing.x).abs() < midi_editor::UNIT_STRIP_WIDTH * 0.5);
        assert!(mode.y > timing.y && mode.y < timing.y + knobs::CHOICE_SIZE * 1.5);
        click(&mut app, &ctx, mode, false);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let retrigger = label(&shapes, "Retrigger");
        click(&mut app, &ctx, retrigger, false);
        let attack = Pos2::new(stages[0].center().x, y);
        click(&mut app, &ctx, attack, false);
        key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
        let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
        let envelope = &instrument.layers[0].envelopes[0];
        assert_eq!(envelope.mode, EnvelopeMode::Retrigger);
        assert_eq!(envelope.settings.timing, EnvelopeTiming::Note);
        assert!((envelope.settings.attack_note - 0.99).abs() < 1e-6);
        assert_eq!(envelope.settings.attack.time_ms, 5.0);
        assert_eq!(
            height,
            app.midi_editor.layer_bounds[&layer].envelope_panel.height()
        );
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let reopened: daw_core::Project = serde_json::from_slice(&saved).unwrap();
        assert_eq!(
            reopened.tracks[0].instrument,
            app.session.project.tracks[0].instrument
        );
        click(&mut app, &ctx, timing, false);
        key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
        let envelope = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0];
        assert_eq!(envelope.settings.timing, EnvelopeTiming::Time);
        assert_eq!(envelope.settings.attack.time_ms, 5.0);
        assert!((envelope.settings.attack_note - 0.99).abs() < 1e-6);
    }
}

#[test]
fn eq_modulation_reuses_strips_markers_and_saved_visibility_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        let state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        state.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::ModulationSourceId {
            layer,
            unit: state.envelopes[0].id,
        };
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::equalizer());
        frame(&mut app, &ctx, vec![]);
        let header = app.effects_bounds.units[0].header;
        let height = app.effects_bounds.units[0].bands[0].bottom() - header.top();
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.units[0].header.width() > header.width());
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let daw_core::EffectSettings::Eq3(d) = &mut app.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open);
        assert_eq!(d.modulations.len(), 1);
        assert_eq!(
            d.modulations[0].parameter,
            daw_core::ModulationParameter::EqLowGain
        );
        assert!(d.modulations[0].source.is_none() && d.modulations[0].amount == 0.0);
        d.modulations = daw_core::ModulationParameter::EQUALIZER
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
        for (strip, row) in [(0, 0), (0, 1), (0, 2), (1, 0), (2, 0), (2, 1), (2, 2)] {
            let daw_core::EffectSettings::Eq3(d) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            d.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point =
                knobs::modulation_marker_rect(app.effects_bounds.units[0].knobs[strip][row])
                    .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Eq3(d) = &app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            let mut expected = base.clone();
            expected.modulation_open = true;
            assert_eq!(*d, expected);
            assert!(app.dirty && app.effects_drag.is_none());
            assert_eq!(
                app.effects_bounds.units[0].bands[0].bottom() - header.top(),
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
        let daw_core::EffectSettings::Eq3(d) = &reopened.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open && !reopened.dirty);
        assert!(reopened.effects_bounds.units[0].header.width() > header.width());
    }
}

#[test]
fn eq_modulation_compact_headers_keep_shared_footers_without_knobs_or_level_rails() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let mut effect = daw_core::EffectInstance::equalizer();
        let daw_core::EffectSettings::Eq3(s) = &mut effect.unit else {
            unreachable!()
        };
        s.modulation_open = true;
        s.modulations.push(daw_core::Modulation::new(
            daw_core::ModulationParameter::EqHighQ,
        ));
        app.session.project.tracks[0].effects.push(effect);
        app.session.project.tracks[0].effects_compact = true;
        frame(&mut app, &ctx, vec![]);
        let bounds = &app.effects_bounds.units[0];
        assert!(bounds.header.width() > 3.0 * midi_editor::UNIT_STRIP_WIDTH);
        assert!(bounds.knobs.iter().flatten().all(|r| !r.is_positive()));
        assert!(bounds.meters.iter().all(|r| !r.is_positive()));
        assert!(bounds.bands.iter().all(|r| r.is_positive()));
    }
}

#[test]
fn compressor_modulation_reuses_strips_markers_and_saved_visibility_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        add_component(&mut app, &ctx, layer, 1);
        let state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        state.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::ModulationSourceId {
            layer,
            unit: state.envelopes[0].id,
        };
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::compressor());
        frame(&mut app, &ctx, vec![]);
        let header = app.effects_bounds.compressor_units[0].header;
        let height = app.effects_bounds.compressor_units[0].sections[0].bottom() - header.top();
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.compressor_units[0].header.width() > header.width());
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let daw_core::EffectSettings::Compressor(d) =
            &mut app.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open);
        assert_eq!(d.modulations.len(), 1);
        assert_eq!(
            d.modulations[0].parameter,
            daw_core::ModulationParameter::CompressorThreshold
        );
        assert!(d.modulations[0].source.is_none() && d.modulations[0].amount == 0.0);
        d.modulations = daw_core::ModulationParameter::COMPRESSOR
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
        for (strip, row) in [(0, 0), (0, 1), (1, 0), (1, 1), (1, 2), (2, 0), (2, 1)] {
            let daw_core::EffectSettings::Compressor(d) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            d.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point = knobs::modulation_marker_rect(
                app.effects_bounds.compressor_units[0].controls[strip][row],
            )
            .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Compressor(d) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            let mut expected = base.clone();
            expected.modulation_open = true;
            assert_eq!(*d, expected);
            assert!(app.dirty && app.effects_drag.is_none());
            assert_eq!(
                app.effects_bounds.compressor_units[0].sections[0].bottom() - header.top(),
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
        let daw_core::EffectSettings::Compressor(d) =
            &reopened.session.project.tracks[0].effects[0].unit
        else {
            panic!()
        };
        assert!(d.modulation_open && !reopened.dirty);
        assert!(reopened.effects_bounds.compressor_units[0].header.width() > header.width());
    }
}

#[test]
fn compressor_modulation_preserves_compact_and_collapsed_unit_geometry() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                let (mut app, ctx, _) = fixture(theme);
                let mut effect = daw_core::EffectInstance::compressor();
                let daw_core::EffectSettings::Compressor(s) = &mut effect.unit else {
                    unreachable!()
                };
                s.modulation_open = true;
                s.collapsed = collapsed;
                s.modulations.push(daw_core::Modulation::new(
                    daw_core::ModulationParameter::CompressorThreshold,
                ));
                app.session.project.tracks[0].effects.push(effect);
                app.session.project.tracks[0].effects_compact = compact;
                frame(&mut app, &ctx, vec![]);
                let bounds = &app.effects_bounds.compressor_units[0];
                if collapsed {
                    assert!(bounds.sections.iter().all(|r| !r.is_positive()));
                } else {
                    assert!(bounds.sections.iter().all(|r| r.is_positive()));
                    assert!(bounds.header.width() > 3.0 * midi_editor::UNIT_STRIP_WIDTH);
                }
                assert!(
                    bounds
                        .controls
                        .iter()
                        .flatten()
                        .all(|r| r.is_positive() == (!compact && !collapsed))
                );
            }
        }
    }
}

#[test]
fn limiter_modulation_reuses_header_strips_markers_and_saved_compact_collapse_layout() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(color);
        let state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        state.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::ModulationSourceId {
            layer,
            unit: state.envelopes[0].id,
        };
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::limiter());
        frame(&mut app, &ctx, vec![]);
        let b = &app.effects_bounds.limiter_units[0];
        let header = b.header;
        let meter_width = b.meter.width();
        let height = b.sections[0].bottom() - header.top();
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.limiter_units[0].header.width() > header.width());
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let daw_core::EffectSettings::Limiter(s) =
            &mut app.session.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        assert!(s.modulation_open);
        assert_eq!(s.modulations.len(), 1);
        assert_eq!(
            s.modulations[0].parameter,
            daw_core::ModulationParameter::LimiterInput
        );
        assert!(s.modulations[0].source.is_none() && s.modulations[0].amount == 0.0);
        s.modulations = daw_core::ModulationParameter::LIMITER
            .into_iter()
            .map(|p| {
                let mut route = daw_core::Modulation::new(p);
                route.source = Some(source);
                route.amount = -p.limit() * 0.5;
                route
            })
            .collect();
        s.modulation_open = false;
        let base = s.clone();
        for (column, row) in [(0, 0), (0, 1), (0, 2), (1, 0)] {
            let daw_core::EffectSettings::Limiter(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point = knobs::modulation_marker_rect(
                app.effects_bounds.limiter_units[0].controls[column][row],
            )
            .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Limiter(s) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            let mut expected = base.clone();
            expected.modulation_open = true;
            assert_eq!(*s, expected);
            assert!(app.dirty && app.effects_drag.is_none());
            let b = &app.effects_bounds.limiter_units[0];
            assert_eq!(b.sections[0].bottom() - b.header.top(), height);
            assert_eq!(b.meter.width(), meter_width);
            assert_eq!(b.sections[0].width(), midi_editor::UNIT_STRIP_WIDTH);
        }
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let reopened: daw_core::Project = serde_json::from_slice(&saved).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            reopened.tracks[0].effects,
            app.session.project.tracks[0].effects
        );
        for compact in [false, true] {
            for collapsed in [false, true] {
                app.session.project.tracks[0].effects_compact = compact;
                let daw_core::EffectSettings::Limiter(s) =
                    &mut app.session.project.tracks[0].effects[0].unit
                else {
                    unreachable!()
                };
                s.collapsed = collapsed;
                frame(&mut app, &ctx, vec![]);
                let b = &app.effects_bounds.limiter_units[0];
                assert!(
                    b.controls
                        .iter()
                        .flatten()
                        .all(|r| r.is_positive() == (!compact && !collapsed))
                );
                assert_eq!(b.meter.is_positive(), !compact && !collapsed);
                assert!(b.sections.iter().all(|r| r.is_positive() == !collapsed));
            }
        }
    }
}

#[test]
fn ott_modulation_reuses_header_strips_markers_and_saved_compact_collapse_layout() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(color);
        let state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        state.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
        let source = daw_core::ModulationSourceId {
            layer,
            unit: state.envelopes[0].id,
        };
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::ott());
        frame(&mut app, &ctx, vec![]);
        let b = &app.effects_bounds.ott_units[0];
        let header = b.header;
        let meter_widths = b.meters.map(|r| r.width());
        let height = b.sections[0].bottom() - header.top();
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() - 13.0, header.center().y),
            false,
        );
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.ott_units[0].header.width() > header.width());
        click(
            &mut app,
            &ctx,
            Pos2::new(header.right() + 18.0, header.bottom() + 109.0),
            false,
        );
        let daw_core::EffectSettings::Ott(s) = &mut app.session.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        assert!(s.modulation_open);
        assert_eq!(s.modulations.len(), 1);
        assert_eq!(
            s.modulations[0].parameter,
            daw_core::ModulationParameter::OttLowFrequency
        );
        assert!(s.modulations[0].source.is_none() && s.modulations[0].amount == 0.0);
        s.modulations = daw_core::ModulationParameter::OTT
            .into_iter()
            .map(|p| {
                let mut route = daw_core::Modulation::new(p);
                route.source = Some(source);
                route.amount = -p.limit() * 0.5;
                route
            })
            .collect();
        s.modulation_open = false;
        let base = s.clone();
        let controls: Vec<_> = (0..4)
            .flat_map(|section| {
                (0..2).flat_map(move |column| (0..3).map(move |row| (section, column, row)))
            })
            .filter(|(section, column, row)| {
                crate::ott_panel::parameter_index(*section, *column, *row).is_some()
            })
            .collect();
        assert_eq!(controls.len(), 19);
        for (section, column, row) in controls {
            let daw_core::EffectSettings::Ott(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.modulation_open = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point = knobs::modulation_marker_rect(
                app.effects_bounds.ott_units[0].controls[section][column][row],
            )
            .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Ott(s) = &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            let mut expected = base.clone();
            expected.modulation_open = true;
            assert_eq!(*s, expected);
            assert!(app.dirty && app.effects_drag.is_none());
            let b = &app.effects_bounds.ott_units[0];
            assert_eq!(b.sections[0].bottom() - b.header.top(), height);
            assert_eq!(b.meters.map(|r| r.width()), meter_widths);
            assert_eq!(b.sections[3].width(), midi_editor::UNIT_STRIP_WIDTH);
        }
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let reopened: daw_core::Project = serde_json::from_slice(&saved).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            reopened.tracks[0].effects,
            app.session.project.tracks[0].effects
        );
        for compact in [false, true] {
            for collapsed in [false, true] {
                app.session.project.tracks[0].effects_compact = compact;
                let daw_core::EffectSettings::Ott(s) =
                    &mut app.session.project.tracks[0].effects[0].unit
                else {
                    unreachable!()
                };
                s.collapsed = collapsed;
                frame(&mut app, &ctx, vec![]);
                let b = &app.effects_bounds.ott_units[0];
                for (section, columns) in b.controls.iter().enumerate() {
                    for (column, rows) in columns.iter().enumerate() {
                        assert!(rows.iter().all(|r| r.is_positive()
                            == (!compact && !collapsed && (section < 3 || column == 0))));
                    }
                }
                assert!(
                    b.meters
                        .iter()
                        .all(|r| r.is_positive() == (!compact && !collapsed))
                );
                assert!(b.sections.iter().all(|r| r.is_positive() == !collapsed));
            }
        }
    }
}

#[test]
fn direction_mixer_modulation_markers_share_sources_controls_and_connection_opening() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let main = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .main;
        main.macro_module.add();
        let source = daw_core::ModulationSourceId {
            layer: main.id,
            unit: main.macro_module.controls[0].id,
        };
        let mut effect = daw_core::EffectInstance::direction_mixer();
        let daw_core::EffectSettings::DirectionMixer(s) = &mut effect.unit else {
            unreachable!()
        };
        s.modulations = daw_core::ModulationParameter::DIRECTION_MIXER
            .into_iter()
            .map(|parameter| daw_core::Modulation {
                source: Some(source),
                amount: parameter.limit() * 0.5,
                ..daw_core::Modulation::new(parameter)
            })
            .collect();
        app.session.project.tracks[0].effects.push(effect);
        app.session.project.validate().unwrap();
        for i in 0..2 {
            let daw_core::EffectSettings::DirectionMixer(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.modulation_open = false;
            let before = s.clone();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point = knobs::modulation_marker_rect(
                app.effects_bounds.direction_mixer_units[0].controls[i + 1],
            )
            .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::DirectionMixer(s) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            let mut expected = before;
            expected.modulation_open = true;
            assert_eq!(*s, expected);
            assert!(app.dirty && app.effects_drag.is_none());
        }
    }
}

#[test]
fn stereo_spread_modulation_markers_share_sources_controls_and_connection_opening() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let main = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .main;
        main.macro_module.add();
        let source = daw_core::ModulationSourceId {
            layer: main.id,
            unit: main.macro_module.controls[0].id,
        };
        let mut effect = daw_core::EffectInstance::stereo_spread();
        let daw_core::EffectSettings::StereoSpread(s) = &mut effect.unit else {
            unreachable!()
        };
        s.modulations = daw_core::ModulationParameter::STEREO_SPREAD
            .into_iter()
            .map(|parameter| daw_core::Modulation {
                source: Some(source),
                amount: parameter.limit() * 0.5,
                ..daw_core::Modulation::new(parameter)
            })
            .collect();
        app.session.project.tracks[0].effects.push(effect);
        app.session.project.validate().unwrap();
        for i in 0..4 {
            let daw_core::EffectSettings::StereoSpread(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.modulation_open = false;
            let before = s.clone();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point = knobs::modulation_marker_rect(
                app.effects_bounds.stereo_spread_units[0].controls[[0, 2, 1, 3][i]],
            )
            .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::StereoSpread(s) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            let mut expected = before;
            expected.modulation_open = true;
            assert_eq!(*s, expected);
            assert!(app.dirty && app.effects_drag.is_none());
        }
    }
}

#[test]
fn bitcrusher_modulation_markers_share_sources_controls_and_connection_opening() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let main = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .main;
        main.macro_module.add();
        let source = daw_core::ModulationSourceId {
            layer: main.id,
            unit: main.macro_module.controls[0].id,
        };
        let mut effect = daw_core::EffectInstance::bitcrusher();
        let daw_core::EffectSettings::Bitcrusher(s) = &mut effect.unit else {
            unreachable!()
        };
        s.modulations = daw_core::ModulationParameter::BITCRUSHER
            .into_iter()
            .map(|parameter| daw_core::Modulation {
                source: Some(source),
                amount: parameter.limit() * 0.5,
                ..daw_core::Modulation::new(parameter)
            })
            .collect();
        app.session.project.tracks[0].effects.push(effect);
        app.session.project.validate().unwrap();
        for i in 0..5 {
            let daw_core::EffectSettings::Bitcrusher(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.modulation_open = false;
            let before = s.clone();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let point =
                knobs::modulation_marker_rect(app.effects_bounds.bitcrusher_units[0].controls[i])
                    .center();
            click(&mut app, &ctx, point, false);
            frame(&mut app, &ctx, vec![]);
            let daw_core::EffectSettings::Bitcrusher(s) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            let mut expected = before;
            expected.modulation_open = true;
            assert_eq!(*s, expected);
            assert!(app.dirty && app.effects_drag.is_none());
        }
    }
}

#[test]
fn envelope_inputs_share_strips_markers_source_filtering_and_saved_open_state() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for main in [false, true] {
            let (mut app, ctx, layer) = fixture(theme);
            app.timeline_effects_visible = false;
            let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
            i.main.macro_module.add();
            let macro_source = daw_core::ModulationSourceId {
                layer: i.main.id,
                unit: i.main.macro_module.controls[0].id,
            };
            let owner = if main { i.main.id } else { layer };
            let mut e = daw_core::EnvelopeInstance {
                mode: if main {
                    daw_core::EnvelopeMode::Legato
                } else {
                    daw_core::EnvelopeMode::PerNote
                },
                ..Default::default()
            };
            e.modulations.push(daw_core::Modulation::new(
                daw_core::ModulationParameter::EnvelopeAttack,
            ));
            let id = e.id;
            if main {
                i.main.collapsed = false;
                i.main.envelopes.push(e);
            } else {
                i.layers[0].envelopes.push(e);
            }
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let rect = app
                .instrument_ui
                .component_slots
                .iter()
                .find(|(o, c, _)| *o == owner && *c == daw_core::InstrumentComponent::Envelope(id))
                .unwrap()
                .2;
            let toggle = Pos2::new(
                rect.right() - 13.0,
                rect.top() + panels::HEADER_HEIGHT * 0.5,
            );
            click(&mut app, &ctx, toggle, false);
            frame(&mut app, &ctx, vec![]);
            let get = |app: &DawUi| {
                app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .envelope_destinations()
                    .find(|(source, _)| source.unit == id)
                    .unwrap()
                    .1
                    .clone()
            };
            assert!(get(&app).modulation_open);
            assert!(app.dirty);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            label(&shapes, "×1");
            label(&shapes, "×1/64");
            label(&shapes, "×64");
            label(&shapes, "Source");
            label(&shapes, "Destination");
            let none = label(&shapes, "None");
            click(&mut app, &ctx, none, false);
            let popup = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let source = label(&popup, "Output · Macro 1");
            assert!(!popup.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text()==format!("{} · Envelope 1",if main {"Output"}else{"Layer 1"}))));
            click(&mut app, &ctx, source, false);
            assert_eq!(get(&app).modulations[0].source, Some(macro_source));
            let saved = serde_json::to_vec(&app.session.project).unwrap();
            let reopened: daw_core::Project = serde_json::from_slice(&saved).unwrap();
            assert_eq!(
                reopened.tracks[0].instrument,
                app.session.project.tracks[0].instrument
            );
            // A connected parameter uses the same marker interaction as other destinations.
            let rect = app
                .instrument_ui
                .component_slots
                .iter()
                .find(|(o, c, _)| *o == owner && *c == daw_core::InstrumentComponent::Envelope(id))
                .unwrap()
                .2;
            let toggle = Pos2::new(
                rect.right() - 13.0,
                rect.top() + panels::HEADER_HEIGHT * 0.5,
            );
            click(&mut app, &ctx, toggle, false);
            assert!(!get(&app).modulation_open);
            let attack = Pos2::new(
                rect.left() + 135.0,
                rect.top() + panels::HEADER_HEIGHT + 10.0 + knobs::CHOICE_SIZE * 0.5,
            );
            let marker = knobs::modulation_marker_rect(Rect::from_center_size(
                attack,
                Vec2::splat(knobs::CHOICE_SIZE),
            ))
            .center();
            click(&mut app, &ctx, marker, false);
            assert!(get(&app).modulation_open);
        }
    }
}

#[test]
fn auto_filter_lfo_targets_are_selectable_and_saved_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for parameter in [
            daw_core::ModulationParameter::AutoFilterStrength,
            daw_core::ModulationParameter::AutoFilterSyncPhase,
            daw_core::ModulationParameter::AutoFilterStereoPhase,
        ] {
            let (mut app, ctx, _) = fixture(theme);
            app.session.project.tracks[0]
                .effects
                .push(daw_core::EffectInstance::auto_filter());
            frame(&mut app, &ctx, vec![]);
            let header = app.effects_bounds.auto_filter_units[0].header;
            let daw_core::EffectSettings::AutoFilter(settings) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            settings.modulation_open = true;
            settings.modulations.push(daw_core::Modulation::new(
                daw_core::ModulationParameter::Frequency,
            ));
            frame(&mut app, &ctx, vec![]);
            click(
                &mut app,
                &ctx,
                Pos2::new(
                    header.right() + 45.0,
                    header.bottom()
                        + 10.0
                        + knobs::CHOICE_SIZE * 1.5
                        + f32::from(theme::TOOLBAR_PADDING),
                ),
                false,
            );
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let choice = label(&shapes, parameter.label());
            click(&mut app, &ctx, choice, false);
            let daw_core::EffectSettings::AutoFilter(settings) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                panic!()
            };
            assert_eq!(settings.modulations[0].parameter, parameter);
            let reopened: daw_core::Project =
                serde_json::from_value(serde_json::to_value(&app.session.project).unwrap())
                    .unwrap();
            reopened.validate().unwrap();
            assert_eq!(
                reopened.tracks[0].effects,
                app.session.project.tracks[0].effects
            );
        }
    }
}
