//! Selection, keyboard and outline rules shared by installed effect and instrument units.
use crate::{panels, selection_focus::SelectionFocus};
use egui::{Rect, Response, Ui};

pub(super) fn clicked(ui: &Ui, rect: Rect, allowed: bool) -> bool {
    allowed
        && ui.rect_contains_pointer(rect)
        && ui.input(|input| {
            input.focused && input.pointer.button_clicked(egui::PointerButton::Primary)
        })
}

/// Cover the pane without sensing clicks or drags, so child controls keep their gestures.
/// A zero-area target loses focus when egui reads its response after a lane-space click.
pub(super) fn keyboard_focus(ui: &mut Ui, bounds: Rect, id: egui::Id) -> Response {
    ui.interact(bounds, id, egui::Sense::focusable_noninteractive())
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

#[derive(Default)]
pub(super) struct Shortcuts {
    pub delete: bool,
    pub select_all: bool,
}

/// Unit panes consume their shortcuts even when repeats or gestures block the action.
/// Text editors retain their keys. Unrelated input must not change pane focus.
pub(super) fn shortcuts(
    ui: &Ui,
    pane: &SelectionFocus,
    allowed: bool,
    dragging: bool,
) -> Shortcuts {
    if !ui.input(|input| {
        input.key_pressed(egui::Key::Delete)
            || input.key_pressed(egui::Key::Backspace)
            || input.events.iter().any(|event| {
                matches!(event,
                egui::Event::Key { key: egui::Key::A, pressed: true, modifiers, .. }
                    if modifiers.command || modifiers.ctrl || modifiers.mac_cmd)
            })
    }) {
        return Shortcuts::default();
    }
    let text_editing = ui
        .ctx()
        .memory(|memory| memory.focused())
        .is_some_and(|id| egui::TextEdit::load_state(ui.ctx(), id).is_some());
    if text_editing || pane.lost(ui.ctx(), true, false) {
        return Shortcuts::default();
    }
    let active = allowed && ui.is_enabled() && !dragging && !ui.input(|i| i.pointer.any_down());
    ui.input_mut(|input| {
        let mut actions = Shortcuts::default();
        input.events.retain(|event| {
            if let egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
            {
                match key {
                    egui::Key::Delete | egui::Key::Backspace => {
                        actions.delete |= active
                            && !repeat
                            && !modifiers.alt
                            && !modifiers.ctrl
                            && !modifiers.command
                            && !modifiers.mac_cmd;
                        return false;
                    }
                    egui::Key::A if modifiers.command || modifiers.ctrl || modifiers.mac_cmd => {
                        actions.select_all |=
                            active && !repeat && modifiers.matches_exact(egui::Modifiers::COMMAND);
                        return false;
                    }
                    _ => {}
                }
            }
            true
        });
        actions
    })
}

pub(super) fn select_all<T: Eq + std::hash::Hash>(
    selected: &mut std::collections::HashSet<T>,
    units: impl IntoIterator<Item = T>,
) {
    selected.clear();
    selected.extend(units);
}
