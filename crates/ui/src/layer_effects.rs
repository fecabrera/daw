//! Inline layer effects reuse the installed-effect painter and gestures.
use super::*;
use crate::effects_panel::EffectTarget;
pub(super) struct InlineEffects<'a> {
    pub id: Id,
    pub effects: &'a mut Vec<daw_core::EffectInstance>,
}
impl DawUi {
    pub(super) fn effect_catalog_target(&self) -> Option<EffectTarget> {
        if self.view_mode == ViewMode::Instrument {
            let track = self.selected_track?;
            if !self.instrument_ui.track_effects_target
                && let Some(layer) = self.instrument_ui.selected_layer
            {
                let i = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == track)?
                    .instrument
                    .as_ref()?;
                if layer == i.lfo_lane.id || layer == i.envelope_lane.id {
                    return None;
                }
                if layer == i.main.id || i.layers.iter().any(|l| l.id == layer) {
                    return Some(EffectTarget::Layer { track, layer });
                }
            }
        }
        self.effects_target()
    }
    pub(super) fn add_catalog_effects(
        &mut self,
        track_effects: &mut Vec<daw_core::EffectInstance>,
        added: impl IntoIterator<Item = daw_core::EffectInstance>,
    ) {
        let Some(target) = self.effect_catalog_target() else {
            return;
        };
        if let EffectTarget::Layer { track, layer } = target {
            if let Some((effects, compact)) =
                self.effect_chain(EffectTarget::Layer { track, layer })
            {
                let mut effects = effects.to_vec();
                effects.extend(added);
                self.edit(Edit::SetLayerEffects {
                    track_id: track,
                    layer_id: layer,
                    effects,
                    compact,
                });
            }
        } else {
            track_effects.extend(added);
        }
    }
    pub(super) fn inline_layer_effect(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        owner: Id,
        effect: &mut daw_core::EffectInstance,
        dimensions: (f32, bool),
        sources: &[crate::modulation_panel::Source],
    ) -> Rect {
        let (height, compact) = dimensions;
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        let pane = Rect::from_min_max(
            ui.cursor().min,
            Pos2::new(ui.clip_rect().right(), ui.cursor().min.y + height),
        )
        .intersect(ui.clip_rect());
        if ui.input(|i| {
            i.pointer.button_pressed(egui::PointerButton::Primary)
                && i.pointer.press_origin().is_some_and(|p| pane.contains(p))
        }) {
            if self.instrument_ui.effects_layer != Some(owner) {
                self.selected_effects.clear();
            }
            self.instrument_ui.effects_layer = Some(owner);
            self.effects_lane_focus.set(ui, pane, true);
        }
        let sources: Vec<_> = sources
            .iter()
            .filter(|s| daw_unit_core::ModulationScope::Shared.accepts(s.scope))
            .cloned()
            .collect();
        let (slots, remove) = self.effect_units(
            ui,
            std::slice::from_mut(effect),
            crate::effects_panel::UnitLane {
                compact,
                height,
                target: Some(EffectTarget::Layer {
                    track,
                    layer: owner,
                }),
                allowed,
                sources: &sources,
            },
        );
        let rect = slots[0].1;
        self.instrument_ui
            .effect_slots
            .extend(slots.into_iter().map(|(id, r)| (owner, id, r)));
        if remove.is_some() {
            self.instrument_ui
                .inline_effect_removals
                .push((owner, effect.id));
        }
        rect
    }
    pub(super) fn inline_unit_slots(
        &self,
        owner: Id,
    ) -> Vec<(daw_core::InstrumentComponent, Rect)> {
        let mut slots: Vec<_> = self
            .instrument_ui
            .component_slots
            .iter()
            .filter(|(id, _, _)| *id == owner)
            .map(|(_, u, r)| (*u, *r))
            .chain(
                self.instrument_ui
                    .effect_slots
                    .iter()
                    .filter(|(id, _, _)| *id == owner)
                    .map(|(_, id, r)| (daw_core::InstrumentComponent::Effect(*id), *r)),
            )
            .collect();
        slots.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
        slots
    }
    pub(super) fn finish_inline_layer_effects(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        layer: InlineEffects<'_>,
        viewport: Rect,
    ) {
        for (owner, targets, enable) in &self.instrument_ui.inline_power_actions {
            if *owner == layer.id {
                for effect in layer.effects.iter_mut().filter(|e| targets.contains(&e.id)) {
                    if let Some(state) = crate::unit_power::effect_enabled(&mut effect.unit) {
                        *state = *enable;
                    }
                }
            }
        }
        self.instrument_ui
            .inline_power_actions
            .retain(|(owner, _, _)| *owner != layer.id);
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        let target = EffectTarget::Layer {
            track,
            layer: layer.id,
        };
        let slots: Vec<_> = self
            .instrument_ui
            .effect_slots
            .iter()
            .filter(|(owner, _, _)| *owner == layer.id)
            .map(|(_, id, r)| (*id, *r))
            .collect();
        let source_right = self
            .instrument_ui
            .component_slots
            .iter()
            .find(|(id, u, _)| *id == layer.id && u.is_source())
            .map_or(viewport.left(), |(_, _, r)| r.right());
        let effects_bounds = Rect::from_min_max(
            Pos2::new(source_right.max(viewport.left()), viewport.top()),
            viewport.max,
        );
        let over_component =
            self.instrument_ui.component_slots.iter().any(|(id, _, r)| {
                *id == layer.id && ui.rect_contains_pointer(r.intersect(viewport))
            });
        if over_component && crate::unit_selection::clicked(ui, viewport, allowed) {
            self.instrument_ui.effects_layer = None;
            self.selected_effects.clear();
        }
        let clicked = !layer.effects.is_empty()
            && !over_component
            && crate::unit_selection::clicked(ui, effects_bounds, allowed);
        if clicked {
            self.instrument_ui.selected_components.clear();
            self.instrument_ui.component_focus = Default::default();
            if self.instrument_ui.effects_layer != Some(layer.id) {
                self.selected_effects.clear();
            }
            self.instrument_ui.effects_layer = Some(layer.id);
            self.instrument_ui.selected_layer = Some(layer.id);
            self.instrument_ui.track_effects_target = false;
        }
        if self.instrument_ui.effects_layer == Some(layer.id) {
            self.effects_lane_focus.set(ui, effects_bounds, true);
            let keyboard = crate::unit_selection::keyboard_focus(
                ui,
                effects_bounds,
                ui.id().with((layer.id, "inline-effect-focus")),
            );
            if clicked
                && !slots
                    .iter()
                    .any(|(_, r)| ui.rect_contains_pointer(r.intersect(viewport)))
            {
                self.selected_effects.clear();
                keyboard.request_focus();
            }
            self.effect_selection_shortcuts(ui, layer.effects, allowed, &keyboard);
        }
        if ui.input(|i| i.pointer.latest_pos().is_some_and(|p| viewport.contains(p))) {
            self.retarget_library_effect_drag(target, layer.effects);
        }
        let units = self.inline_unit_slots(layer.id);
        if let Some(transfer) =
            self.finish_inline_effect_drag(ui, target, &units, effects_bounds, allowed)
        {
            let keyboard = crate::unit_selection::keyboard_focus(
                ui,
                effects_bounds,
                ui.id().with((layer.id, "inline-effect-focus")),
            );
            self.instrument_ui.pending_effect_transfer =
                Some((transfer, layer.id, effects_bounds, keyboard.id));
        }
        layer.effects.retain(|e| {
            !self
                .instrument_ui
                .inline_effect_removals
                .contains(&(layer.id, e.id))
        });
    }
}
