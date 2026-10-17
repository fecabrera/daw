//! Component selection is UI state, independent of the catalog and selected layer.
use super::*;

impl DawUi {
    pub(super) fn component_selection(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        settings: &mut daw_core::InstrumentSettings,
    ) {
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
                || keyboard.has_focus()
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
            self.instrument_ui.component_focus = Default::default();
        }
        self.instrument_ui.selected_components.retain(|key| {
            self.instrument_ui
                .component_slots
                .iter()
                .any(|(id, unit, _)| (*id, *unit) == *key)
        });
        let delete = crate::unit_selection::delete_pressed(
            ui,
            &self.instrument_ui.component_focus,
            self.effects_editing_enabled(ui),
            self.component_drag.is_some() || self.effects_drag.is_some() || dragging,
        );
        if delete
            && let Some((owner, _)) = self
                .instrument_ui
                .selected_components
                .iter()
                .next()
                .copied()
        {
            let units: Vec<_> = self
                .instrument_ui
                .selected_components
                .iter()
                .map(|(_, c)| *c)
                .collect();
            let removed = if owner == settings.main.id {
                settings.main.remove_components(&units)
            } else {
                settings
                    .layers
                    .iter_mut()
                    .find(|l| l.id == owner)
                    .and_then(|l| l.remove_components(&units))
            };
            if removed == Some(true) {
                self.instrument_ui.selected_components.clear();
                if let Some(keyboard) =
                    ui.ctx()
                        .read_response(ui.id().with((track, owner, "component-selection")))
                {
                    keyboard.request_focus();
                }
                ui.ctx().request_repaint();
            }
        }
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
