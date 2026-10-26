//! Native editor bindings for Limiter.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    ui: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    settings: &daw_unit_core::LimiterSettings,
) {
    let meter = &model.output_meter.reduction;
    let strip = bounds.sections[0];
    let response = daw_unit_ui::meters::gain_reduction_rail(
        ui,
        daw_unit_ui::meters::unit_monitor_rail(strip, daw_unit_ui::layout::PANEL_MARGIN),
        daw_unit_ui::meters::GainReduction::new(meter.reading(), Some(meter))
            .for_unit(settings.enabled),
    );
    if response.clicked() {
        model
            .meter_reset
            .fetch_or(2, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;
