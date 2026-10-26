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
