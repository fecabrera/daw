use super::*;
use daw_drive_plugin::{Parameter as P, PluginState};
#[test]
fn body_binds_all_visible_parameters_and_fits_declared_height() {
    let ctx = egui::Context::default();
    daw_unit_ui::theme::configure(&ctx);
    daw_unit_ui::fonts::configure(&ctx);
    let mut seen = [false; P::COUNT];
    for mode in 0..2 {
        let mut state = PluginState::default();
        if let Some(p) = P::ALL.into_iter().find(|p| p.info().key == "rate_mode") {
            state.set(p, f64::from(mode)).unwrap();
        }
        let mut settings = state.to_settings();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(WIDTH as f32, HEIGHT as f32),
                )),
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
                for (slot, response) in bounds.controls.iter().enumerate() {
                    if let Some(response) = response {
                        seen[slot + 1] = true;
                        assert!(
                            response.rect.bottom() <= HEIGHT as f32 + 1.0,
                            "{:?}: {:?}",
                            P::ALL[slot + 1],
                            response.rect
                        );
                    }
                }
            },
        );
        output.textures_delta.clear();
    }
    for p in P::ALL.into_iter().skip(1) {
        assert!(seen[p as usize - 100], "unbound parameter {p:?}");
    }
}
#[test]
fn model_queues_bound_gestures_and_meter_reset_does_not_edit_state() {
    use daw_unit_core::MeterStorage;
    let model = Model::default();
    let p = P::ALL[1];
    let v = p.denormalize(0.7).unwrap();
    assert!(model.push(&[Edit::Begin(p), Edit::Value(p, v), Edit::End(p)]));
    assert_eq!(model.pop(), Some(Edit::Begin(p)));
    assert_eq!(model.pop(), Some(Edit::Value(p, v)));
    assert_eq!(model.pop(), Some(Edit::End(p)));
    assert_eq!(model.pop(), None);
    let before = model.current();
    model.output_meter.store([1.5, 0.0, 0.0]);
    daw_drive_plugin::reset_meter(&model.output_meter, 1);
    assert_eq!(model.current(), before);
    assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.0);
    assert_eq!(model.pop(), None);
}
