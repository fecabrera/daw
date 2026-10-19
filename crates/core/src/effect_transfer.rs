//! Atomic effect-chain transfers. Copies retain source assignments and receive fresh identities.
use crate::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectChainTarget {
    Track(Id),
    /// An instrument bus: a regular layer or Main, identified by its saved owner ID.
    Layer {
        track: Id,
        layer: Id,
    },
    Master,
}
pub struct EffectTransfer {
    pub edit: Edit,
    pub selected: Vec<Id>,
}
impl EffectChainTarget {
    pub fn edit(self, effects: Vec<EffectInstance>, compact: bool) -> Edit {
        match self {
            Self::Track(track_id) => Edit::SetEffects {
                track_id,
                effects,
                compact,
            },
            Self::Layer { track, layer } => Edit::SetLayerEffects {
                track_id: track,
                layer_id: layer,
                effects,
                compact,
            },
            Self::Master => Edit::SetMasterEffects { effects, compact },
        }
    }
}
impl Project {
    pub fn effect_chain(&self, target: EffectChainTarget) -> Option<(&[EffectInstance], bool)> {
        match target {
            EffectChainTarget::Master => Some((&self.master.effects, self.master.effects_compact)),
            EffectChainTarget::Track(id) => self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .map(|t| (t.effects.as_slice(), t.effects_compact)),
            EffectChainTarget::Layer { track, layer } => {
                let i = self
                    .tracks
                    .iter()
                    .find(|t| t.id == track)?
                    .instrument
                    .as_ref()?;
                if i.main.id == layer {
                    Some((&i.main.effects, i.main.collapsed))
                } else {
                    i.layers
                        .iter()
                        .find(|l| l.id == layer)
                        .map(|l| (l.effects.as_slice(), l.collapsed))
                }
            }
        }
    }
    /// Prepare a complete validated group edit. No intermediate removal is published.
    pub fn transfer_effects(
        &self,
        source: EffectChainTarget,
        destination: EffectChainTarget,
        units: &[Id],
        index: usize,
        duplicate: bool,
    ) -> Result<EffectTransfer> {
        let (original, source_compact) = self
            .effect_chain(source)
            .ok_or_else(|| Error("Source effect chain not found".into()))?;
        let (target, target_compact) = self
            .effect_chain(destination)
            .ok_or_else(|| Error("Destination effect chain not found".into()))?;
        let ids: std::collections::HashSet<_> = units.iter().copied().collect();
        if ids.is_empty()
            || ids.len() != units.len()
            || index > target.len()
            || ids.iter().any(|id| !original.iter().any(|e| e.id == *id))
        {
            return Err(Error("Invalid effect group or insertion point".into()));
        }
        let incoming: Vec<_> = original
            .iter()
            .filter(|e| ids.contains(&e.id))
            .map(|e| if duplicate { e.duplicate() } else { e.clone() })
            .collect();
        let selected = incoming.iter().map(|e| e.id).collect();
        let mut result = target.to_vec();
        let mut edits = Vec::new();
        let index = if source == destination && !duplicate {
            let before = result[..index]
                .iter()
                .filter(|e| ids.contains(&e.id))
                .count();
            result.retain(|e| !ids.contains(&e.id));
            index - before
        } else {
            index
        };
        result.splice(index..index, incoming);
        if source != destination && !duplicate {
            edits.push(
                source.edit(
                    original
                        .iter()
                        .filter(|e| !ids.contains(&e.id))
                        .cloned()
                        .collect(),
                    source_compact,
                ),
            );
        }
        edits.push(destination.edit(result, target_compact));
        let edit = Edit::Batch(edits);
        let mut candidate = self.clone();
        candidate.edit(edit.clone())?;
        Ok(EffectTransfer { edit, selected })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_uses_the_same_effect_transactions_and_preserves_identity() {
        for constructor in [EffectInstance::drive, EffectInstance::saturator] {
            let mut p = Project::default();
            let track = p.add_midi_track().unwrap();
            let i = p.tracks[0].instrument.as_mut().unwrap();
            let regular = EffectChainTarget::Layer {
                track,
                layer: i.layers[0].id,
            };
            let main = EffectChainTarget::Layer {
                track,
                layer: i.main.id,
            };
            i.main.effects.push(constructor());
            let id = i.main.effects[0].id;
            let transfer = p.transfer_effects(main, regular, &[id], 0, true).unwrap();
            p.edit(transfer.edit).unwrap();
            let copied = p.effect_chain(regular).unwrap().0[0].id;
            assert_ne!(copied, id);
            let transfer = p
                .transfer_effects(regular, main, &[copied], 1, false)
                .unwrap();
            p.edit(transfer.edit).unwrap();
            assert!(p.effect_chain(regular).unwrap().0.is_empty());
            assert_eq!(p.all_effects().count(), 2);
            assert_eq!(p.effect_chain(main).unwrap().0[1].id, copied);
            let track_target = EffectChainTarget::Track(track);
            let transfer = p
                .transfer_effects(main, track_target, &[copied], 0, false)
                .unwrap();
            p.edit(transfer.edit).unwrap();
            let transfer = p
                .transfer_effects(track_target, main, &[copied], 1, true)
                .unwrap();
            p.edit(transfer.edit).unwrap();
            assert_eq!(p.all_effects().count(), 3);
            let before = p.clone();
            assert!(
                p.edit(main.edit(vec![p.effect_chain(main).unwrap().0[0].clone(); 2], false))
                    .is_err()
            );
            assert_eq!(
                serde_json::to_value(p).unwrap(),
                serde_json::to_value(before).unwrap()
            );
        }
    }
    #[test]
    fn groups_move_and_copy_between_layers_in_chain_order_as_one_transaction() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.layers.push(InstrumentLayerSettings::default());
        let a = EffectChainTarget::Layer {
            track,
            layer: i.layers[0].id,
        };
        let b = EffectChainTarget::Layer {
            track,
            layer: i.layers[1].id,
        };
        i.main.macro_module.add();
        let source = ModulationSourceId {
            layer: i.main.id,
            unit: i.main.macro_module.controls[0].id,
        };
        let mut filter = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(s) = &mut filter.unit else {
            unreachable!()
        };
        let mut route = Modulation::new(ModulationParameter::Frequency);
        route.source = Some(source);
        route.amount = 2.0;
        s.modulations.push(route);
        let original_route = s.modulations[0];
        let filter_id = filter.id;
        i.layers[0].effects = vec![EffectInstance::drive(), filter, EffectInstance::equalizer()];
        i.layers[1].effects.push(EffectInstance::limiter());
        let ids: Vec<_> = i.layers[0].effects.iter().map(|e| e.id).collect();
        let moved = p
            .transfer_effects(a, b, &[ids[2], ids[0]], 1, false)
            .unwrap();
        assert_eq!(moved.selected, vec![ids[0], ids[2]]);
        p.edit(moved.edit).unwrap();
        assert_eq!(p.effect_chain(a).unwrap().0[0].id, filter_id);
        assert_eq!(p.effect_chain(b).unwrap().0.len(), 3);
        let copied = p.transfer_effects(a, b, &[filter_id], 0, true).unwrap();
        let copied_id = copied.selected[0];
        assert_ne!(copied_id, filter_id);
        p.edit(copied.edit).unwrap();
        let copy = &p.effect_chain(b).unwrap().0[0];
        assert_eq!(copy.id, copied_id);
        let r = copy.unit.modulations()[0];
        assert_ne!(r.id, original_route.id);
        assert_eq!(r.source, original_route.source);
        assert_eq!(r.amount, original_route.amount);
        assert_eq!(
            p.effect_chain(a).unwrap().0[0].unit.modulations()[0],
            original_route
        );
        let before = serde_json::to_value(&p).unwrap();
        assert!(p.transfer_effects(a, b, &[Id::new_v4()], 0, false).is_err());
        assert!(p.transfer_effects(a, b, &[filter_id], 99, false).is_err());
        // A foreign or Master destination cannot consume this instrument's provider.
        assert!(
            p.transfer_effects(a, EffectChainTarget::Master, &[filter_id], 0, false)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
    }
}
