use super::*;

pub(super) struct EffectDrag {
    pub track: Id,
    view: ViewMode,
    source: DragSource,
    origin: Pos2,
    focus: egui::Id,
    pub active: bool,
    // A changed chain invalidates the gesture; do not overwrite another edit on release.
    order: Vec<Id>,
}
enum DragSource {
    Chain {
        unit: Id,
        units: Vec<Id>,
    },
    Library {
        types: Vec<&'static str>,
        index: usize,
    },
}
impl EffectDrag {
    pub fn is_library(&self) -> bool {
        matches!(self.source, DragSource::Library { .. })
    }
    #[cfg(test)]
    pub fn chain_unit(&self) -> Option<Id> {
        match self.source {
            DragSource::Chain { unit, .. } => Some(unit),
            DragSource::Library { .. } => None,
        }
    }
}
impl DawUi {
    pub(super) fn select_effect(&mut self, id: Id, toggle: bool) {
        if toggle {
            if !self.selected_effects.remove(&id) {
                self.selected_effects.insert(id);
            }
        } else {
            self.selected_effects.clear();
            self.selected_effects.insert(id);
        }
    }

    pub(super) fn cancel_unavailable_effect_drag(&mut self, ui: &egui::Ui) {
        if self.effects_drag.as_ref().is_some_and(|drag| {
            !self.view_mode.has_effects()
                || (self.view_mode == ViewMode::Timeline && !self.timeline_effects_visible)
                || self.view_mode != drag.view
                || self.selected_track != Some(drag.track)
                || !ui.is_enabled()
                || !self.edit_environment_available()
                || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
                || !self
                    .session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == drag.track)
                    .is_some_and(|track| {
                        track
                            .effects
                            .iter()
                            .map(|unit| unit.id)
                            .eq(drag.order.iter().copied())
                    })
        }) {
            if ui.input(|input| input.focused && input.key_pressed(egui::Key::Escape))
                && self.view_mode.has_effects()
                && self.edit_environment_available()
                && let Some(drag) = &self.effects_drag
            {
                ui.ctx()
                    .memory_mut(|memory| memory.request_focus(drag.focus));
            }
            self.effects_drag = None;
        }
    }
    pub(super) fn start_effect_drag(
        &mut self,
        ui: &egui::Ui,
        header: &egui::Response,
        power: Rect,
        track: Option<Id>,
        unit: Id,
        order: &[Id],
    ) {
        if header.is_pointer_button_down_on()
            && ui.is_enabled()
            && ui.input(|input| {
                input.focused
                    && !input.key_pressed(egui::Key::Escape)
                    && input.pointer.button_pressed(egui::PointerButton::Primary)
            })
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
            && !power.contains(origin)
            && let Some(track) = track
        {
            let units = if self.selected_effects.contains(&unit) {
                order
                    .iter()
                    .copied()
                    .filter(|id| self.selected_effects.contains(id))
                    .collect()
            } else {
                vec![unit]
            };
            // Arm on press so a fast motion+release cannot lose its origin between frames.
            header.request_focus();
            self.effects_drag = Some(EffectDrag {
                track,
                view: self.view_mode,
                source: DragSource::Chain { unit, units },
                origin,
                focus: header.id,
                active: false,
                order: order.to_vec(),
            });
        }
    }
    pub(super) fn start_library_effect_drag(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        track: Option<Id>,
        index: usize,
        effects: &[daw_core::EffectInstance],
    ) {
        if self.effects_drag.is_none()
            && response.is_pointer_button_down_on()
            && ui.is_enabled()
            && ui.input(|input| {
                input.focused && input.pointer.button_pressed(egui::PointerButton::Primary)
            })
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
            && response.rect.contains(origin)
            && let Some(track) = track
        {
            self.effects_catalog.focus_row(ui, response);
            let type_id = effects_catalog::ENTRIES[index].0;
            let types = if self.effects_catalog.selection.contains(type_id) {
                effects_catalog::ENTRIES
                    .iter()
                    .filter(|entry| self.effects_catalog.selection.contains(entry.0))
                    .map(|entry| entry.0)
                    .collect()
            } else {
                vec![type_id]
            };
            self.effects_drag = Some(EffectDrag {
                track,
                view: self.view_mode,
                source: DragSource::Library { types, index },
                origin,
                focus: response.id,
                active: false,
                order: effects.iter().map(|effect| effect.id).collect(),
            });
        }
    }
    pub(super) fn finish_effect_drag(
        &mut self,
        ui: &egui::Ui,
        effects: &mut Vec<daw_core::EffectInstance>,
        slots: &[(Id, Rect)],
        viewport: Rect,
        allowed: bool,
    ) {
        if !allowed || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape)) {
            self.effects_drag = None;
            return;
        }
        let Some(drag) = &mut self.effects_drag else {
            return;
        };
        if !drag.active {
            let threshold = ui
                .ctx()
                .options(|options| options.input_options.max_click_dist);
            if ui
                .input(|input| input.pointer.latest_pos())
                .is_some_and(|pointer| pointer.distance(drag.origin) > threshold)
            {
                drag.active = true;
                match &drag.source {
                    DragSource::Chain { unit, .. } => {
                        let unit = *unit;
                        if !self.selected_effects.contains(&unit) {
                            self.select_effect(unit, false);
                        }
                    }
                    DragSource::Library { index, .. } => {
                        if !self
                            .effects_catalog
                            .selection
                            .contains(effects_catalog::ENTRIES[*index].0)
                        {
                            self.effects_catalog.click(*index, false);
                        }
                    }
                }
            } else {
                if !ui.input(|input| input.pointer.primary_down()) {
                    self.effects_drag = None;
                }
                return;
            }
        }
        let drag = self.effects_drag.as_ref().unwrap();
        if matches!(&drag.source, DragSource::Chain { unit, .. } if !effects.iter().any(|effect| effect.id == *unit))
        {
            self.effects_drag = None;
            return;
        }
        let target = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
            .map(|pointer| {
                let slot = slots
                    .iter()
                    .position(|(_, rect)| pointer.x < rect.center().x)
                    .unwrap_or(slots.len());
                match &drag.source {
                    DragSource::Library { .. } => (slot, slot, true),
                    DragSource::Chain { units, .. } => {
                        let moving: HashSet<_> = units.iter().copied().collect();
                        let index = slots[..slot]
                            .iter()
                            .filter(|(id, _)| !moving.contains(id))
                            .count();
                        let mut order: Vec<_> = drag
                            .order
                            .iter()
                            .copied()
                            .filter(|id| !moving.contains(id))
                            .collect();
                        order.splice(index..index, units.iter().copied());
                        (slot, index, order != drag.order)
                    }
                }
            });
        if let Some((slot, _, changed)) = target
            && changed
        {
            let x = slots
                .get(slot)
                .map_or_else(
                    || {
                        slots
                            .last()
                            .map_or(viewport.left(), |(_, rect)| rect.right())
                    },
                    |(_, rect)| rect.left(),
                )
                .clamp(viewport.left() + 1.0, viewport.right() - 1.0);
            ui.painter_at(viewport).vline(
                x,
                viewport.y_range(),
                Stroke::new(2.0, theme::accent(ui.ctx())),
            );
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NotAllowed
        });
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.effects_drag.take().unwrap();
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(drag.focus));
            if let Some((_, index, true)) = target {
                match drag.source {
                    DragSource::Library { types, .. } => {
                        let added = effects_catalog::ENTRIES
                            .iter()
                            .filter(|entry| types.contains(&entry.0))
                            .map(|entry| (entry.2)());
                        effects.splice(index..index, added);
                    }
                    DragSource::Chain { units, .. } => {
                        // Partition in existing chain order, regardless of selection click order.
                        let moving: HashSet<_> = units.into_iter().collect();
                        let (mut units, mut rest): (Vec<_>, Vec<_>) = effects
                            .drain(..)
                            .partition(|effect| moving.contains(&effect.id));
                        let mut after = rest.split_off(index);
                        rest.append(&mut units);
                        rest.append(&mut after);
                        *effects = rest;
                    }
                }
            }
        } else if !ui.input(|input| input.pointer.primary_down()) {
            self.effects_drag = None;
        }
    }
    pub(super) fn scroll_effect_drag(&self, ui: &egui::Ui) {
        if self.effects_drag.as_ref().is_none_or(|drag| !drag.active)
            || !ui.is_enabled()
            || ui.input(|input| {
                !input.focused
                    || !input.pointer.primary_down()
                    || input.key_pressed(egui::Key::Escape)
            })
        {
            return;
        }
        let viewport = ui.clip_rect();
        let Some(pointer) = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
        else {
            return;
        };
        let edge = 24.0_f32.min(viewport.width() / 2.0);
        let direction = if pointer.x < viewport.left() + edge {
            1.0
        } else if pointer.x > viewport.right() - edge {
            -1.0
        } else {
            return;
        };
        ui.scroll_with_delta_animation(
            Vec2::new(
                direction * 360.0 * ui.input(|input| input.stable_dt.min(0.05)),
                0.0,
            ),
            egui::style::ScrollAnimation::none(),
        );
        ui.ctx().request_repaint();
    }
}
