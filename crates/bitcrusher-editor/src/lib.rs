//! Bitcrusher editor using shared bounded gestures, appearance, and output telemetry.
use daw_unit_ui::bitcrusher as body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    _: &mut egui::Ui,
    _: &body::Bounds,
    _: &Model,
    _: &daw_unit_core::BitcrusherSettings,
) {
}

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::BitcrusherSettings,
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
    settings: &mut daw_unit_core::BitcrusherSettings,
    model: &Model,
) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let _ = daw_unit_ui::bitcrusher::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn::<_, 5, _>(|_| None),
        level,
    );
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::BitcrusherSettings,
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
    let builtin: daw_unit_core::BitcrusherSettings = Default::default();
    assert_eq!(native, builtin);
}
