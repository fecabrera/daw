//! Native editor bindings for OTT.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    ui: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    settings: &daw_unit_core::OttSettings,
) {
    let meter = &model.output_meter.bands;
    for i in 0..3 {
        let rail = daw_unit_ui::meters::unit_monitor_rail(
            bounds.sections[i],
            daw_unit_ui::layout::PANEL_MARGIN,
        );
        let readings = daw_unit_ui::meters::BandGainChange::new(meter.readings(), Some(meter));
        let r = daw_unit_ui::meters::gain_change(
            ui,
            rail,
            ["Low", "Mid", "High"][i],
            readings.band(i).for_unit(settings.enabled),
            settings.bands[i].enabled,
        );
        if r.clicked() {
            model
                .meter_reset
                .fetch_or(4 << i, std::sync::atomic::Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;
