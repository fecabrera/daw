//! Native editor bindings for Chorus.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn paint_extra(
    ui: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    settings: &daw_unit_core::ChorusSettings,
) {
    let _ = (ui, bounds, model, settings);
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;
