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
        if let Some(EffectTarget::Layer { track, layer }) = self.effect_catalog_target() {
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
    pub(super) fn inline_layer_effects(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        layer: InlineEffects<'_>,
        height: f32,
        compact: bool,
        sources: &[crate::modulation_panel::Source],
    ) {
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        let sources: Vec<_> = sources
            .iter()
            .filter(|s| daw_unit_core::ModulationScope::Shared.accepts(s.scope))
            .cloned()
            .collect();
        let pane = Rect::from_min_max(
            ui.cursor().min,
            Pos2::new(ui.clip_rect().right(), ui.cursor().min.y + height),
        )
        .intersect(ui.clip_rect());
        if !layer.effects.is_empty()
            && ui.input(|i| {
                i.pointer.button_pressed(egui::PointerButton::Primary)
                    && i.pointer.press_origin().is_some_and(|p| pane.contains(p))
            })
        {
            if self.instrument_ui.effects_layer != Some(layer.id) {
                self.selected_effects.clear();
            }
            self.instrument_ui.effects_layer = Some(layer.id);
            self.effects_lane_focus.set(ui, pane, true);
        }
        if !layer.effects.is_empty() {
            let (slots, remove) = self.effect_units(
                ui,
                layer.effects,
                crate::effects_panel::UnitLane {
                    compact,
                    height,
                    target: Some(EffectTarget::Layer {
                        track,
                        layer: layer.id,
                    }),
                    allowed,
                    sources: &sources,
                },
            );
            // Inline shells share their left edge. Paint separators after all fills,
            // including the boundary from the final component to the first effect.
            let mut previous = self
                .instrument_ui
                .component_slots
                .iter()
                .rev()
                .find(|(owner, _, _)| *owner == layer.id)
                .map(|(_, _, rect)| *rect);
            for (_, rect) in &slots {
                if let Some(previous) = previous {
                    panels::unit_divider(
                        ui,
                        previous.right() - 0.5,
                        previous.top()..=rect.bottom(),
                    );
                }
                previous = Some(*rect);
            }
            self.instrument_ui
                .effect_slots
                .extend(slots.into_iter().map(|(id, r)| (layer.id, id, r)));
            if let Some(id) = remove {
                layer.effects.retain(|e| e.id != id);
            }
        }
        // Keep an empty insertion destination scrollable after the final component.
        if self.effects_drag.is_some() {
            ui.allocate_exact_size(
                Vec2::new(crate::midi_editor::UNIT_STRIP_WIDTH, height),
                Sense::hover(),
            );
        }
    }
    pub(super) fn finish_inline_layer_effects(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        layer: InlineEffects<'_>,
        viewport: Rect,
        left: Option<f32>,
    ) {
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
        let effects_bounds = Rect::from_min_max(
            Pos2::new(
                left.unwrap_or(viewport.right())
                    .clamp(viewport.left(), viewport.right()),
                viewport.top(),
            ),
            viewport.max,
        );
        let clicked = crate::unit_selection::clicked(ui, effects_bounds, allowed);
        if clicked {
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
        if let Some(transfer) =
            self.finish_cross_effect_drag(ui, target, &slots, effects_bounds, allowed)
        {
            self.effects_lane_focus.set(ui, effects_bounds, true);
            let keyboard = crate::unit_selection::keyboard_focus(
                ui,
                effects_bounds,
                ui.id().with((layer.id, "inline-effect-focus")),
            );
            keyboard.request_focus();
            self.instrument_ui.pending_effect_transfer =
                Some((transfer, layer.id, effects_bounds, keyboard.id));
        }
        if self
            .effects_drag
            .as_ref()
            .is_some_and(|d| d.target == target)
            && ui.input(|i| {
                i.pointer
                    .latest_pos()
                    .is_some_and(|p| effects_bounds.contains(p))
            })
        {
            self.finish_effect_drag(ui, layer.effects, &slots, effects_bounds, allowed);
        }
    }
}
