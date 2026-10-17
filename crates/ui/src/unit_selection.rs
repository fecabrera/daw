//! Click, focus and outline rules shared by installed effect and instrument units.
use crate::{panels, selection_focus::SelectionFocus};
use egui::{Rect, Response, Ui};

pub(super) fn clicked(ui: &Ui, rect: Rect, allowed: bool) -> bool {
    allowed
        && ui.rect_contains_pointer(rect)
        && ui.input(|input| {
            input.focused && input.pointer.button_clicked(egui::PointerButton::Primary)
        })
}

pub(super) fn retain_focus(ui: &Ui, response: &Response, pane: &SelectionFocus) {
    if !pane.owns(ui.ctx()) {
        response.request_focus();
    }
}

pub(super) fn paint(ui: &Ui, rect: Rect, selected: bool) {
    if selected {
        panels::selection_border(ui, rect);
    }
}

/// Plain clicks replace a selection; Shift-click toggles membership.
pub(super) fn select<T: Eq + std::hash::Hash>(
    selected: &mut std::collections::HashSet<T>,
    unit: T,
    toggle: bool,
) {
    if toggle {
        if !selected.remove(&unit) {
            selected.insert(unit);
        }
    } else {
        selected.clear();
        selected.insert(unit);
    }
}

/// Capture the selected group in lane order, independent of click order.
pub(super) fn drag_group<T: Copy + Eq + std::hash::Hash>(
    order: &[T],
    selected: &std::collections::HashSet<T>,
    anchor: T,
) -> Vec<T> {
    if selected.contains(&anchor) {
        order
            .iter()
            .copied()
            .filter(|c| selected.contains(c))
            .collect()
    } else {
        vec![anchor]
    }
}
