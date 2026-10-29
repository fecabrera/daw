//! Instrument layer and component selection, including contextual mix actions.
use super::*;

impl DawUi {
    pub(super) fn instrument_mix_action(&mut self, action: TrackAction) {
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| Some(track.id) == self.selected_track)
        else {
            return;
        };
        let track_id = track.id;
        if let Some(mut instrument) = track.instrument.clone() {
            let mut changed = false;
            for layer in &mut instrument.layers {
                if self.instrument_ui.track == Some(track_id)
                    && (self.instrument_ui.selected_layer == Some(layer.id)
                        || self.instrument_ui.selected_layers.contains(&layer.id))
                {
                    if action == TrackAction::Mute {
                        layer.muted = !layer.muted;
                    } else {
                        layer.soloed = !layer.soloed;
                    }
                    changed = true;
                }
            }
            if changed {
                self.edit(Edit::SetInstrument {
                    track_id,
                    instrument: Some(instrument),
                });
                return;
            }
        }
        // Output and an empty layer selection address only the displayed track.
        self.edit(if action == TrackAction::Mute {
            Edit::SetTrackMuted {
                track_id,
                muted: !track.muted,
            }
        } else {
            Edit::SetTrackSoloed {
                track_id,
                soloed: !track.soloed,
            }
        });
    }

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
        let mut focused_owner = None;
        for (owner, viewport) in &self.instrument_ui.component_viewports {
            if self.instrument_ui.effects_layer == Some(*owner) {
                continue;
            }
            let keyboard = crate::unit_selection::keyboard_focus(
                ui,
                *viewport,
                ui.id().with((track, owner, "component-selection")),
            );
            let mut pane = selection_focus::SelectionFocus::default();
            pane.set(ui, *viewport, true);
            if pane.owns(ui.ctx()) {
                focused_owner = Some(*owner);
            }
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
                    keyboard.request_focus();
                }
                focused_owner = Some(*owner);
                self.instrument_ui.component_focus.set(ui, *viewport, true);
                crate::unit_selection::retain_focus(
                    ui,
                    &keyboard,
                    &self.instrument_ui.component_focus,
                );
            }
        }
        if !self.collection_preserves_selection()
            && !(available && crate::unit_power::keeps_selection(ui))
            && self
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
        let actions = crate::unit_selection::shortcuts(
            ui,
            &self.instrument_ui.component_focus,
            self.effects_editing_enabled(ui),
            self.component_drag.is_some() || self.effects_drag.is_some() || dragging,
        );
        if actions.select_all
            && let Some(owner) = focused_owner
        {
            crate::unit_selection::select_all(
                &mut self.instrument_ui.selected_components,
                self.instrument_ui
                    .component_slots
                    .iter()
                    .filter(|(id, _, _)| *id == owner)
                    .map(|(id, unit, _)| (*id, *unit)),
            );
            ui.ctx().request_repaint();
        }
        if actions.delete
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
