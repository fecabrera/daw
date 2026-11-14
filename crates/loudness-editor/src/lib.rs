//! Native editor bindings for the shared Loudness Meter body.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;
fn show_body(
    ui: &mut egui::Ui,
    s: &mut daw_unit_core::LoudnessSettings,
    model: &Model,
) -> body::Bounds {
    body::render(ui, s, model)
}
fn paint_extra(_: &mut egui::Ui, _: &body::Bounds, _: &Model, _: &daw_unit_core::LoudnessSettings) {
}

#[cfg(test)]
mod parity_tests;
#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;
#[cfg(test)]
fn paint_reference(ui: &mut egui::Ui, s: &mut daw_unit_core::LoudnessSettings, model: &Model) {
    daw_unit_ui::loudness::show(
        ui,
        s,
        false,
        daw_unit_ui::layout::unit_control_height(),
        Some(&model.output_meter.loudness),
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
    );
}

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::LoudnessSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = show_body(ui, settings, &Model::default());
    b.controls
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .map(|r| (daw_drive_plugin::Parameter::ALL[i + 1], r.rect))
        })
        .collect()
}
