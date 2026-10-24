use super::*;
use crate::effects_panel::EffectTarget;

pub(super) struct EffectDrag {
    pub target: EffectTarget,
    view: ViewMode,
    source: DragSource,
    origin: Pos2,
    focus: egui::Id,
    pub active: bool,
    timeline_hover: bool,
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

    pub(super) fn retarget_library_effect_drag(
        &mut self,
        target: EffectTarget,
        effects: &[daw_core::EffectInstance],
    ) {
        if let Some(drag) = &mut self.effects_drag
            && drag.is_library()
            && drag.target != target
        {
            drag.target = target;
            drag.order = effects.iter().map(|e| e.id).collect();
        }
    }
    pub(super) fn cancel_unavailable_effect_drag(&mut self, ui: &egui::Ui) {
        if self.effects_drag.as_ref().is_some_and(|drag| {
            !self.view_mode.has_effects()
                || (!self.timeline_effects_visible
                    && !matches!(drag.target, EffectTarget::Layer { .. })
                    && !(self.view_mode == ViewMode::Instrument && drag.is_library()))
                || self.view_mode != drag.view
                || match drag.target {
                    EffectTarget::Layer { track, .. } => {
                        self.view_mode != ViewMode::Instrument || self.selected_track != Some(track)
                    }
                    _ => self.effects_target() != Some(drag.target),
                }
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
                timeline_hover: false,
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
                timeline_hover: false,
                order: effects.iter().map(|effect| effect.id).collect(),
            });
        }
    }

    fn activate_effect_drag(&mut self, ui: &egui::Ui) -> bool {
        let Some(drag) = &mut self.effects_drag else {
            return false;
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
                return false;
            }
        }
        true
    }

    /// Timeline row drops append to a chain through the same transfer transaction.
    /// The workspace resolves visible geometry while ordinary track controls stay disabled.
    pub(super) fn finish_timeline_effect_drag(
        &mut self,
        ui: &egui::Ui,
        target: Option<(EffectTarget, Rect)>,
        allowed: bool,
    ) -> Option<(EffectTarget, daw_core::EffectTransfer)> {
        if let Some(drag) = &mut self.effects_drag {
            drag.timeline_hover = false;
        }
        if !allowed
            || self.view_mode != ViewMode::Timeline
            || self.effects_drag.as_ref().is_none_or(|d| d.is_library())
            || !self.activate_effect_drag(ui)
        {
            return None;
        }
        let (destination, row) = target?;
        let drag = self.effects_drag.as_mut()?;
        drag.timeline_hover = true;
        let DragSource::Chain { units, .. } = &drag.source else {
            return None;
        };
        let valid = self
            .session
            .project
            .can_transfer_effects(drag.target, destination, units);
        let duplicate = ui.input(|i| clip_copy_modifier(i.modifiers));
        ui.ctx().set_cursor_icon(if !valid {
            egui::CursorIcon::NotAllowed
        } else if duplicate {
            egui::CursorIcon::Copy
        } else {
            egui::CursorIcon::Grabbing
        });
        if valid {
            panels::selection_border(ui, row);
        }
        if !ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            return None;
        }
        let drag = self.effects_drag.take()?;
        drag::restore_focus(ui.ctx(), drag.focus);
        if !valid {
            return None;
        }
        let DragSource::Chain { units, .. } = drag.source else {
            return None;
        };
        let index = self.effect_chain(destination)?.0.len();
        if drag.target == destination
            && !duplicate
            && !drag::group_insertion(&drag.order, &units, index).1
        {
            return None;
        }
        match self.session.project.transfer_effects(
            drag.target,
            destination,
            &units,
            index,
            duplicate,
        ) {
            Ok(transfer) => Some((destination, transfer)),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    pub(super) fn finish_cross_effect_drag(
        &mut self,
        ui: &egui::Ui,
        destination: EffectTarget,
        slots: &[(Id, Rect)],
        viewport: Rect,
        allowed: bool,
    ) -> Option<daw_core::EffectTransfer> {
        if !allowed {
            return None;
        }
        let drag = self.effects_drag.as_ref()?;
        let owner = |target| match target {
            EffectTarget::Track(track) | EffectTarget::Layer { track, .. } => Some(track),
            EffectTarget::Master => None,
        };
        if !matches!(drag.source, DragSource::Chain { .. })
            || drag.target == destination
            || owner(drag.target).is_none()
            || owner(drag.target) != owner(destination)
            || self.view_mode != ViewMode::Instrument
        {
            return None;
        }
        if !self.activate_effect_drag(ui) {
            return None;
        }
        let drag = self.effects_drag.as_ref()?;
        let DragSource::Chain { units, .. } = &drag.source else {
            return None;
        };
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| viewport.contains(*p))?;
        let slot = drag::insertion_slot(pointer, slots.iter().map(|(_, r)| *r));
        drag::insertion_line(
            ui,
            &slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
            slot,
            viewport,
        );
        let duplicate = ui.input(|i| clip_copy_modifier(i.modifiers));
        ui.ctx().set_cursor_icon(if duplicate {
            egui::CursorIcon::Copy
        } else {
            egui::CursorIcon::Grabbing
        });
        if !ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            return None;
        }
        let result =
            self.session
                .project
                .transfer_effects(drag.target, destination, units, slot, duplicate);
        self.effects_drag = None;
        match result {
            Ok(edit) => Some(edit),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    /// Inline effect insertion shares the complete row geometry with component moves.
    pub(super) fn finish_inline_effect_drag(
        &mut self,
        ui: &egui::Ui,
        destination: EffectTarget,
        slots: &[(daw_core::InstrumentComponent, Rect)],
        viewport: Rect,
        allowed: bool,
    ) -> Option<daw_core::EffectTransfer> {
        if !allowed || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape)) {
            self.effects_drag = None;
            return None;
        }
        if !self.activate_effect_drag(ui) {
            return None;
        }
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| viewport.contains(*p))?;
        let drag = self.effects_drag.as_ref()?;
        let duplicate = ui.input(|i| clip_copy_modifier(i.modifiers));
        let moving: Vec<_> = match &drag.source {
            DragSource::Chain { units, .. } if drag.target == destination && !duplicate => units
                .iter()
                .copied()
                .map(daw_core::InstrumentComponent::Effect)
                .collect(),
            _ => Vec::new(),
        };
        let slot = drag::insertion_slot(pointer, slots.iter().map(|(_, r)| *r))
            .max(slots.iter().take_while(|(u, _)| u.is_source()).count());
        let before = slots[slot..]
            .iter()
            .map(|(u, _)| *u)
            .find(|u| !moving.contains(u));
        let index = slots[..slot]
            .iter()
            .filter(|(u, _)| matches!(u, daw_core::InstrumentComponent::Effect(_)))
            .count();
        let valid = match &drag.source {
            DragSource::Chain { units, .. } => {
                self.session
                    .project
                    .can_transfer_effects(drag.target, destination, units)
            }
            _ => true,
        };
        ui.ctx().set_cursor_icon(if !valid {
            egui::CursorIcon::NotAllowed
        } else if duplicate && !drag.is_library() {
            egui::CursorIcon::Copy
        } else {
            egui::CursorIcon::Grabbing
        });
        let changed = drag.target != destination
            || duplicate
            || drag.is_library()
            || drag::group_insertion(
                &slots.iter().map(|(u, _)| *u).collect::<Vec<_>>(),
                &moving,
                slot,
            )
            .1;
        if valid && changed {
            drag::insertion_line(
                ui,
                &slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
                slot,
                viewport,
            );
        }
        if !ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            return None;
        }
        let drag = self.effects_drag.take()?;
        drag::restore_focus(ui.ctx(), drag.focus);
        if !valid || !changed {
            return None;
        }
        let transfer = match drag.source {
            DragSource::Chain { units, .. } => self.session.project.transfer_effects(
                drag.target,
                destination,
                &units,
                index,
                duplicate,
            ),
            DragSource::Library { types, .. } => {
                let mut effects = self.effect_chain(destination)?.0.to_vec();
                let added: Vec<_> = effects_catalog::instantiate(&types).collect();
                let selected = added.iter().map(|e| e.id).collect();
                effects.splice(index..index, added);
                Ok(daw_core::EffectTransfer {
                    edit: destination.edit(effects, self.effect_chain(destination)?.1),
                    selected,
                })
            }
        }
        .and_then(|transfer| {
            self.session
                .project
                .position_inline_effects(transfer, destination, before)
        });
        match transfer {
            Ok(transfer) => Some(transfer),
            Err(error) => {
                self.fail(error);
                None
            }
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
        if !self.activate_effect_drag(ui) {
            return;
        }
        let drag = self.effects_drag.as_ref().unwrap();
        if drag.timeline_hover {
            return;
        }
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
                        let added: Vec<_> = effects_catalog::instantiate(&types).collect();
                        effects.splice(index..index, added);
                    }
                    DragSource::Chain { units, .. } if duplicate => {
                        let selected: HashSet<_> = units.into_iter().collect();
                        let copies: Vec<_> = effects
                            .iter()
                            .filter(|e| selected.contains(&e.id))
                            .map(daw_core::EffectInstance::duplicate)
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
    pub(super) fn cancel_released_effect_drag(&mut self, ui: &egui::Ui) {
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(drag) = self.effects_drag.take()
        {
            drag::restore_focus(ui.ctx(), drag.focus);
        }
    }
    pub(super) fn scroll_effect_drag(&self, ui: &egui::Ui) {
        if self.effects_drag.as_ref().is_some_and(|drag| drag.active) {
            drag::scroll_near_edges(ui, drag::Axis::Horizontal, 360.0);
        }
    }
}
