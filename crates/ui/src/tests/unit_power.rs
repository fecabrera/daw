use super::*;
use daw_core::InstrumentComponent as C;

fn click_at(app: &mut DawUi, ctx: &egui::Context, pos: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(pos), button(pos, pressed)],
        );
    }
    frame(app, ctx, vec![]);
}

fn open_menu(app: &mut DawUi, ctx: &egui::Context, pos: Pos2, label: &str) -> Pos2 {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
    let item = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "power menu action {label}, at {pos:?}; text: {:?}",
                shapes
                    .iter()
                    .filter_map(|s| if let egui::Shape::Text(t) = &s.shape {
                        Some(t.galley.text())
                    } else {
                        None
                    })
                    .collect::<Vec<_>>()
            )
        });
    for shape in &shapes {
        if let egui::Shape::Text(t) = &shape.shape
            && (t.galley.text().starts_with("Remove") || t.galley.text().starts_with("Replace"))
        {
            assert!(t.pos.y > item.top(), "power action must be first");
        }
    }
    item.center()
}

#[test]
fn component_power_menu_changes_selected_group_and_undoes_together() {
    for main in [false, true] {
        for mixed in [false, true] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            app.session.project.tracks[0].instrument =
                Some(daw_core::OscillatorSettings::default().into());
            let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
            let owner = if main {
                i.main.macro_module.add();
                i.main.envelopes.push(daw_core::EnvelopeInstance::default());
                i.main.envelopes[0].present = true;
                i.main.envelopes[0].mode = daw_core::EnvelopeMode::Retrigger;
                i.main.envelopes[0].enabled = !mixed;
                i.main.id
            } else {
                i.layers[0].amplitude.present = true;
                i.layers[0].amplitude.enabled = !mixed;
                i.layers[0].id
            };
            let first = if main {
                C::Envelope(i.main.envelopes[0].id)
            } else {
                C::Oscillators
            };
            let second = if main { C::Macros } else { C::Amplitude };
            app.select_track(track, false);
            let ctx = context();
            app.set_view_mode(ViewMode::Instrument, &ctx);
            for _ in 0..3 {
                frame(&mut app, &ctx, vec![]);
            }
            let rect = app
                .instrument_ui
                .component_bounds
                .iter()
                .find(|(o, c, _)| *o == owner && *c == first)
                .unwrap()
                .2;
            let pos = if main {
                rect.center()
            } else {
                app.midi_editor.layer_bounds[&owner]
                    .oscillator_header
                    .center()
            };
            click_at(&mut app, &ctx, pos);
            app.instrument_ui.selected_components =
                [(owner, first), (owner, second)].into_iter().collect();
            let before = app.session.project.clone();
            app.dirty = false;
            let item = open_menu(
                &mut app,
                &ctx,
                pos,
                if mixed { "Enable" } else { "Disable" },
            );
            assert!(!app.dirty);
            assert_eq!(app.instrument_ui.selected_components.len(), 2);
            click_at(&mut app, &ctx, item);
            let mut i = app.session.project.tracks[0].instrument.clone().unwrap();
            for unit in [first, second] {
                let enabled = if main {
                    crate::unit_power::bus_enabled(&mut i.main, unit)
                } else {
                    crate::unit_power::layer_enabled(&mut i.layers[0], unit)
                };
                assert_eq!(enabled.copied(), Some(mixed));
            }
            assert!(app.dirty);
            assert_eq!(app.instrument_ui.selected_components.len(), 2);
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, before);
        }
    }
}

#[test]
fn effect_power_menu_changes_group_or_unselected_target() {
    for target in ["track", "master", "layer"] {
        for selected in [false, true] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            let effects = vec![
                daw_core::EffectInstance::drive(),
                daw_core::EffectInstance::equalizer(),
            ];
            let ids: Vec<_> = effects.iter().map(|e| e.id).collect();
            let owner = app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .id;
            match target {
                "master" => app.session.project.master.effects = effects,
                "layer" => {
                    app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .layers[0]
                        .effects = effects
                }
                _ => app.session.project.tracks[0].effects = effects,
            }
            app.select_track(track, false);
            let ctx = context();
            app.set_view_mode(
                if target == "layer" {
                    ViewMode::Instrument
                } else {
                    ViewMode::Timeline
                },
                &ctx,
            );
            app.master_selected = target == "master";
            frame(&mut app, &ctx, vec![]);
            let rect = if target == "layer" {
                app.instrument_ui
                    .effect_slots
                    .iter()
                    .find(|(_, id, _)| *id == ids[0])
                    .unwrap()
                    .2
            } else {
                app.effects_bounds.chain[0].1
            };
            let pos = Pos2::new(rect.left() + 55.0, rect.top() + 12.0);
            click_at(&mut app, &ctx, pos);
            app.selected_effects = if selected {
                ids.iter().copied().collect()
            } else {
                [ids[1]].into_iter().collect()
            };
            app.instrument_ui.effects_layer = (target == "layer").then_some(owner);
            let before = app.session.project.clone();
            let item = open_menu(&mut app, &ctx, pos, "Disable");
            click_at(&mut app, &ctx, item);
            let mut effects = match target {
                "master" => app.session.project.master.effects.clone(),
                "layer" => app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers[0]
                    .effects
                    .clone(),
                _ => app.session.project.tracks[0].effects.clone(),
            };
            assert_eq!(app.selected_effects.len(), if selected { 2 } else { 1 });
            assert_eq!(
                crate::unit_power::effect_enabled(&mut effects[0].unit).copied(),
                Some(false)
            );
            assert_eq!(
                crate::unit_power::effect_enabled(&mut effects[1].unit).copied(),
                Some(!selected)
            );
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, before);
        }
    }
}

#[test]
fn every_component_menu_toggles_in_expanded_and_collapsed_rows() {
    for kind in [
        "oscillator",
        "sampler",
        "granulizer",
        "envelope",
        "lfo",
        "macro",
        "amplitude",
    ] {
        for compact in [false, true] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            let layer = &mut app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0];
            layer.collapsed = compact;
            let unit = match kind {
                "oscillator" => {
                    layer.oscillators.push(Default::default());
                    layer.oscillator_collapsed = compact;
                    C::Oscillators
                }
                "sampler" | "granulizer" => {
                    let mut sample = if kind == "granulizer" {
                        daw_core::SamplerSettings::granular()
                    } else {
                        Default::default()
                    };
                    sample.collapsed = compact;
                    layer.sampler = Some(sample);
                    C::Sampler
                }
                "envelope" => {
                    let e = daw_core::EnvelopeInstance {
                        collapsed: compact,
                        ..Default::default()
                    };
                    let id = e.id;
                    layer.envelopes.push(e);
                    C::Envelope(id)
                }
                "lfo" => {
                    let lfo = daw_core::LfoInstance {
                        collapsed: compact,
                        ..Default::default()
                    };
                    let id = lfo.id;
                    layer.lfos.push(lfo);
                    C::Lfo(id)
                }
                "macro" => {
                    layer.macro_module.add();
                    layer.macro_module.collapsed = compact;
                    C::Macros
                }
                _ => {
                    layer.amplitude.present = true;
                    layer.amplitude.collapsed = compact;
                    C::Amplitude
                }
            };
            let owner = layer.id;
            app.select_track(track, false);
            let ctx = context();
            app.set_view_mode(ViewMode::Instrument, &ctx);
            for _ in 0..3 {
                frame(&mut app, &ctx, vec![]);
            }
            for label in ["Disable", "Enable"] {
                let header = app
                    .instrument_ui
                    .component_bounds
                    .iter()
                    .rev()
                    .find(|(o, c, _)| *o == owner && *c == unit)
                    .unwrap()
                    .2;
                let before = app.session.project.clone();
                let item = open_menu(&mut app, &ctx, header.center(), label);
                assert_eq!(app.session.project, before, "opening {kind} menu");
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(item), button(item, true)],
                );
                assert_eq!(app.session.project, before, "pressing {kind} action");
                frame(&mut app, &ctx, vec![button(item, false)]);
                frame(&mut app, &ctx, vec![]);
                let layer = &mut app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0];
                assert_eq!(
                    crate::unit_power::layer_enabled(layer, unit).copied(),
                    Some(label == "Enable"),
                    "{kind}, compact={compact}"
                );
            }
        }
    }
}
