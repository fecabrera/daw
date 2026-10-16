use super::*;
use daw_core::InstrumentComponent;

fn setup(theme: settings::ColorTheme, compact: bool) -> (DawUi, egui::Context, [Id; 2]) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.layers.push(daw_core::InstrumentLayerSettings::default());
    for l in &mut i.layers {
        l.collapsed = compact;
    }
    let a = &mut i.layers[0];
    a.oscillators = vec![Default::default(), Default::default()];
    a.envelopes[0].present = true;
    a.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
    a.lfos.push(Default::default());
    a.amplitude.present = true;
    let owners = [i.layers[0].id, i.layers[1].id];
    app.select_track(track, false);
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.timeline_catalog_visible = false;
    app.timeline_effects_visible = false;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, owners)
}
fn modifier(copy: bool) -> egui::Modifiers {
    egui::Modifiers {
        alt: copy,
        ctrl: copy,
        ..Default::default()
    }
}
fn event(pos: Pos2, pressed: bool, copy: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: modifier(copy),
    }
}
fn source(app: &DawUi, owner: Id, c: InstrumentComponent) -> Pos2 {
    let r = app
        .instrument_ui
        .component_bounds
        .iter()
        .rev()
        .find(|(id, kind, _)| *id == owner && *kind == c)
        .unwrap()
        .2;
    if matches!(c, InstrumentComponent::Oscillator(_)) {
        egui::pos2(r.left() + 2.0, r.center().y)
    } else {
        egui::pos2(r.left() + 35.0, r.top() + 10.0)
    }
}
fn start(app: &mut DawUi, ctx: &egui::Context, p: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(p), event(p, true, false)],
    );
    assert!(app.component_drag.is_some(), "component press at {p:?}");
}
#[test]
fn installed_component_headers_and_source_gaps_move_and_copy_on_release() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for kind in 0..5 {
                for copy in [false, true] {
                    let (mut app, ctx, owners) = setup(theme, compact);
                    // Keep all five components visible without horizontal scrolling.
                    if kind != 2 {
                        app.session.project.tracks[0]
                            .instrument
                            .as_mut()
                            .unwrap()
                            .layers[0]
                            .envelopes[0]
                            .present = false;
                    }
                    frame(&mut app, &ctx, vec![]);
                    let l = &app.session.project.tracks[0]
                        .instrument
                        .as_ref()
                        .unwrap()
                        .layers[0];
                    let c = match kind {
                        0 => InstrumentComponent::Oscillators,
                        1 => InstrumentComponent::Oscillator(l.oscillators[0].id),
                        2 => InstrumentComponent::Envelope(l.envelopes[0].id),
                        3 => InstrumentComponent::Lfo(l.lfos[0].id),
                        _ => InstrumentComponent::Amplitude,
                    };
                    let p = source(&app, owners[0], c);
                    let end = app.midi_editor.layer_bounds[&owners[1]].label.center();
                    let original = serde_json::to_value(&app.session.project).unwrap();
                    start(&mut app, &ctx, p);
                    frame(
                        &mut app,
                        &ctx,
                        vec![
                            egui::Event::ModifiersChanged(modifier(copy)),
                            egui::Event::PointerMoved(end),
                        ],
                    );
                    assert_eq!(
                        app.instrument_catalog_bounds.drop_target,
                        Some(owners[1]),
                        "kind {kind} compact {compact}"
                    );
                    assert_eq!(
                        serde_json::to_value(&app.session.project).unwrap(),
                        original
                    );
                    assert!(!app.dirty);
                    frame(&mut app, &ctx, vec![event(end, false, copy)]);
                    assert!(app.component_drag.is_none());
                    assert!(app.dirty, "kind {kind} compact {compact}");
                    assert!(app.error.is_none(), "{:?}", app.error);
                    app.session.project.validate().unwrap();
                    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                    match kind {
                        0 | 1 => {
                            assert_eq!(
                                i.layers[1].oscillators.len(),
                                if kind == 0 { 2 } else { 1 }
                            );
                            assert_eq!(
                                i.layers[0].oscillators.len(),
                                if copy {
                                    2
                                } else if kind == 0 {
                                    0
                                } else {
                                    1
                                }
                            );
                        }
                        2 => {
                            assert_eq!(
                                i.layers[0].envelopes.iter().filter(|e| e.present).count(),
                                usize::from(copy)
                            );
                            assert!(i.layers[1].envelopes.iter().any(|e| e.present));
                        }
                        3 => {
                            assert_eq!(i.layers[0].lfos.len(), usize::from(copy));
                            assert_eq!(i.layers[1].lfos.len(), 1);
                        }
                        _ => {
                            assert_eq!(i.layers[0].amplitude.present, copy);
                            assert!(i.layers[1].amplitude.present);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn component_drag_cancel_invalid_drop_and_snapshot_changes_leave_project_untouched() {
    for mode in [
        "escape",
        "outside",
        "focus",
        "modal",
        "changed",
        "main-invalid",
        "same-layer",
    ] {
        let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
        let c = InstrumentComponent::Oscillators;
        let p = source(&app, owners[0], c);
        let end = match mode {
            "outside" => egui::pos2(1.0, 1.0),
            "main-invalid" => app.instrument_ui.main_bounds.label.center(),
            "same-layer" => app.midi_editor.layer_bounds[&owners[0]].label.center(),
            _ => app.midi_editor.layer_bounds[&owners[1]].label.center(),
        };
        start(&mut app, &ctx, p);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let events = match mode {
            "escape" => vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            "focus" => vec![egui::Event::WindowFocused(false)],
            "modal" => {
                app.settings_open = true;
                vec![]
            }
            "changed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .oscillators[0]
                    .transpose = 7;
                vec![]
            }
            _ => vec![],
        };
        let original = serde_json::to_value(&app.session.project).unwrap();
        if mode == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
            frame(&mut app, &ctx, events);
        }
        frame(&mut app, &ctx, vec![event(end, false, false)]);
        assert!(app.component_drag.is_none(), "{mode}");
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{mode}"
        );
        assert!(!app.dirty, "{mode}");
    }
}
#[test]
fn shared_provider_can_move_to_main_and_copy_back() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .oscillators
        .clear();
    frame(&mut app, &ctx, vec![]);
    let id = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .envelopes[0]
        .id;
    let c = InstrumentComponent::Envelope(id);
    let p = source(&app, owners[0], c);
    let end = app.instrument_ui.main_bounds.label.center();
    start(&mut app, &ctx, p);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end), event(end, false, false)],
    );
    let main = &app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .main;
    assert_eq!(main.envelopes.len(), 1);
    let mainid = main.id;
    frame(&mut app, &ctx, vec![]);
    let p = source(&app, mainid, c);
    let end = app.midi_editor.layer_bounds[&owners[1]].label.center();
    start(&mut app, &ctx, p);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(modifier(true)),
            egui::Event::PointerMoved(end),
            event(end, false, true),
        ],
    );
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.main.envelopes.len(), 1);
    assert!(i.layers[1].envelopes.iter().any(|e| e.present));
    app.session.project.validate().unwrap();
}

#[test]
fn component_header_click_selects_its_layer_without_editing_or_requiring_a_drag() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
    app.instrument_ui.selected_layer = Some(owners[1]);
    let p = source(&app, owners[0], InstrumentComponent::Oscillators);
    super::instrument_catalog::click(&mut app, &ctx, p, false);
    assert_eq!(app.instrument_ui.selected_layer, Some(owners[0]));
    assert!(app.component_drag.is_none());
    assert!(!app.dirty);
}

#[test]
fn oscillator_strip_reorder_uses_insertion_slots_without_changing_pitch_routes() {
    for compact in [false, true] {
        let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, compact);
        let l = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        let mut r = daw_core::Modulation::new(daw_core::ModulationParameter::Pitch);
        r.target = Some(l.oscillators[0].id);
        r.amount = 12.0;
        l.oscillator_modulations.push(r);
        let original = l.clone();
        frame(&mut app, &ctx, vec![]);
        let p = source(
            &app,
            owners[0],
            InstrumentComponent::Oscillator(original.oscillators[0].id),
        );
        let end = app.instrument_ui.oscillator_slots[1].2.right_center() - egui::vec2(2.0, 0.0);
        start(&mut app, &ctx, p);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0],
            original
        );
        frame(&mut app, &ctx, vec![event(end, false, false)]);
        let l = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert_eq!(
            l.oscillators,
            [
                original.oscillators[1].clone(),
                original.oscillators[0].clone()
            ]
        );
        assert_eq!(l.oscillator_modulations, original.oscillator_modulations);
        assert!(app.dirty);
    }
}

#[test]
fn shared_modulation_strips_reorder_on_amplitude_filter_drive_and_oscillator() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for kind in 0..4 {
            let (mut app, ctx, owners) = setup(theme, true);
            app.timeline_effects_visible = true;
            let l = &mut app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0];
            l.envelopes.clear();
            l.lfos.clear();
            let mut routes = (0..3)
                .map(|_| {
                    daw_core::Modulation::new(match kind {
                        0 => daw_core::ModulationParameter::Amplitude,
                        1 => daw_core::ModulationParameter::Frequency,
                        2 => daw_core::ModulationParameter::Volume,
                        _ => daw_core::ModulationParameter::Pitch,
                    })
                })
                .collect::<Vec<_>>();
            if kind == 3 {
                for r in &mut routes {
                    r.target = Some(l.oscillators[0].id);
                }
            }
            match kind {
                0 => {
                    l.oscillators.clear();
                    l.amplitude.modulation_open = true;
                    l.amplitude.modulations = routes.clone();
                }
                1 | 2 => {
                    let mut e = if kind == 1 {
                        daw_core::EffectInstance::auto_filter()
                    } else {
                        daw_core::EffectInstance::drive()
                    };
                    match &mut e.unit {
                        daw_core::EffectSettings::AutoFilter(s) => {
                            s.modulation_open = true;
                            s.modulations = routes.clone();
                        }
                        daw_core::EffectSettings::Drive(s) => {
                            s.modulation_open = true;
                            s.modulations = routes.clone();
                        }
                        _ => unreachable!(),
                    }
                    app.session.project.tracks[0].effects = vec![e];
                    app.session.project.tracks[0].effects_compact = true;
                }
                _ => {
                    l.oscillator_modulation_open = true;
                    l.oscillator_modulations = routes.clone();
                }
            }
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let first = crate::strip_reorder::bounds(&ctx, routes[0].id);
            let last = crate::strip_reorder::bounds(&ctx, routes[2].id);
            let p = egui::pos2(first.left() + 2.0, first.center().y);
            let end = egui::pos2(last.right() - 2.0, last.center().y);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(p), button(p, true)],
            );
            assert!(crate::strip_reorder::dragging(&ctx), "kind {kind}");
            assert!(!app.file_action_enabled(FileAction::Save));
            assert!(!app.view_switch_enabled());
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(!crate::strip_reorder::dragging(&ctx));
            let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
            let l = i.layers.iter().find(|l| l.id == owners[0]).unwrap();
            let actual = match kind {
                0 => l.amplitude.modulations.as_slice(),
                1 | 2 => app.session.project.tracks[0].effects[0].unit.modulations(),
                _ => &l.oscillator_modulations,
            };
            assert_eq!(actual, [routes[1], routes[2], routes[0]], "kind {kind}");
            assert!(app.dirty);
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn modulation_reorder_cancels_invalid_gestures_and_keeps_no_op_drops_clean() {
    for mode in [
        "escape",
        "outside",
        "focus",
        "modal",
        "changed",
        "hidden",
        "host",
        "same-slot",
    ] {
        let (mut app, ctx, _) = setup(settings::ColorTheme::Dark, true);
        let l = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        l.oscillators.clear();
        l.envelopes.clear();
        l.lfos.clear();
        l.amplitude.modulation_open = true;
        l.amplitude.modulations = (0..3)
            .map(|_| daw_core::Modulation::new(daw_core::ModulationParameter::Amplitude))
            .collect();
        let ids = l
            .amplitude
            .modulations
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>();
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let first = crate::strip_reorder::bounds(&ctx, ids[0]);
        let last = crate::strip_reorder::bounds(&ctx, ids[2]);
        let p = egui::pos2(first.left() + 2.0, first.center().y);
        let end = match mode {
            "outside" => egui::pos2(1.0, 1.0),
            "same-slot" => egui::pos2(first.center().x - 2.0, first.center().y),
            _ => last.right_center() - egui::vec2(2.0, 0.0),
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let mut events = vec![];
        match mode {
            "escape" => events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }),
            "modal" => app.settings_open = true,
            "changed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .amplitude
                    .modulations[0]
                    .amount = 0.5
            }
            "hidden" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .amplitude
                    .modulation_open = false
            }
            "host" => app.view_mode = ViewMode::Timeline,
            _ => {}
        }
        let original = serde_json::to_value(&app.session.project).unwrap();
        if mode == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
            frame(&mut app, &ctx, events);
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(!crate::strip_reorder::dragging(&ctx), "{mode}");
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{mode}"
        );
        assert!(!app.dirty, "{mode}");
    }
}
