//! Transient Shaper editor using shared bounded gestures, appearance, and output telemetry.
use daw_unit_ui::transient_shaper as body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    _: &mut egui::Ui,
    _: &body::Bounds,
    _: &Model,
    _: &daw_unit_core::TransientShaperSettings,
) {
}

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::TransientShaperSettings,
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
    settings: &mut daw_unit_core::TransientShaperSettings,
    model: &Model,
) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let _ = daw_unit_ui::transient_shaper::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn::<_, 6, _>(|_| None),
        level,
    );
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::TransientShaperSettings,
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
            r.as_ref().filter(|r| r.enabled()).and_then(|r| {
                let parameter = daw_drive_plugin::Parameter::ALL[i + 1];
                (parameter != daw_drive_plugin::Parameter::ClipMode).then_some((parameter, r.rect))
            })
        })
        .collect()
}

#[cfg(test)]
fn assert_builtin_defaults() {
    let native = daw_drive_plugin::PluginState::default().to_settings();
    let builtin: daw_unit_core::TransientShaperSettings = Default::default();
    assert_eq!(native, builtin);
}

#[cfg(test)]
#[test]
fn clip_mode_menu_stays_in_bounds_and_sends_one_automation_gesture() {
    use daw_drive_plugin::Parameter;
    use egui::{Event, Pos2, Rect, Vec2};

    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32));
    let ctx = egui::Context::default();
    daw_unit_ui::fonts::configure(&ctx);
    daw_unit_ui::theme::configure(&ctx);
    let input = |events| egui::RawInput {
        screen_rect: Some(screen),
        events,
        ..Default::default()
    };
    let mut settings = daw_unit_core::TransientShaperSettings::default();
    let mut selector = Rect::NOTHING;
    ctx.run_ui(input(vec![]), |ui| {
        let bounds = body::show(
            ui,
            &mut settings,
            false,
            0.0,
            &[const { None }; 6],
            0.0.into(),
        );
        for response in bounds.controls.iter().flatten() {
            assert!(
                screen.contains_rect(response.rect),
                "control outside editor: {:?}",
                response.rect
            );
        }
        assert!(
            bounds
                .sections
                .windows(2)
                .all(|pair| (pair[0].right() - pair[1].left()).abs() < 0.1)
        );
        selector = bounds.controls[6].as_ref().unwrap().rect;
    })
    .textures_delta
    .clear();

    let model = std::sync::Arc::new(Model::default());
    let mut editor = Editor::new(model.clone(), || {});
    let ctx = egui::Context::default();
    daw_unit_ui::fonts::configure(&ctx);
    daw_unit_ui::theme::configure(&ctx);
    let mut frame = |events| {
        let mut output = ctx.run_ui(input(events), |ui| editor.show(ui));
        output.textures_delta.clear();
        output.shapes
    };
    let click = |pos, pressed| {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            },
        ]
    };
    frame(vec![]);
    frame(click(selector.center(), true));
    frame(click(selector.center(), false));
    let shapes = frame(vec![]);
    for mode in daw_unit_core::TransientShaperClipMode::ALL {
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == mode.label())));
    }
    let tube = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Tube" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    frame(click(tube, true));
    frame(click(tube, false));
    assert_eq!(
        std::iter::from_fn(|| model.pop()).collect::<Vec<_>>(),
        [
            Edit::Begin(Parameter::ClipMode),
            Edit::Value(Parameter::ClipMode, 3.0),
            Edit::End(Parameter::ClipMode),
        ]
    );
}
