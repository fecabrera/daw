//! Native editor bindings for Multiband EQ.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::MultibandEqSettings,
    model: &Model,
) -> body::Bounds {
    let mut visible = model
        .visible_band_slots
        .load(std::sync::atomic::Ordering::Relaxed);
    let spectrum = model.output_meter.spectrum.reading();
    let result = body::render_visual(
        ui,
        settings,
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
        &mut visible,
        &std::array::from_fn(|i| {
            let m = &model.output_meter.eq_bands[i];
            daw_unit_ui::meters::OutputLevel::new(m.readings()[0], Some(&m.boost_peaks()[0]))
                .with_reset_mask(&model.output_meter.band_level_resets, 1_u64 << i)
        }),
        &model.output_meter.solos,
        Some(&spectrum),
    );
    model
        .visible_band_slots
        .store(visible, std::sync::atomic::Ordering::Relaxed);
    result
}
fn paint_extra(
    _: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    _: &daw_unit_core::MultibandEqSettings,
) {
    for (mask, response) in &bounds.readouts {
        if response.clicked() {
            model
                .meter_reset
                .fetch_or(*mask, std::sync::atomic::Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;

#[cfg(test)]
mod band_input_tests {
    use super::*;
    #[test]
    fn mode_uses_whole_keyboard_steps_and_preserves_band_identity() {
        let ctx = egui::Context::default();
        daw_unit_ui::theme::configure(&ctx);
        daw_unit_ui::fonts::configure(&ctx);
        let mut settings = daw_drive_plugin::PluginState::default().to_settings();
        let original = settings.bands[0].id;
        let mut point = egui::Pos2::ZERO;
        for phase in 0..4 {
            let events = match phase {
                1 | 2 => vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: Default::default(),
                    },
                ],
                3 => vec![egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                _ => vec![],
            };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(WIDTH as f32, HEIGHT as f32),
                    )),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ui| {
                    let bounds = body::show(
                        ui,
                        &mut settings,
                        false,
                        0.0,
                        &std::array::from_fn(|_| None),
                        daw_unit_ui::meters::OutputLevel::new(0.0, None),
                    );
                    point = bounds.controls[1].as_ref().unwrap().rect.center();
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(settings.bands[0].mode, daw_unit_core::EqBandMode::LowShelf);
        assert_eq!(settings.bands[0].id, original);
    }
}

#[cfg(test)]
fn paint_reference(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::MultibandEqSettings,
    model: &Model,
) {
    let spectrum = model.output_meter.spectrum.reading();
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let slots = body::visible(
        settings,
        model
            .visible_band_slots
            .load(std::sync::atomic::Ordering::Relaxed),
    );
    let (mut view, indices) = body::view(settings, slots);
    let levels: Vec<_> = indices
        .iter()
        .map(|i| {
            let m = &model.output_meter.eq_bands[*i];
            daw_unit_ui::meters::OutputLevel::new(m.readings()[0], Some(&m.boost_peaks()[0]))
                .with_reset_mask(&model.output_meter.band_level_resets, 1_u64 << i)
        })
        .collect();
    egui::ScrollArea::horizontal()
        .id_salt("bands")
        .show(ui, |ui| {
            daw_unit_ui::multiband_eq::show(
                ui,
                &mut view,
                (false, daw_unit_ui::layout::unit_control_height()),
                daw_unit_ui::multiband_eq::Controls {
                    graph: daw_unit_ui::eq_graph::Input {
                        live_edits: true,
                        spectrum: Some(&spectrum),
                        sample_rate_hz: spectrum.sample_rate_hz,
                        effective: &[],
                        output_gain_db: None,
                        response: daw_equalizer::band_response,
                    },
                    solos: &mut [],
                    add_enabled: indices.len() < 64,
                    markers: &[],
                    levels: &levels,
                    output_marker: None,
                    output_level: Some(level),
                },
            )
        });
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::MultibandEqSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = body::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn(|_| None),
        0.0.into(),
    );
    b.controls
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .filter(|r| r.enabled())
                .map(|r| (daw_drive_plugin::Parameter::ALL[i + 1], r.rect))
        })
        .collect()
}

#[cfg(test)]
fn assert_builtin_defaults() {
    let native = daw_drive_plugin::PluginState::default().to_settings();
    let builtin: daw_unit_core::MultibandEqSettings = Default::default();
    let (mut native, _) = body::view(&native, body::visible(&native, 1));
    for (a, b) in native.bands.iter_mut().zip(&builtin.bands) {
        a.id = b.id;
    }
    assert_eq!(native, builtin);
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use daw_drive_plugin::{Parameter as P, PluginState};
    use egui::{Event, Rect, Vec2};
    fn pointer(pos: egui::Pos2, pressed: bool, button: egui::PointerButton) -> Event {
        Event::PointerButton {
            pos,
            pressed,
            button,
            modifiers: Default::default(),
        }
    }
    fn frame(
        ctx: &egui::Context,
        settings: &mut daw_unit_core::MultibandEqSettings,
        slots: &mut u64,
        events: Vec<Event>,
    ) -> (body::Bounds, egui::FullOutput) {
        let mut result = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(body::render(ui, settings, 0.0.into(), slots, &[0.0; 64]));
            },
        );
        out.textures_delta.clear();
        (result.unwrap(), out)
    }
    #[test]
    fn add_power_and_remove_share_builtin_controls_and_keep_slot_identity() {
        let ctx = egui::Context::default();
        daw_unit_ui::fonts::configure(&ctx);
        daw_unit_ui::theme::configure(&ctx);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let mut settings = PluginState::default().to_settings();
        let ids: Vec<_> = settings.bands.iter().map(|b| b.id).collect();
        let mut slots = 1;
        let (b, _) = frame(&ctx, &mut settings, &mut slots, vec![]);
        assert_eq!(b.sections.len(), 3);
        let add = b.sections[1].center();
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![
                Event::PointerMoved(add),
                pointer(add, true, egui::PointerButton::Primary),
            ],
        );
        let (b, _) = frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![pointer(add, false, egui::PointerButton::Primary)],
        );
        assert_eq!(slots, 3);
        assert!(settings.bands[1].enabled);
        assert!(b.controls[5].as_ref().is_some_and(|r| r.changed()));
        let (b, _) = frame(&ctx, &mut settings, &mut slots, vec![]);
        assert_eq!(b.sections.len(), 4);
        let power = b.controls[5].as_ref().unwrap().rect.center();
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![
                Event::PointerMoved(power),
                pointer(power, true, egui::PointerButton::Primary),
            ],
        );
        let (b, _) = frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![pointer(power, false, egui::PointerButton::Primary)],
        );
        assert!(!settings.bands[1].enabled);
        assert_eq!(slots, 3);
        assert!(b.controls[5].as_ref().is_some_and(|r| r.changed()));
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![
                Event::PointerMoved(power),
                pointer(power, true, egui::PointerButton::Secondary),
            ],
        );
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![pointer(power, false, egui::PointerButton::Secondary)],
        );
        let (_, out) = frame(&ctx, &mut settings, &mut slots, vec![]);
        let point = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Remove band" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("built-in remove menu");
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![
                Event::PointerMoved(point),
                pointer(point, true, egui::PointerButton::Primary),
            ],
        );
        frame(
            &ctx,
            &mut settings,
            &mut slots,
            vec![pointer(point, false, egui::PointerButton::Primary)],
        );
        assert_eq!(slots, 1);
        assert_eq!(settings.bands.iter().map(|b| b.id).collect::<Vec<_>>(), ids);
        let (b, _) = frame(&ctx, &mut settings, &mut slots, vec![]);
        assert_eq!(b.sections.len(), 3);
        assert_eq!(settings.bands.len(), 64);
        assert_eq!(P::COUNT, 322);
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use daw_drive_plugin::{Parameter, PluginState};
    #[test]
    fn band_drag_preserves_automation_slots_and_persists_only_on_release() {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            daw_unit_ui::theme::configure(&ctx);
            daw_unit_ui::fonts::configure(&ctx);
            let mut settings = PluginState::default().to_settings();
            let mut slots = 7;
            let mut render = |events| {
                let mut bounds = None;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(700.0, 600.0),
                        )),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ui| {
                        bounds = Some(body::render(
                            ui,
                            &mut settings,
                            0.0.into(),
                            &mut slots,
                            &[0.0; 64],
                        ));
                    },
                );
                output.textures_delta.clear();
                (
                    bounds.unwrap(),
                    PluginState::from_settings(&settings).unwrap(),
                )
            };
            let (bounds, before) = render(vec![]);
            let start = bounds.sections[0].left_center() + egui::vec2(2.0, 0.0);
            let end = bounds.sections[2].right_center() - egui::vec2(2.0, 0.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            render(vec![egui::Event::PointerMoved(start), button(start, true)]);
            let (_, preview) = render(vec![egui::Event::PointerMoved(end)]);
            assert_eq!(preview, before);
            if cancel {
                render(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]);
            }
            let (_, after) = render(vec![button(end, false)]);
            if cancel {
                assert_eq!(after, before);
                continue;
            }
            assert_eq!(&after.presentation()[..3], &[1, 2, 0]);
            assert_eq!(
                Parameter::ALL.map(|p| after.get(p)),
                Parameter::ALL.map(|p| before.get(p))
            );
            let model = Model::default();
            model.publish(after);
            assert_eq!(model.current(), Some(after));
            assert_eq!(PluginState::decode(&after.encode()).unwrap(), after);
        }
    }
}

#[cfg(test)]
mod band_menu_tests {
    use super::*;
    use daw_drive_plugin::{Parameter as P, PluginState};
    use daw_unit_core::MultibandEqSettings as Settings;
    fn frame(
        ctx: &egui::Context,
        s: &mut Settings,
        model: &Model,
        events: Vec<egui::Event>,
    ) -> (body::Bounds, Vec<egui::epaint::ClippedShape>) {
        let mut result = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, body::SIZE)),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                let mut visible = model
                    .visible_band_slots
                    .load(std::sync::atomic::Ordering::Relaxed);
                result = Some(body::render_audition(
                    ui,
                    s,
                    0.0.into(),
                    &mut visible,
                    &[0.0.into(); 64],
                    &model.output_meter.solos,
                ));
                model
                    .visible_band_slots
                    .store(visible, std::sync::atomic::Ordering::Relaxed);
            },
        );
        out.textures_delta.clear();
        (result.unwrap(), out.shapes)
    }
    fn action(
        ctx: &egui::Context,
        s: &mut Settings,
        model: &Model,
        point: egui::Pos2,
        label: &str,
    ) -> body::Bounds {
        let button = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        for pressed in [true, false] {
            frame(
                ctx,
                s,
                model,
                vec![
                    egui::Event::PointerMoved(point),
                    button(point, egui::PointerButton::Secondary, pressed),
                ],
            );
        }
        let (_, shapes) = frame(ctx, s, model, vec![]);
        let point = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing {label}; visible text: {:?}",
                    shapes
                        .iter()
                        .filter_map(|s| match &s.shape {
                            egui::Shape::Text(t) => Some(t.galley.text()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                )
            });
        frame(
            ctx,
            s,
            model,
            vec![
                egui::Event::PointerMoved(point),
                button(point, egui::PointerButton::Primary, true),
            ],
        );
        let result = frame(
            ctx,
            s,
            model,
            vec![button(point, egui::PointerButton::Primary, false)],
        )
        .0;
        frame(ctx, s, model, vec![]);
        result
    }
    #[test]
    fn context_actions_bind_to_the_band_slot_and_solo_stays_out_of_parameter_state() {
        let ctx = egui::Context::default();
        daw_unit_ui::theme::configure(&ctx);
        daw_unit_ui::fonts::configure(&ctx);
        let model = Model::default();
        let mut s = PluginState::default().to_settings();
        let (b, _) = frame(&ctx, &mut s, &model, vec![]);
        let point = b.sections[0].left_top() + egui::vec2(3.0, 40.0);
        let b = action(&ctx, &mut s, &model, point, "Disable band");
        assert_eq!(
            PluginState::from_settings(&s).unwrap().get(P::Band1Enabled),
            0.0
        );
        assert!(
            b.controls[P::Band1Enabled as usize - 101]
                .as_ref()
                .unwrap()
                .changed()
        );
        action(&ctx, &mut s, &model, point, "Enable band");
        assert_eq!(
            PluginState::from_settings(&s).unwrap().get(P::Band1Enabled),
            1.0
        );
        let before = PluginState::from_settings(&s).unwrap();
        action(&ctx, &mut s, &model, point, "Solo");
        assert!(model.output_meter.solos[0].load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(PluginState::from_settings(&s).unwrap(), before);
        let b = action(&ctx, &mut s, &model, point, "Remove band");
        assert_eq!(
            PluginState::from_settings(&s).unwrap().get(P::Band1Enabled),
            0.0
        );
        assert!(
            b.controls[P::Band1Enabled as usize - 101]
                .as_ref()
                .unwrap()
                .changed()
        );
        assert_eq!(
            model
                .visible_band_slots
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(s.bands.len(), 64);
        assert!(!model.output_meter.solos[0].load(std::sync::atomic::Ordering::Relaxed));
    }
}

#[cfg(test)]
mod band_peak_tests {
    use super::*;
    use daw_unit_core::MeterStorage;
    use std::sync::atomic::Ordering;
    #[test]
    fn visible_band_peak_reset_targets_its_fixed_slot_without_edits_or_strip_drag() {
        let ctx = egui::Context::default();
        daw_unit_ui::theme::configure(&ctx);
        daw_unit_ui::fonts::configure(&ctx);
        let model = Model::default();
        let mut state = daw_drive_plugin::PluginState::default();
        state
            .set(daw_drive_plugin::Parameter::Band1Enabled, 0.0)
            .unwrap();
        state
            .set(daw_drive_plugin::Parameter::ALL[1 + 5 * 63], 1.0)
            .unwrap();
        let mut settings = state.to_settings();
        model
            .visible_band_slots
            .store(1_u64 << 63, Ordering::Relaxed);
        model.output_meter.eq_bands[63].store([0.8, 0.0, 0.0]);
        model.output_meter.eq_bands[63].initialize([0.1, 0.0, 0.0]);
        model.output_meter.eq_bands[0].store([0.6, 0.0, 0.0]);
        model.output_meter.output.store([0.7, 0.0, 0.0]);
        let mut frame = |events| {
            let mut b = None;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, body::SIZE)),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    b = Some(show_body(ui, &mut settings, &model));
                },
            );
            out.textures_delta.clear();
            (
                b.unwrap(),
                daw_drive_plugin::PluginState::from_settings(&settings).unwrap(),
            )
        };
        let (bounds, before) = frame(vec![]);
        let rail = daw_unit_ui::meters::unit_monitor_rail(
            bounds.sections[0],
            daw_unit_ui::layout::PANEL_MARGIN,
        );
        let pos = rail.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        let (_, after) = frame(vec![button(pos, false)]);
        assert_eq!(before, after);
        assert_eq!(model.output_meter.eq_bands[63].boost_peaks()[0].peak(), 0.0);
        assert_eq!(
            model
                .output_meter
                .band_level_resets
                .swap(0, Ordering::Relaxed),
            1_u64 << 63
        );
        assert_eq!(model.output_meter.eq_bands[0].boost_peaks()[0].peak(), 0.6);
        assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.7);
        assert_eq!(model.output_meter.eq_bands[63].readings()[0], 0.1);
        frame(vec![]);
        frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        frame(vec![egui::Event::PointerMoved(
            pos + egui::vec2(30.0, 30.0),
        )]);
        let (_, after) = frame(vec![button(pos + egui::vec2(30.0, 30.0), false)]);
        assert_eq!(before, after);
        assert!(model.pop().is_none());
    }
}

#[cfg(test)]
mod graph_binding_tests {
    use super::*;
    use daw_drive_plugin::Parameter as P;
    fn frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(WIDTH as f32, HEIGHT as f32),
                )),
                events,
                ..Default::default()
            },
            |ui| editor.show(ui),
        )
    }
    #[test]
    fn graph_drag_previews_live_with_balanced_host_gestures_and_escape_restores_values() {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            daw_unit_ui::theme::configure(&ctx);
            daw_unit_ui::fonts::configure(&ctx);
            let model = std::sync::Arc::new(Model::default());
            let mut editor = Editor::new(model.clone(), || {});
            let mut out = frame(&ctx, &mut editor, vec![]);
            let start = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "1" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            out.textures_delta.clear();
            let end = start + egui::vec2(22.0, -16.0);
            let mut out = frame(
                &ctx,
                &mut editor,
                vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            out.textures_delta.clear();
            let mut out = frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(end)]);
            out.textures_delta.clear();
            let mut events = Vec::new();
            while let Some(e) = model.pop() {
                events.push(e);
            }
            assert_eq!(events.len(), 4, "live Begin/Value for Frequency and Gain");
            assert!(!events.iter().any(|e| matches!(e, Edit::End(_))));
            if cancel {
                let mut out = frame(
                    &ctx,
                    &mut editor,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                );
                out.textures_delta.clear();
            }
            let mut out = frame(
                &ctx,
                &mut editor,
                vec![egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            out.textures_delta.clear();
            while let Some(e) = model.pop() {
                events.push(e);
            }
            for p in [P::ALL[3], P::ALL[4]] {
                let trace: Vec<_> = events
                    .iter()
                    .filter(|e| match e {
                        Edit::Begin(v) | Edit::Value(v, _) | Edit::End(v) => *v == p,
                        _ => false,
                    })
                    .copied()
                    .collect();
                assert_eq!(trace.first(), Some(&Edit::Begin(p)));
                assert_eq!(trace.last(), Some(&Edit::End(p)));
                let values: Vec<_> = trace
                    .iter()
                    .filter_map(|e| {
                        if let Edit::Value(_, v) = e {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(values[0] > p.info().default);
                if cancel {
                    assert_eq!(values.last(), Some(&p.info().default));
                    assert_eq!(trace.len(), 4);
                } else {
                    assert_eq!(trace.len(), 3);
                }
            }
        }
    }
}
