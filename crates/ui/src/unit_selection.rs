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

/// A unit pane owns deletion keys even when a held key, modifier or drag blocks removal.
/// Text editors keep their events; repeats cannot fall through to clip shortcuts.
pub(super) fn delete_pressed(
    ui: &Ui,
    pane: &SelectionFocus,
    allowed: bool,
    dragging: bool,
) -> bool {
    if !ui.input(|input| {
        input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
    }) {
        return false;
    }
    let text_editing = ui
        .ctx()
        .memory(|memory| memory.focused())
        .is_some_and(|id| egui::TextEdit::load_state(ui.ctx(), id).is_some());
    if text_editing || pane.lost(ui.ctx(), true, false) {
        return false;
    }
    let remove = allowed && ui.is_enabled() && !dragging && !ui.input(|i| i.pointer.any_down());
    ui.input_mut(|input| {
        let mut delete = false;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key: egui::Key::Delete | egui::Key::Backspace,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
            {
                delete |= remove
                    && !repeat
                    && !modifiers.alt
                    && !modifiers.ctrl
                    && !modifiers.command
                    && !modifiers.mac_cmd;
                return false;
            }
            true
        });
        delete
    })
}
