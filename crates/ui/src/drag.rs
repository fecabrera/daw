//! Shared pointer gesture primitives.
pub(super) use daw_unit_ui::drag::*;

/// A whole-unit insertion boundary includes the trailing output-meter gap.
pub(super) fn unit_insertion_line(
    ui: &egui::Ui,
    slots: &[egui::Rect],
    slot: usize,
    viewport: egui::Rect,
) {
    if slot >= slots.len()
        && let Some(last) = slots.last()
    {
        let complete = egui::Rect::from_min_max(
            last.min,
            last.max + egui::vec2(crate::panels::UNIT_GAP, 0.0),
        );
        insertion_line(ui, &[complete], 1, viewport);
    } else {
        insertion_line(ui, slots, slot, viewport);
    }
}
