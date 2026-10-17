//! Component selection is UI state, independent of the catalog and selected layer.
use super::*;
use daw_core::InstrumentComponent;

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
                .selected_component
                .is_some_and(|(id, _)| id == *owner)
            {
                self.instrument_ui.component_focus.set(ui, *viewport, true);
                if self.instrument_ui.component_refocus {
                    keyboard.request_focus();
                    self.instrument_ui.component_refocus = false;
                }
            }
            if crate::unit_selection::clicked(ui, *viewport, allowed) {
                self.instrument_ui.selected_component = self
                    .instrument_ui
                    .component_slots
                    .iter()
                    .find(|(id, _, rect)| {
                        id == owner && ui.rect_contains_pointer(rect.intersect(*viewport))
                    })
                    .map(|(id, unit, _)| (*id, *unit));
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
            || self.instrument_ui.selected_component.is_some_and(|key| {
                !self
                    .instrument_ui
                    .component_slots
                    .iter()
                    .any(|(id, unit, _)| (*id, *unit) == key)
            })
        {
            self.instrument_ui.selected_component = None;
        }
        // Paint after every row fill and divider, with each lane's own scroll clip.
        if let Some((owner, unit)) = self.instrument_ui.selected_component
            && let Some((_, viewport)) = self
                .instrument_ui
                .component_viewports
                .iter()
                .find(|(id, _)| *id == owner)
            && let Some((_, _, rect)) = self
                .instrument_ui
                .component_slots
                .iter()
                .find(|(id, c, _)| *id == owner && *c == unit)
        {
            let mut clipped = ui.new_child(egui::UiBuilder::new().max_rect(*viewport));
            clipped.set_clip_rect(*viewport);
            crate::unit_selection::paint(&clipped, *rect, true);
        }
    }

    pub(super) fn select_transferred_component(
        &mut self,
        track: Id,
        owner: Id,
        component: InstrumentComponent,
    ) {
        let Some(i) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_ref())
        else {
            return;
        };
        let unit = match component {
            InstrumentComponent::Envelope(_) => {
                let units = if owner == i.main.id {
                    &i.main.envelopes
                } else {
                    &i.layers.iter().find(|l| l.id == owner).unwrap().envelopes
                };
                units
                    .last()
                    .map(|e| InstrumentComponent::Envelope(e.id))
                    .unwrap_or(component)
            }
            InstrumentComponent::Lfo(_) => {
                let units = if owner == i.main.id {
                    &i.main.lfos
                } else {
                    &i.layers.iter().find(|l| l.id == owner).unwrap().lfos
                };
                units
                    .last()
                    .map(|e| InstrumentComponent::Lfo(e.id))
                    .unwrap_or(component)
            }
            InstrumentComponent::Oscillator(_) => InstrumentComponent::Oscillators,
            unit => unit,
        };
        self.instrument_ui.selected_component = Some((owner, unit));
        self.instrument_ui.component_refocus = true;
    }
}
