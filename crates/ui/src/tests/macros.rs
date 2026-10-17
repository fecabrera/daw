use super::instrument_catalog::{add_component, click};
use super::*;
use daw_core::{MacroModuleSettings, Modulation, ModulationParameter as P, ModulationSourceId};

fn text_rect(shapes: &[egui::epaint::ClippedShape], label: &str) -> Rect {
    shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}
#[test]
fn macro_catalog_adds_multiple_named_powered_strips_to_layers_and_main() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let mut app = DawUi::default();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        let track = app.session.project.add_midi_track().unwrap();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
        let layer = i.layers[0].id;
        let main = i.main.id;
        add_component(&mut app, &ctx, layer, 4);
        add_component(&mut app, &ctx, layer, 4);
        let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(i.layers[0].macro_module.controls.len(), 2);
        let sources = crate::modulation_panel::sources(Some(i));
        assert_eq!(sources[0].label, "Layer 1 · Macro 1");
        assert_eq!(sources[1].label, "Layer 1 · Macro 2");
        assert_ne!(sources[0].id, sources[1].id);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let label = text_rect(&shapes, "Macro 2");
        let power = Pos2::new(
            label.left() - f32::from(theme::TOOLBAR_PADDING) - icons::COMPACT_BUTTON_SIZE * 0.5,
            label.center().y,
        );
        click(&mut app, &ctx, power, false);
        let m = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .macro_module;
        assert!(m.controls[0].enabled && !m.controls[1].enabled);
        let main_label = app.instrument_ui.main_bounds.label.left_top() + Vec2::new(20.0, 15.0);
        click(&mut app, &ctx, main_label, false);
        add_component(&mut app, &ctx, main, 4);
        let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(i.main.macro_module.controls.len(), 1);
        assert!(
            crate::modulation_panel::sources(Some(i))
                .iter()
                .any(|s| s.label == "Main · Macro 1")
        );
        app.session.project.validate().unwrap();
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        assert_eq!(
            serde_json::from_slice::<daw_core::Project>(&saved)
                .unwrap()
                .tracks[0]
                .instrument,
            app.session.project.tracks[0].instrument
        );
    }
}
#[test]
fn macro_panel_uses_one_knob_per_strip_plus_power_add_and_shared_route_controls() {
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
        let mut instrument = daw_core::InstrumentSettings::default();
        let layer = instrument.layers[0].id;
        let m = &mut instrument.layers[0].macro_module;
        m.add();
        m.add();
        let second = m.controls[1].id;
        m.modulations.push(Modulation {
            target: Some(second),
            amount: 0.0,
            ..Modulation::new(P::MacroValue)
        });
        let mut module = m.clone();
        let mut name_edit = None;
        let mut draw = |m: &mut MacroModuleSettings, compact, events| {
            instrument.layers[0].macro_module = m.clone();
            let sources = crate::modulation_panel::sources(Some(&instrument));
            let available = crate::modulation_panel::provider_sources(&instrument);
            let mut unit = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    unit = crate::macro_panel::unit(
                        ui,
                        m,
                        panels::unit_height(compact),
                        compact,
                        crate::macro_panel::Controls {
                            layer,
                            sources: &sources,
                            available: &available,
                            live: &[],
                            name_edit: &mut name_edit,
                        },
                    )
                },
            );
            output.textures_delta.clear();
            (unit.unwrap(), output.shapes)
        };
        let (unit, shapes) = draw(&mut module, false, vec![]);
        assert_eq!(unit.response.rect.height(), panels::unit_height(false));
        let knob_count = shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::Circle(c) if (c.radius - 9.0).abs() < 0.01 && c.center.y < unit.header.rect.bottom() + 60.0)).count();
        assert_eq!(knob_count, 2);
        let first_knob = Pos2::new(
            unit.response.rect.left() + 1.0 + midi_editor::UNIT_STRIP_WIDTH * 0.5,
            unit.header.rect.bottom() + 10.0 + knobs::CHOICE_SIZE * 0.5,
        );
        draw(
            &mut module,
            false,
            vec![
                egui::Event::PointerMoved(first_knob),
                button(first_knob, true),
            ],
        );
        draw(&mut module, false, vec![button(first_knob, false)]);
        draw(
            &mut module,
            false,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!((module.controls[0].settings.value - 0.01).abs() < 1e-6);
        assert_eq!(module.controls[1].settings.value, 0.0);
        let plus = Pos2::new(
            unit.response.rect.right() - 1.0 - (icons::COMPACT_BUTTON_SIZE + 20.0) * 0.5,
            unit.response.rect.center().y + panels::HEADER_HEIGHT * 0.5,
        );
        draw(
            &mut module,
            false,
            vec![egui::Event::PointerMoved(plus), button(plus, true)],
        );
        draw(&mut module, false, vec![button(plus, false)]);
        assert_eq!(module.controls.len(), 3);
        module.modulation_open = true;
        let (expanded, shapes) = draw(&mut module, false, vec![]);
        assert!(
            expanded.response.rect.width()
                > unit.response.rect.width() + midi_editor::UNIT_STRIP_WIDTH
        );
        assert!(text_rect(&shapes, "Source").is_positive());
        assert!(text_rect(&shapes, "Destination").is_positive());
        assert!(text_rect(&shapes, "Macro 1").is_positive());
        let (compact, shapes) = draw(&mut module, true, vec![]);
        assert_eq!(compact.response.rect.height(), panels::unit_height(true));
        assert!(text_rect(&shapes, "Macro 3").is_positive());
        module.collapsed = true;
        let (collapsed, _) = draw(&mut module, false, vec![]);
        assert!(collapsed.inner.is_none());
        module.collapsed = false;
        module.modulation_open = false;
        for control in &mut module.controls {
            control.present = false;
        }
        let (empty, _) = draw(&mut module, false, vec![]);
        assert!(empty.response.rect.width() >= midi_editor::UNIT_STRIP_WIDTH);
        assert!(empty.power.rect.right() < empty.header.rect.right() - 26.0);
    }
}
#[test]
fn macro_source_pickers_filter_cycles_per_target_without_hiding_other_strips() {
    let mut i = daw_core::InstrumentSettings::default();
    let l = &mut i.layers[0];
    l.macro_module.add();
    l.macro_module.add();
    let a = ModulationSourceId {
        layer: l.id,
        unit: l.macro_module.controls[0].id,
    };
    let b = ModulationSourceId {
        layer: l.id,
        unit: l.macro_module.controls[1].id,
    };
    l.macro_module.modulations.push(Modulation {
        source: Some(a),
        target: Some(b.unit),
        amount: 0.5,
        ..Modulation::new(P::MacroValue)
    });
    let choices = crate::modulation_panel::provider_sources(&i);
    let a_choices = &choices.iter().find(|(id, _)| *id == a).unwrap().1;
    let b_choices = &choices.iter().find(|(id, _)| *id == b).unwrap().1;
    assert!(!a_choices.iter().any(|s| s.id == a || s.id == b));
    assert!(b_choices.iter().any(|s| s.id == a));
    assert!(!b_choices.iter().any(|s| s.id == b));
}

#[test]
fn macro_strip_context_menu_removes_only_its_target_on_release() {
    for main in [false, true] {
        for compact in [false, true] {
            for hit in ["padding", "control", "power"] {
                let ctx = context();
                let mut app = DawUi::default();
                let track = app.session.project.add_midi_track().unwrap();
                let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
                instrument.layers[0].collapsed = main || compact;
                instrument.main.collapsed = !main || compact;
                let module = if main {
                    &mut instrument.main.macro_module
                } else {
                    &mut instrument.layers[0].macro_module
                };
                module.add();
                module.add();
                module.controls[1].settings.value = 0.3;
                app.select_track(track, false);
                app.set_view_mode(ViewMode::Instrument, &ctx);
                frame(&mut app, &ctx, vec![]);
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let label = text_rect(&shapes, "Macro 2");
                let power = Pos2::new(
                    label.left()
                        - f32::from(theme::TOOLBAR_PADDING)
                        - icons::COMPACT_BUTTON_SIZE * 0.5,
                    label.center().y,
                );
                let strip_center =
                    (power.x - icons::COMPACT_BUTTON_SIZE * 0.5 + label.right()) * 0.5;
                let point = match hit {
                    "padding" => Pos2::new(strip_center - 42.0, label.center().y),
                    "control" if !compact => Pos2::new(
                        strip_center,
                        label.center().y
                            - 3.0 * (knobs::CHOICE_SIZE + f32::from(theme::TOOLBAR_PADDING)),
                    ),
                    "control" => label.center(),
                    _ => power,
                };
                let before = app.session.project.tracks[0].instrument.clone().unwrap();
                for pressed in [true, false] {
                    frame(
                        &mut app,
                        &ctx,
                        vec![
                            egui::Event::PointerMoved(point),
                            egui::Event::PointerButton {
                                pos: point,
                                button: egui::PointerButton::Secondary,
                                pressed,
                                modifiers: Default::default(),
                            },
                        ],
                    );
                }
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let menu = text_rect(&shapes, "Remove macro").center();
                assert_eq!(
                    app.session.project.tracks[0].instrument.as_ref().unwrap(),
                    &before
                );
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(menu), button(menu, true)],
                );
                assert_eq!(
                    app.session.project.tracks[0].instrument.as_ref().unwrap(),
                    &before
                );
                frame(&mut app, &ctx, vec![button(menu, false)]);
                let mut expected = before;
                let module = if main {
                    &mut expected.main.macro_module
                } else {
                    &mut expected.layers[0].macro_module
                };
                module.controls[1].present = false;
                assert_eq!(
                    app.session.project.tracks[0].instrument.as_ref().unwrap(),
                    &expected
                );
                app.session.project.validate().unwrap();
            }
        }
    }
}

pub(super) fn begin_macro_rename(app: &mut DawUi, ctx: &egui::Context, name: &str) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|i| i.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    let point = text_rect(&output.shapes, name).center();
    for _ in 0..2 {
        click(app, ctx, point, false);
    }
    frame(app, ctx, vec![]);
    assert!(app.instrument_ui.macro_name_edit.is_some());
}

fn rename_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn macro_names_commit_on_enter_and_cancel_without_changing_routes_or_power() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for main in [false, true] {
            for compact in [false, true] {
                let ctx = context();
                let mut app = DawUi::default();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme,
                        ..Default::default()
                    },
                );
                let track = app.session.project.add_midi_track().unwrap();
                let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
                instrument.layers[0].collapsed = main || compact;
                instrument.main.collapsed = !main || compact;
                let (owner, module) = if main {
                    (instrument.main.id, &mut instrument.main.macro_module)
                } else {
                    (
                        instrument.layers[0].id,
                        &mut instrument.layers[0].macro_module,
                    )
                };
                module.add();
                module.add();
                module.controls[1].enabled = false;
                module.controls[1].settings.value = 0.3;
                module.modulations.push(Modulation {
                    target: Some(module.controls[1].id),
                    amount: 0.2,
                    ..Modulation::new(P::MacroValue)
                });
                let source = ModulationSourceId {
                    layer: owner,
                    unit: module.controls[1].id,
                };
                instrument.layers[0].amplitude.modulations.push(Modulation {
                    source: Some(source),
                    ..Modulation::new(P::Amplitude)
                });
                let before = instrument.clone();
                app.select_track(track, false);
                app.set_view_mode(ViewMode::Instrument, &ctx);
                frame(&mut app, &ctx, vec![]);
                app.dirty = false;
                begin_macro_rename(&mut app, &ctx, "Macro 2");
                frame(&mut app, &ctx, vec![egui::Event::Text("  Tone  ".into())]);
                assert_eq!(
                    app.session.project.tracks[0].instrument.as_ref().unwrap(),
                    &before
                );
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![rename_key(egui::Key::Enter)]);
                let mut expected = before;
                let module = if main {
                    &mut expected.main.macro_module
                } else {
                    &mut expected.layers[0].macro_module
                };
                module.controls[1].name = Some("Tone".into());
                assert_eq!(
                    app.session.project.tracks[0].instrument.as_ref().unwrap(),
                    &expected
                );
                assert!(app.dirty);
                assert!(
                    crate::modulation_panel::sources(Some(&expected))
                        .iter()
                        .any(|s| s.id == source
                            && s.label
                                == format!("{} · Tone", if main { "Main" } else { "Layer 1" }))
                );
                let saved = serde_json::to_vec(&app.session.project).unwrap();
                assert_eq!(
                    serde_json::from_slice::<daw_core::Project>(&saved)
                        .unwrap()
                        .tracks[0]
                        .instrument,
                    app.session.project.tracks[0].instrument
                );
                for cancel in [
                    "escape",
                    "blur",
                    "view",
                    "window",
                    "track",
                    "empty",
                    "unchanged",
                ] {
                    begin_macro_rename(&mut app, &ctx, "Tone");
                    app.dirty = false;
                    if cancel != "unchanged" {
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::Text(
                                if cancel == "empty" { "  " } else { "Discard" }.into(),
                            )],
                        );
                    }
                    match cancel {
                        "escape" => frame(&mut app, &ctx, vec![rename_key(egui::Key::Escape)]),
                        "empty" | "unchanged" => {
                            frame(&mut app, &ctx, vec![rename_key(egui::Key::Enter)])
                        }
                        "view" => {
                            app.set_view_mode(ViewMode::Timeline, &ctx);
                            frame(&mut app, &ctx, vec![]);
                            app.set_view_mode(ViewMode::Instrument, &ctx);
                        }
                        "track" => {
                            let other = app.session.project.add_midi_track().unwrap();
                            app.select_track(other, false);
                            frame(&mut app, &ctx, vec![]);
                            app.select_track(track, false);
                            frame(&mut app, &ctx, vec![]);
                        }
                        "window" => {
                            let mut output = ctx.run_ui(
                                egui::RawInput {
                                    focused: false,
                                    screen_rect: Some(Rect::from_min_size(
                                        Pos2::ZERO,
                                        Vec2::new(1280.0, 800.0),
                                    )),
                                    ..Default::default()
                                },
                                |ui| app.show(ui),
                            );
                            output.textures_delta.clear();
                            frame(&mut app, &ctx, vec![]);
                        }
                        _ => click(&mut app, &ctx, Pos2::new(1200.0, 750.0), false),
                    }
                    assert_eq!(
                        app.session.project.tracks[0].instrument.as_ref().unwrap(),
                        &expected,
                        "{cancel}"
                    );
                    assert!(!app.dirty, "{cancel}");
                    assert!(app.instrument_ui.macro_name_edit.is_none(), "{cancel}");
                }
                let width = app
                    .instrument_ui
                    .component_bounds
                    .iter()
                    .find(|(id, c, _)| *id == owner && *c == daw_core::InstrumentComponent::Macros)
                    .unwrap()
                    .2
                    .width();
                begin_macro_rename(&mut app, &ctx, "Tone");
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Text("A very long macro name 🎛".into())],
                );
                let editing_width = app
                    .instrument_ui
                    .component_bounds
                    .iter()
                    .find(|(id, c, _)| *id == owner && *c == daw_core::InstrumentComponent::Macros)
                    .unwrap()
                    .2
                    .width();
                assert!((width - editing_width).abs() < 0.01);
                frame(&mut app, &ctx, vec![rename_key(egui::Key::Enter)]);
                frame(&mut app, &ctx, vec![]);
                let after_width = app
                    .instrument_ui
                    .component_bounds
                    .iter()
                    .find(|(id, c, _)| *id == owner && *c == daw_core::InstrumentComponent::Macros)
                    .unwrap()
                    .2
                    .width();
                assert!((width - after_width).abs() < 0.01);
            }
        }
    }
}
