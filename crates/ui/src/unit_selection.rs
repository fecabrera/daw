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
