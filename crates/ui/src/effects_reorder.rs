use super::*;
use crate::effects_panel::EffectTarget;

pub(super) struct EffectDrag {
    pub target: EffectTarget,
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
        crate::unit_selection::select(&mut self.selected_effects, id, toggle);
    }

    pub(super) fn cancel_unavailable_effect_drag(&mut self, ui: &egui::Ui) {
        if self.effects_drag.as_ref().is_some_and(|drag| {
            !self.view_mode.has_effects()
                || !self.timeline_effects_visible
                || self.view_mode != drag.view
                || self.effects_target() != Some(drag.target)
                || !ui.is_enabled()
                || !self.edit_environment_available()
                || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
                || !self.effect_chain(drag.target).is_some_and(|(effects, _)| {
                    effects
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
        target: Option<EffectTarget>,
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
            && let Some(target) = target
        {
            let units = crate::unit_selection::drag_group(order, &self.selected_effects, unit);
            // Arm on press so a fast motion+release cannot lose its origin between frames.
            header.request_focus();
            self.effects_drag = Some(EffectDrag {
                target,
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
        target: Option<EffectTarget>,
        index: usize,
        effects: &[daw_core::EffectInstance],
    ) {
        if self.effects_drag.is_none()
            && let Some(target) = target
            && let Some(start) = self.effects_catalog.start_drag(ui, response, index)
        {
            self.effects_drag = Some(EffectDrag {
                target,
                view: self.view_mode,
                source: DragSource::Library {
                    types: start.types,
                    index: start.index,
                },
                origin: start.origin,
                focus: start.focus,
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
            if drag::crossed_threshold(ui, drag.origin) {
                drag.active = true;
                match &drag.source {
                    DragSource::Chain { unit, .. } => {
                        let unit = *unit;
                        if !self.selected_effects.contains(&unit) {
                            self.select_effect(unit, false);
                        }
                    }
                    DragSource::Library { index, .. } => {
                        self.effects_catalog.select_dragged(*index);
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
        let duplicate = ui.input(|i| clip_copy_modifier(i.modifiers));
        let target = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
            .map(|pointer| {
                let slot = drag::insertion_slot(pointer, slots.iter().map(|(_, r)| *r));
                match &drag.source {
                    DragSource::Library { .. } => (slot, slot, true),
                    DragSource::Chain { .. } if duplicate => (slot, slot, true),
                    DragSource::Chain { units, .. } => {
                        let (index, changed) = drag::group_insertion(&drag.order, units, slot);
                        (slot, index, changed)
                    }
                }
            });
        if let Some((slot, _, changed)) = target
            && changed
        {
            drag::insertion_line(
                ui,
                &slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
                slot,
                viewport,
            );
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            if duplicate && !drag.is_library() {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            }
        } else {
            egui::CursorIcon::NotAllowed
        });
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.effects_drag.take().unwrap();
            drag::restore_focus(ui.ctx(), drag.focus);
            if let Some((_, index, true)) = target {
                match drag.source {
                    DragSource::Library { types, .. } => {
                        let added = effects_catalog::instantiate(&types);
                        effects.splice(index..index, added);
                    }
                    DragSource::Chain { units, .. } if duplicate => {
                        let selected: HashSet<_> = units.into_iter().collect();
                        let copies: Vec<_> = effects
                            .iter()
                            .filter(|e| selected.contains(&e.id))
                            .map(|e| {
                                let mut copy = e.clone();
                                copy.id = Id::new_v4();
                                for route in copy.unit.modulations_mut() {
                                    route.id = Id::new_v4();
                                }
                                copy
                            })
                            .collect();
                        self.selected_effects = copies.iter().map(|e| e.id).collect();
                        effects.splice(index..index, copies);
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
        if self.effects_drag.as_ref().is_some_and(|drag| drag.active) {
            drag::scroll_near_edges(ui, drag::Axis::Horizontal, 360.0);
        }
    }
}
