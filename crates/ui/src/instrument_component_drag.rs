//! Shared drag lifecycle for catalog additions and installed component transfers.
use super::*;
use crate::instrument_catalog::add_components;

pub(super) struct ComponentDrag {
    pub(super) project: Id,
    pub(super) track: Id,
    pub(super) instrument: daw_core::InstrumentSettings,
    pub(super) source: ComponentDragSource,
    pub(super) origin: Pos2,
    pub(super) focus: egui::Id,
    pub active: bool,
}

pub(super) enum ComponentDragSource {
    Catalog {
        types: Vec<&'static str>,
        index: usize,
    },
    Installed {
        owner: Id,
        component: daw_core::InstrumentComponent,
        components: Vec<daw_core::InstrumentComponent>,
    },
}

impl DawUi {
    pub(super) fn cancel_unavailable_component_drag(&mut self, ui: &egui::Ui) {
        let invalid = self.component_drag.as_ref().is_some_and(|drag| {
            self.view_mode != ViewMode::Instrument
                || (!self.instrument_catalog_visible
                    && matches!(drag.source, ComponentDragSource::Catalog { .. }))
                || self.session.project.project_id != drag.project
                || self.selected_track != Some(drag.track)
                || !self.effects_editing_enabled(ui)
                || self.effects_drag.is_some()
                || drag::cancelled(ui)
                || !self.session.project.tracks.iter().any(|track| {
                    track.id == drag.track
                        && track.kind == daw_core::TrackKind::Midi
                        && track.instrument.as_ref() == Some(&drag.instrument)
                })
        });
        if invalid {
            if self.view_mode == ViewMode::Instrument
                && (self.instrument_catalog_visible
                    || self
                        .component_drag
                        .as_ref()
                        .is_some_and(|d| matches!(d.source, ComponentDragSource::Installed { .. })))
                && self.edit_environment_available()
                && ui.input(|input| input.focused && input.key_pressed(egui::Key::Escape))
                && let Some(drag) = &self.component_drag
            {
                drag::restore_focus(ui.ctx(), drag.focus);
            }
            self.component_drag = None;
        }
    }

    pub(super) fn activate_component_drag(&mut self, ui: &egui::Ui) {
        if let Some(drag) = &mut self.component_drag
            && !drag.active
            && drag::crossed_threshold(ui, drag.origin)
        {
            drag.active = true;
            match &drag.source {
                ComponentDragSource::Catalog { index, .. } => {
                    self.instrument_catalog.select_dragged(*index)
                }
                ComponentDragSource::Installed {
                    owner,
                    component,
                    components,
                } => {
                    self.instrument_ui.selected_components = if components.is_empty() {
                        [(
                            *owner,
                            if matches!(component, daw_core::InstrumentComponent::Oscillator(_)) {
                                daw_core::InstrumentComponent::Oscillators
                            } else {
                                *component
                            },
                        )]
                        .into_iter()
                        .collect()
                    } else {
                        components.iter().map(|c| (*owner, *c)).collect()
                    };
                }
            }
        }
    }

    pub(super) fn finish_component_drag(
        &mut self,
        ui: &egui::Ui,
        track: Id,
        settings: &mut daw_core::InstrumentSettings,
        rows: &[(Id, Rect)],
        viewport: Rect,
    ) -> Option<(Edit, Vec<daw_core::InstrumentComponent>)> {
        self.cancel_unavailable_component_drag(ui);
        let Some(drag) = &self.component_drag else {
            return None;
        };
        if !drag.active {
            if !ui.input(|input| input.pointer.primary_down()) {
                self.component_drag = None;
            }
            return None;
        }
        // A component drag never overwrites parameter or topology edits made after its press.
        if drag.track != track || settings != &drag.instrument {
            self.component_drag = None;
            return None;
        }
        let target = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
            .and_then(|pointer| {
                rows.iter().find_map(|(id, row)| {
                    let rect = row.intersect(viewport);
                    rect.contains(pointer).then_some((*id, rect))
                })
            });
        let duplicate = ui.input(|i| clip_copy_modifier(i.modifiers));
        let target = target.filter(|(id, _)| match &drag.source {
            ComponentDragSource::Catalog { types, .. } => {
                crate::instrument_catalog::can_add_components(settings, *id, types)
            }
            ComponentDragSource::Installed {
                owner,
                component,
                components,
            } => {
                if matches!(component, daw_core::InstrumentComponent::Oscillator(_)) {
                    self.session
                        .project
                        .can_transfer_component(track, *owner, *id, *component, duplicate)
                } else {
                    self.session
                        .project
                        .can_transfer_components(track, *owner, *id, components, duplicate)
                }
            }
        });
        let mut destination_index = None;
        let mut oscillator_insertion = None;
        let mut unit_insertion = None;
        let target = target.filter(|(owner, row)| {
            let ComponentDragSource::Installed {
                owner: from,
                component,
                components,
            } = &drag.source
            else {
                return true;
            };
            if matches!(component, daw_core::InstrumentComponent::Oscillator(_)) {
                return true;
            }
            let pointer = ui.input(|i| i.pointer.latest_pos()).unwrap();
            let same_move = from == owner && !duplicate;
            if same_move && pointer.x < row.left() + TRACK_WIDTH {
                return false;
            }
            let slots = self.inline_unit_slots(*owner);
            let first = slots
                .iter()
                .take_while(|(unit, _)| unit.is_source())
                .count();
            let only_sources = components.iter().all(|c| c.is_source());
            let slot = if only_sources {
                0
            } else if pointer.x < row.left() + TRACK_WIDTH {
                slots.len()
            } else {
                drag::insertion_slot(pointer, slots.iter().map(|(_, r)| *r)).max(first)
            };
            let order: Vec<_> = slots.iter().map(|(c, _)| *c).collect();
            // Sources stay pinned. A Macro transfer merges its one existing destination unit.
            let moving: Vec<_> = components
                .iter()
                .copied()
                .filter(|c| {
                    !c.is_source()
                        && (same_move
                            || (*c == daw_core::InstrumentComponent::Macros && order.contains(c)))
                })
                .collect();
            let (mut index, changed) = drag::group_insertion(&order, &moving, slot);
            let incoming_source = !same_move && components.iter().any(|c| c.is_source());
            if first == 0 && incoming_source {
                index += 1;
            }
            index = index.max(usize::from(first > 0 || incoming_source));
            destination_index = Some(index);
            unit_insertion = Some((
                slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
                slot,
                *row,
                !same_move || changed,
            ));
            true
        });
        let target = target.filter(|(owner, row)| {
            let ComponentDragSource::Installed {
                owner: from,
                component: daw_core::InstrumentComponent::Oscillator(id),
                ..
            } = &drag.source
            else {
                return true;
            };
            let slots: Vec<_> = self
                .instrument_ui
                .oscillator_slots
                .iter()
                .filter(|(layer, _, _)| layer == owner)
                .map(|(_, id, r)| (*id, *r))
                .collect();
            let pointer = ui.input(|i| i.pointer.latest_pos()).unwrap();
            let slot = if pointer.x < row.left() + TRACK_WIDTH {
                slots.len()
            } else {
                drag::insertion_slot(pointer, slots.iter().map(|(_, r)| *r))
            };
            let old = slots.iter().position(|(unit, _)| *unit == *id);
            let index = slot
                - usize::from(!duplicate && *from == *owner && old.is_some_and(|old| old < slot));
            if !duplicate && *from == *owner && old == Some(index) {
                return false;
            }
            destination_index = Some(index);
            oscillator_insertion = Some((
                slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
                slot,
                *row,
            ));
            true
        });
        if let Some((_, rect)) = target
            && matches!(drag.source, ComponentDragSource::Catalog { .. })
        {
            panels::selection_border(ui, rect);
        }
        if let Some((slots, slot, row)) = oscillator_insertion {
            let body = Rect::from_min_max(Pos2::new(row.left() + TRACK_WIDTH, row.top()), row.max)
                .intersect(viewport);
            drag::insertion_line(ui, &slots, slot, body);
        }
        if let Some((slots, slot, row, changed)) = &unit_insertion
            && *changed
        {
            let body = Rect::from_min_max(Pos2::new(row.left() + TRACK_WIDTH, row.top()), row.max)
                .intersect(viewport);
            drag::insertion_line(ui, slots, *slot, body);
        }
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.drop_target = target.map(|(id, _)| id);
            self.instrument_catalog_bounds.drop_bounds =
                target.map_or(Rect::NOTHING, |(_, rect)| rect);
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            if duplicate && matches!(drag.source, ComponentDragSource::Installed { .. }) {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            }
        } else {
            egui::CursorIcon::NotAllowed
        });
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.component_drag.take().unwrap();
            drag::restore_focus(ui.ctx(), drag.focus);
            if let Some((layer, _)) = target {
                match drag.source {
                    ComponentDragSource::Catalog { types, .. } => {
                        add_components(settings, layer, &types)
                    }
                    ComponentDragSource::Installed {
                        owner,
                        component,
                        components,
                    } => {
                        self.instrument_ui.select_layer(layer);
                        if unit_insertion
                            .as_ref()
                            .is_some_and(|(_, _, _, changed)| !changed)
                        {
                            return None;
                        }
                        if matches!(component, daw_core::InstrumentComponent::Oscillator(_)) {
                            return Some((
                                Edit::TransferInstrumentComponent {
                                    track_id: track,
                                    source: owner,
                                    destination: layer,
                                    destination_index,
                                    component,
                                    duplicate,
                                },
                                vec![daw_core::InstrumentComponent::Oscillators],
                            ));
                        }
                        match self.session.project.component_group_transfer(
                            track,
                            owner,
                            layer,
                            &components,
                            destination_index.unwrap(),
                            duplicate,
                        ) {
                            Ok(result) if result.changed => {
                                return Some((result.edit, result.components));
                            }
                            Ok(_) => return None,
                            Err(error) => {
                                self.fail(error);
                                return None;
                            }
                        }
                    }
                }
                self.instrument_ui.select_layer(layer);
                ui.ctx().request_repaint();
            }
        } else if !ui.input(|input| input.pointer.primary_down()) {
            self.component_drag = None;
        }
        None
    }

    /// Installed and catalog drags share activation, row targets, scrolling and cancellation.
    pub(super) fn start_installed_component_drag(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        excluded: &[Rect],
        track: Id,
        owner: Id,
        component: daw_core::InstrumentComponent,
    ) {
        #[cfg(test)]
        self.instrument_ui
            .component_bounds
            .push((owner, component, response.rect));
        response
            .clone()
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(if cfg!(target_os = "macos") {
                "Drag to reorder or move this component. Hold Option to duplicate. Sources stay first."
            } else {
                "Drag to reorder or move this component. Hold Ctrl to duplicate. Sources stay first."
            });
        if self.strip_dragging
            || crate::strip_reorder::dragging(ui.ctx())
            || self.component_drag.is_some()
            || self.effects_drag.is_some()
            || self.instrument_ui.dragging()
            || self.instrument_ui.editing_name()
            || !self.effects_editing_enabled(ui)
        {
            return;
        }
        let Some(origin) =
            drag::press_origin(ui, response).filter(|p| !excluded.iter().any(|r| r.contains(*p)))
        else {
            return;
        };
        let Some(instrument) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.clone())
        else {
            return;
        };
        let components = if matches!(component, daw_core::InstrumentComponent::Oscillator(_)) {
            Vec::new()
        } else {
            let order = if let Some(bus) = instrument.bus(owner) {
                bus.visible_components()
            } else {
                instrument
                    .layers
                    .iter()
                    .find(|l| l.id == owner)
                    .unwrap()
                    .visible_components()
            };
            let keys: Vec<_> = order.into_iter().map(|c| (owner, c)).collect();
            crate::unit_selection::drag_group(
                &keys,
                &self.instrument_ui.selected_components,
                (owner, component),
            )
            .into_iter()
            .map(|(_, c)| c)
            .collect()
        };
        response.request_focus();
        self.instrument_ui.select_layer(owner);
        self.component_drag = Some(ComponentDrag {
            project: self.session.project.project_id,
            track,
            instrument,
            source: ComponentDragSource::Installed {
                owner,
                component,
                components,
            },
            origin,
            focus: response.id,
            active: false,
        });
    }
}
