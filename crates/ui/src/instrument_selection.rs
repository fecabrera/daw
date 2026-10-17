//! Component selection is UI state, independent of the catalog and selected layer.
use super::*;

impl DawUi {
    pub(super) fn component_selection(&mut self, ui: &mut egui::Ui, track: Id) {
        let available = self.edit_environment_available() && ui.is_enabled();
        let dragging = self.component_drag.as_ref().is_some_and(|d| d.active)
            || self.strip_dragging
            || crate::strip_reorder::dragging(ui.ctx());
        let allowed = available && !dragging && !self.instrument_ui.dragging();
        for (owner, viewport) in &self.instrument_ui.component_viewports {
            let keyboard = ui.interact(
                Rect::from_center_size(viewport.center(), Vec2::ZERO),
                ui.id().with((track, owner, "component-selection")),
                Sense::focusable_noninteractive(),
            );
            if self
                .instrument_ui
                .selected_components
                .iter()
                .any(|(id, _)| id == owner)
            {
                self.instrument_ui.component_focus.set(ui, *viewport, true);
                if self.instrument_ui.component_refocus {
                    keyboard.request_focus();
                    self.instrument_ui.component_refocus = false;
                }
            }
            if crate::unit_selection::clicked(ui, *viewport, allowed) {
                let clicked = self
                    .instrument_ui
                    .component_slots
                    .iter()
                    .find(|(id, _, rect)| {
                        id == owner && ui.rect_contains_pointer(rect.intersect(*viewport))
                    })
                    .map(|(id, unit, _)| (*id, *unit));
                if let Some(key) = clicked {
                    // Each component lane owns its selection, as each effects chain does.
                    self.instrument_ui
                        .selected_components
                        .retain(|(id, _)| id == owner);
                    crate::unit_selection::select(
                        &mut self.instrument_ui.selected_components,
                        key,
                        ui.input(|i| i.modifiers.shift),
                    );
                } else {
                    self.instrument_ui.selected_components.clear();
                }
                self.instrument_ui.component_focus.set(ui, *viewport, true);
                crate::unit_selection::retain_focus(
                    ui,
                    &keyboard,
                    &self.instrument_ui.component_focus,
                );
            }
        }
        if self
            .instrument_ui
            .component_focus
            .lost(ui.ctx(), available, dragging)
        {
            self.instrument_ui.selected_components.clear();
        }
        self.instrument_ui.selected_components.retain(|key| {
            self.instrument_ui
                .component_slots
                .iter()
                .any(|(id, unit, _)| (*id, *unit) == *key)
        });
        // Paint every selected unit after row fills and dividers, with its lane's scroll clip.
        for (owner, viewport) in &self.instrument_ui.component_viewports {
            let mut clipped = ui.new_child(egui::UiBuilder::new().max_rect(*viewport));
            clipped.set_clip_rect(*viewport);
            for (id, unit, rect) in &self.instrument_ui.component_slots {
                if id == owner {
                    crate::unit_selection::paint(
                        &clipped,
                        *rect,
                        self.instrument_ui
                            .selected_components
                            .contains(&(*id, *unit)),
                    );
                }
            }
        }
    }
}
