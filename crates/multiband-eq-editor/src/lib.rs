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
    let result = body::render(
        ui,
        settings,
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
        &mut visible,
        &std::array::from_fn(|i| model.output_meter.eq_bands[i].readings()[0]),
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
        .map(|i| model.output_meter.eq_bands[*i].readings()[0])
        .collect();
    egui::ScrollArea::horizontal()
        .id_salt("bands")
        .show(ui, |ui| {
            daw_unit_ui::multiband_eq::show(
                ui,
                &mut view,
                (false, daw_unit_ui::layout::unit_control_height()),
                daw_unit_ui::multiband_eq::Controls {
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
