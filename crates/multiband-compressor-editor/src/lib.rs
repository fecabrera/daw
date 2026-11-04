//! Native plugin bindings over the same multiband compressor body as the DAW.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;
fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_drive_plugin::Settings,
    model: &Model,
) -> body::Bounds {
    body::render(ui, settings, model)
}
fn paint_extra(_: &mut egui::Ui, _: &body::Bounds, _: &Model, _: &daw_drive_plugin::Settings) {}

#[cfg(test)]
fn paint_reference(ui: &mut egui::Ui, s: &mut daw_drive_plugin::Settings, model: &Model) {
    use daw_unit_ui::{layout::unit_control_height, meters, multiband_compressor as shared};
    let mut solos = vec![false; s.bands.len()];
    let levels: Vec<_> = s
        .bands
        .iter()
        .map(|b| {
            let slot = (b.id.as_u128() - 1) as usize;
            let meter = &model.output_meter.eq_bands[slot];
            daw_unit_ui::meters::OutputLevel::new(
                meter.readings()[0],
                Some(&meter.boost_peaks()[0]),
            )
            .with_reset_mask(&model.output_meter.band_level_resets, 1_u64 << slot)
        })
        .collect();
    let reductions: Vec<_> = s
        .bands
        .iter()
        .map(|b| {
            let m = &model.output_meter.reductions[(b.id.as_u128() - 1) as usize];
            meters::GainReduction::new(m.reading(), Some(m)).for_unit(s.enabled)
        })
        .collect();
    let level = meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let count = s.bands.len();
    egui::ScrollArea::horizontal()
        .id_salt("bands")
        .show(ui, |ui| {
            shared::show(
                ui,
                &mut s.core,
                (false, unit_control_height()),
                shared::Controls {
                    solos: &mut solos,
                    band_markers: &[],
                    output_markers: &[None, None],
                    level,
                    levels: &levels,
                    reductions: &reductions,
                    add_enabled: count < 64,
                },
            )
        });
}
#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    s: &mut daw_drive_plugin::Settings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = body::render(ui, s, &Model::default());
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
    let mut plugin = daw_drive_plugin::PluginState::default().to_settings().core;
    let builtin: daw_unit_core::MultibandCompressorSettings = Default::default();
    for (a, b) in plugin.bands.iter_mut().zip(&builtin.bands) {
        a.id = b.id;
    }
    plugin.presentation.clear();
    assert_eq!(plugin, builtin);
}
#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
mod inventory_tests {
    use super::*;
    #[test]
    fn default_viewport_contains_controls_and_add_preserves_the_existing_automation_slots() {
        let ctx = egui::Context::default();
        daw_unit_ui::fonts::configure(&ctx);
        daw_unit_ui::theme::configure(&ctx);
        let model = Model::default();
        let state = daw_drive_plugin::PluginState::default();
        let mut s = state.to_settings();
        let mut add = egui::Pos2::ZERO;
        for phase in 0..3 {
            let events = if phase == 0 {
                vec![]
            } else {
                vec![
                    egui::Event::PointerMoved(add),
                    egui::Event::PointerButton {
                        pos: add,
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: Default::default(),
                    },
                ]
            };
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, body::SIZE)),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    let bounds = body::render(ui, &mut s, &model);
                    add = bounds.sections[bounds.sections.len() - 2].center();
                    for rect in &bounds.sections {
                        assert!(
                            rect.top() >= 0.0 && rect.bottom() <= body::SIZE.y + 0.5,
                            "{rect:?}"
                        );
                    }
                    for r in bounds.controls.iter().flatten() {
                        assert!(r.rect.bottom() <= body::SIZE.y + 0.5);
                    }
                },
            );
            out.textures_delta.clear();
        }
        assert_eq!(s.bands.len(), 4);
        let after = daw_drive_plugin::PluginState::from_settings(&s).unwrap();
        assert_eq!(after.get(daw_drive_plugin::Parameter::Band4Present), 1.0);
        assert_eq!(
            after.get(daw_drive_plugin::Parameter::Band1Ratio),
            state.get(daw_drive_plugin::Parameter::Band1Ratio)
        );
        assert_eq!(
            daw_drive_plugin::PluginState::decode(&after.encode()).unwrap(),
            after
        );
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    #[test]
    fn strip_drag_commits_only_visual_order_on_release_and_escape_cancels() {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            daw_unit_ui::theme::configure(&ctx);
            daw_unit_ui::fonts::configure(&ctx);
            let model = Model::default();
            let mut settings = daw_drive_plugin::PluginState::default().to_settings();
            let mut frame = |events| {
                let mut result = None;
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, body::SIZE)),
                        focused: true,
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        result = Some(body::render(ui, &mut settings, &model));
                    },
                );
                out.textures_delta.clear();
                (
                    result.unwrap(),
                    daw_drive_plugin::PluginState::from_settings(&settings).unwrap(),
                )
            };
            let (bounds, before) = frame(vec![]);
            let start = bounds.sections[1].left_center() + egui::vec2(2.0, 0.0);
            let end = bounds.sections[3].right_center() - egui::vec2(2.0, 0.0);
            let event = |pos, pressed| egui::Event::PointerButton {
                pos,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            frame(vec![egui::Event::PointerMoved(start), event(start, true)]);
            let (_, preview) = frame(vec![egui::Event::PointerMoved(end)]);
            assert_eq!(preview, before);
            if cancel {
                frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]);
            }
            let (_, after) = frame(vec![event(end, false)]);
            if cancel {
                assert_eq!(after, before);
            } else {
                assert_eq!(&after.presentation()[..3], &[1, 2, 0]);
                assert_eq!(
                    daw_drive_plugin::Parameter::ALL.map(|p| after.get(p)),
                    daw_drive_plugin::Parameter::ALL.map(|p| before.get(p))
                );
                assert_eq!(
                    daw_drive_plugin::PluginState::decode(&after.encode()).unwrap(),
                    after
                );
            }
        }
    }
}

#[cfg(test)]
mod band_menu_tests {
    use super::*;
    use daw_drive_plugin::{Parameter as P, PluginState, Settings};
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
                result = Some(body::render(ui, s, model));
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
        let point = b.sections[1].left_top() + egui::vec2(3.0, 40.0);
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
        let before = PluginState::from_settings(&s).unwrap();
        action(&ctx, &mut s, &model, point, "Solo");
        assert!(model.output_meter.solos[0].load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(PluginState::from_settings(&s).unwrap(), before);
        let b = action(&ctx, &mut s, &model, point, "Remove band");
        assert_eq!(
            PluginState::from_settings(&s).unwrap().get(P::Band1Present),
            0.0
        );
        assert!(
            b.controls[P::Band1Present as usize - 101]
                .as_ref()
                .unwrap()
                .changed()
        );
        assert!(!model.output_meter.solos[0].load(std::sync::atomic::Ordering::Relaxed));
    }
}
