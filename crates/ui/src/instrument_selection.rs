//! Instrument layer and component selection, including contextual mix actions.
use super::*;

impl DawUi {
    pub(super) fn instrument_mix_action(&mut self, action: TrackAction) {
        let Some(track) = self
            .selected_instrument_owner()
            .and_then(|id| self.session.project.instrument_track(id))
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
        if let Some((parent, row)) = self
            .session
            .project
            .beat_subtrack(track_id)
            .map(|(parent, row)| (parent, row.clone()))
        {
            self.edit(Edit::Beat {
                track_id: parent,
                edit: daw_core::BeatEdit::SetMix {
                    id: row.id,
                    gain_db: row.instrument.main.gain_db,
                    pan: row.instrument.main.pan,
                    muted: if action == TrackAction::Mute {
                        !row.muted
                    } else {
                        row.muted
                    },
                    soloed: if action == TrackAction::Solo {
                        !row.soloed
                    } else {
                        row.soloed
                    },
                },
            });
            return;
        }
        let track_id = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.beat.as_ref().is_some_and(|b| b.output.id == track_id))
            .map_or(track_id, |t| t.id);
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
                ui.ctx().request_repaint();
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
            if !self.instrument_ui.selected_components.is_empty() {
                ui.ctx().request_repaint();
            }
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
            let removed = if let Some(bus) = settings.bus_mut(owner) {
                bus.remove_components(&units)
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
    }
}
