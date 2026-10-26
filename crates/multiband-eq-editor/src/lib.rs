//! Native editor bindings for Multiband EQ.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    ui: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    settings: &daw_unit_core::MultibandEqSettings,
) {
    let _ = (ui, bounds, model, settings);
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
