//! Saturator editor using shared bounded gestures, appearance, and output telemetry.
use daw_unit_ui::saturator as body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    _: &mut egui::Ui,
    _: &body::Bounds,
    _: &Model,
    _: &daw_unit_core::SaturatorSettings,
) {
}

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::SaturatorSettings,
    model: &Model,
) -> body::Bounds {
    body::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn(|_| None),
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
    )
}

#[cfg(test)]
fn paint_reference(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::SaturatorSettings,
    model: &Model,
) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let _ = daw_unit_ui::saturator::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn::<_, 3, _>(|_| None),
        level,
    );
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::SaturatorSettings,
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
    let builtin: daw_unit_core::SaturatorSettings = Default::default();
    assert_eq!(native, builtin);
}

#[cfg(test)]
mod selector_gestures {
    use super::*;
    use daw_drive_plugin::{Parameter as P, PluginState};
    use egui::{Event, Rect, Vec2};
    fn event(pos: egui::Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        out.textures_delta.clear();
        out
    }
    #[test]
    fn shared_shape_and_clip_dropdowns_send_balanced_host_edits() {
        for parameter in [P::Shape, P::ClipMode] {
            let ctx = egui::Context::default();
            daw_unit_ui::fonts::configure(&ctx);
            daw_unit_ui::theme::configure(&ctx);
            ctx.all_styles_mut(|s| s.animation_time = 0.0);
            let model = std::sync::Arc::new(Model::default());
            let mut editor = Editor::new(model.clone(), || {});
            frame(&ctx, &mut editor, vec![]);
            let mut settings = PluginState::default().to_settings();
            let mut point = egui::Pos2::ZERO;
            let probe = egui::Context::default();
            daw_unit_ui::fonts::configure(&probe);
            daw_unit_ui::theme::configure(&probe);
            let mut out = probe.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(WIDTH as f32, HEIGHT as f32),
                    )),
                    ..Default::default()
                },
                |ui| {
                    point = super::probe_controls(ui, &mut settings)
                        .into_iter()
                        .find(|(p, _)| *p == parameter)
                        .unwrap()
                        .1
                        .center();
                },
            );
            out.textures_delta.clear();
            frame(
                &ctx,
                &mut editor,
                vec![Event::PointerMoved(point), event(point, true)],
            );
            frame(&ctx, &mut editor, vec![event(point, false)]);
            let label = if parameter == P::Shape {
                daw_unit_core::SaturatorShape::ALL
                    .into_iter()
                    .find(|v| *v != settings.shape)
                    .unwrap()
                    .label()
            } else {
                daw_unit_core::SaturatorClipMode::ALL
                    .into_iter()
                    .find(|v| *v != settings.clip_mode)
                    .unwrap()
                    .label()
            };
            let opened = frame(&ctx, &mut editor, vec![]);
            let point = opened
                .shapes
                .iter()
                .rev()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("shared dropdown option");
            frame(
                &ctx,
                &mut editor,
                vec![Event::PointerMoved(point), event(point, true)],
            );
            frame(&ctx, &mut editor, vec![event(point, false)]);
            assert_eq!(model.pop(), Some(Edit::Begin(parameter)));
            assert!(
                matches!(model.pop(),Some(Edit::Value(p,v)) if p==parameter && v!=PluginState::default().get(p))
            );
            assert_eq!(model.pop(), Some(Edit::End(parameter)));
            assert!(model.pop().is_none());
        }
    }
}
