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
/// Shared source-scope predicate for project validation and read-only drop checks.
pub(crate) fn source_available(
    instrument: Option<&InstrumentSettings>,
    source: ModulationSourceId,
) -> bool {
    instrument.is_some_and(|i| {
        i.modulation_source(source)
            .is_some_and(|s| daw_unit_core::ModulationScope::Shared.accepts(s.scope))
    })
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
    /// Add a saved mixed-unit position to an already validated effect transaction.
    /// `before` refers to a surviving destination unit; sources remain pinned first.
    pub fn position_inline_effects(
        &self,
        mut transfer: EffectTransfer,
        destination: EffectChainTarget,
        before: Option<InstrumentComponent>,
    ) -> Result<EffectTransfer> {
        let EffectChainTarget::Layer { track, layer } = destination else {
            return Ok(transfer);
        };
        let original = self
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_ref())
            .ok_or_else(|| Error("Instrument not found".into()))?;
        let (mut order, visible) = if layer == original.main.id {
            (
                original.main.retained_units(),
                original.main.visible_units(),
            )
        } else {
            let l = original
                .layers
                .iter()
                .find(|l| l.id == layer)
                .ok_or_else(|| Error("Layer not found".into()))?;
            (l.retained_units(), l.visible_units())
        };
        let selected: Vec<_> = transfer
            .selected
            .iter()
            .copied()
            .map(InstrumentComponent::Effect)
            .collect();
        order.retain(|u| !selected.contains(u));
        let at = match before {
            Some(unit) if !unit.is_source() => order
                .iter()
                .position(|u| *u == unit)
                .ok_or_else(|| Error("Invalid inline effect anchor".into()))?,
            Some(_) => return Err(Error("Audio sources must remain first".into())),
            None => order.len(),
        };
        order.splice(at..at, selected);
        let mut next = self.clone();
        next.edit(transfer.edit.clone())?;
        let instrument = next
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_mut())
            .ok_or_else(|| Error("Inline effect destination was removed".into()))?;
        if layer == instrument.main.id {
            order.retain(|u| !matches!(u,InstrumentComponent::Effect(id) if !instrument.main.effects.iter().any(|e|e.id==*id)));
            let desired: Vec<_> = order
                .iter()
                .copied()
                .filter(|u| {
                    matches!(u, InstrumentComponent::Effect(_)) || instrument.main.has_component(*u)
                })
                .collect();
            if desired != visible {
                instrument.main.component_order = order;
            }
        } else {
            let l = instrument
                .layers
                .iter_mut()
                .find(|l| l.id == layer)
                .unwrap();
            order.retain(|u| !matches!(u,InstrumentComponent::Effect(id) if !l.effects.iter().any(|e|e.id==*id)));
            let desired: Vec<_> = order
                .iter()
                .copied()
                .filter(|u| matches!(u, InstrumentComponent::Effect(_)) || l.has_component(*u))
                .collect();
            if desired != visible {
                l.component_order = order;
            }
        }
        let instrument = instrument.clone();
        next.validate()?;
        transfer.edit = Edit::Batch(vec![
            transfer.edit,
            Edit::SetInstrument {
                track_id: track,
                instrument: Some(instrument),
            },
        ]);
        Ok(transfer)
    }
    /// Read-only transfer eligibility. Does not clone the project or create copy IDs.
    pub fn can_transfer_effects(
        &self,
        source: EffectChainTarget,
        destination: EffectChainTarget,
        units: &[Id],
    ) -> bool {
        let Some((original, _)) = self.effect_chain(source) else {
            return false;
        };
        if self.effect_chain(destination).is_none() || units.is_empty() {
            return false;
        }
        let instrument = match destination {
            EffectChainTarget::Track(track) | EffectChainTarget::Layer { track, .. } => self
                .tracks
                .iter()
                .find(|t| t.id == track)
                .and_then(|t| t.instrument.as_ref()),
            EffectChainTarget::Master => None,
        };
        units.iter().enumerate().all(|(index, id)| {
            !units[..index].contains(id)
                && original.iter().find(|e| e.id == *id).is_some_and(|effect| {
                    effect.unit.modulations().iter().all(|route| {
                        route
                            .source
                            .is_none_or(|source| source_available(instrument, source))
                    })
                })
        })
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
        if !self.can_transfer_effects(source, destination, units) {
            return Err(Error(
                "Effects are not compatible with the destination chain".into(),
            ));
        }
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
    fn track_and_master_transfers_share_order_identity_and_eligibility_rules() {
        let mut p = Project::default();
        let audio = p.add_track().unwrap();
        let midi = p.add_midi_track().unwrap();
        let child = p.add_track().unwrap();
        let group = Id::new_v4();
        p.edit(Edit::GroupTracks {
            track_ids: vec![child],
            group_id: group,
            name: "Bus".into(),
        })
        .unwrap();
        let mut filter = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(settings) = &mut filter.unit else {
            unreachable!()
        };
        // Unassigned connections remain portable and receive fresh route IDs on copy.
        settings
            .modulations
            .push(Modulation::new(ModulationParameter::Frequency));
        p.master.effects = vec![EffectInstance::drive(), filter];
        let original = p.master.effects.clone();
        let ids = [original[1].id, original[0].id];
        let master = EffectChainTarget::Master;
        for track in [audio, midi, group] {
            let destination = EffectChainTarget::Track(track);
            assert!(p.can_transfer_effects(master, destination, &ids));
            let copy = p
                .transfer_effects(master, destination, &ids, 0, true)
                .unwrap();
            p.edit(copy.edit).unwrap();
            let copied = p.effect_chain(destination).unwrap().0;
            assert_eq!(copied.len(), 2);
            assert_eq!(copied[0].unit, original[0].unit);
            assert_ne!(copied[0].id, original[0].id);
            assert_ne!(
                copied[1].unit.modulations()[0].id,
                original[1].unit.modulations()[0].id
            );
            assert_eq!(copied[1].unit.modulations()[0].source, None);
        }
        let destination = EffectChainTarget::Track(audio);
        let transfer = p
            .transfer_effects(master, destination, &ids, 2, false)
            .unwrap();
        assert_eq!(transfer.selected, vec![original[0].id, original[1].id]);
        p.edit(transfer.edit).unwrap();
        assert!(p.master.effects.is_empty());
        assert_eq!(&p.effect_chain(destination).unwrap().0[2..], &original);
        assert!(!p.can_transfer_effects(destination, master, &[]));
        assert!(!p.can_transfer_effects(destination, master, &[ids[0], ids[0]]));
        assert!(!p.can_transfer_effects(destination, master, &[Id::new_v4()]));
        assert!(!p.can_transfer_effects(destination, EffectChainTarget::Track(Id::new_v4()), &ids));
    }
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
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
        // A foreign or Master destination cannot consume this instrument's provider.
        assert!(p.can_transfer_effects(a, b, &[filter_id]));
        let foreign = EffectChainTarget::Track(p.add_midi_track().unwrap());
        assert!(!p.can_transfer_effects(a, foreign, &[filter_id]));
        assert!(!p.can_transfer_effects(a, EffectChainTarget::Master, &[filter_id]));
        let before = serde_json::to_value(&p).unwrap();
        assert!(
            p.transfer_effects(a, foreign, &[filter_id], 0, true)
                .is_err()
        );
        assert!(
            p.transfer_effects(a, EffectChainTarget::Master, &[filter_id], 0, false)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
    }
}

#[cfg(test)]
mod inline_order_tests {
    use super::*;
    use InstrumentComponent::*;
    #[test]
    fn effects_and_controls_interleave_but_sources_stay_first_and_copies_retarget_layout() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        let l = &mut i.layers[0];
        l.oscillators.push(Default::default());
        l.macro_module.add();
        l.amplitude.present = true;
        l.effects = vec![EffectInstance::auto_filter(), EffectInstance::drive()];
        let owner = l.id;
        let a = l.effects[0].id;
        let b = l.effects[1].id;
        let target = EffectChainTarget::Layer {
            track,
            layer: owner,
        };
        let transfer = p.transfer_effects(target, target, &[a], 0, false).unwrap();
        let transfer = p
            .position_inline_effects(transfer, target, Some(Macros))
            .unwrap();
        p.edit(transfer.edit).unwrap();
        let l = &p.tracks[0].instrument.as_ref().unwrap().layers[0];
        assert_eq!(
            l.visible_units(),
            vec![Oscillators, Effect(a), Macros, Amplitude, Effect(b)]
        );
        assert_eq!(
            l.effects.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![a, b]
        );
        let saved = serde_json::to_value(&p).unwrap();
        assert_eq!(serde_json::from_value::<Project>(saved).unwrap(), p);
        let copy = l.duplicate();
        assert_eq!(copy.visible_units()[1], Effect(copy.effects[0].id));
        assert_ne!(copy.effects[0].id, a);
        assert!(copy.valid());
        let transfer = p.transfer_effects(target, target, &[b], 0, false).unwrap();
        assert!(
            p.position_inline_effects(transfer, target, Some(Oscillators))
                .is_err()
        );
        let transfer = p.transfer_effects(target, target, &[b], 1, false).unwrap();
        let transfer = p
            .position_inline_effects(transfer, target, Some(Macros))
            .unwrap();
        p.edit(transfer.edit).unwrap();
        let l = &p.tracks[0].instrument.as_ref().unwrap().layers[0];
        assert_eq!(
            l.visible_units(),
            vec![Oscillators, Effect(a), Effect(b), Macros, Amplitude]
        );
        let mut instrument = p.tracks[0].instrument.clone().unwrap();
        assert_eq!(instrument.layers[0].move_component(Macros, 1), Some(true));
        p.edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
        assert_eq!(
            p.tracks[0].instrument.as_ref().unwrap().layers[0].visible_units(),
            vec![Oscillators, Macros, Effect(a), Effect(b), Amplitude]
        );
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.main.macro_module.add();
        let main = EffectChainTarget::Layer {
            track,
            layer: i.main.id,
        };
        let transfer = p.transfer_effects(target, main, &[a], 0, true).unwrap();
        let copied = transfer.selected[0];
        let transfer = p
            .position_inline_effects(transfer, main, Some(Macros))
            .unwrap();
        p.edit(transfer.edit).unwrap();
        assert_eq!(
            p.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .visible_units(),
            vec![Effect(copied), Macros]
        );
        let before = p.clone();
        p.edit(Edit::SetLayerEffects {
            track_id: track,
            layer_id: owner,
            effects: vec![],
            compact: false,
        })
        .unwrap();
        assert!(
            !p.tracks[0].instrument.as_ref().unwrap().layers[0]
                .component_order
                .iter()
                .any(|u| matches!(u, Effect(_)))
        );
        p = before;
        p.edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: Id::new_v4(),
        })
        .unwrap();
        p.validate().unwrap();
    }
}

#[cfg(test)]
mod inline_invalid_order_tests {
    use super::*;
    #[test]
    fn duplicate_saved_effect_references_reject_the_batch_without_changing_the_project() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        p.tracks[0].instrument.as_mut().unwrap().layers[0]
            .effects
            .push(EffectInstance::auto_filter());
        let before = p.clone();
        let mut instrument = p.tracks[0].instrument.clone().unwrap();
        let l = &mut instrument.layers[0];
        let owner = l.id;
        let effects = l.effects.clone();
        l.component_order = vec![InstrumentComponent::Effect(effects[0].id); 2];
        assert!(
            p.edit(Edit::Batch(vec![
                Edit::SetInstrument {
                    track_id: track,
                    instrument: Some(instrument)
                },
                Edit::SetLayerEffects {
                    track_id: track,
                    layer_id: owner,
                    effects,
                    compact: false
                }
            ]))
            .is_err()
        );
        assert_eq!(p, before);
    }
}
