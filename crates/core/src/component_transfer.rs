//! Instrument-local component transfers. Project::edit validates the complete result atomically.
use crate::*;
#[path = "macro_transfer.rs"]
mod macros;

pub use daw_unit_core::InstrumentComponent;

impl Project {
    /// Allocation-free eligibility check for drag feedback. No identities are created here.
    pub fn can_transfer_component(
        &self,
        track: Id,
        source: Id,
        destination: Id,
        component: InstrumentComponent,
        duplicate: bool,
    ) -> bool {
        self.tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_ref())
            .is_some_and(|i| eligible(i, source, destination, component, duplicate))
    }

    pub(crate) fn apply_component_transfer(
        &mut self,
        track: Id,
        source: Id,
        destination: Id,
        destination_index: Option<usize>,
        component: InstrumentComponent,
        duplicate: bool,
    ) -> Result<InstrumentComponent> {
        let track = self
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .ok_or_else(|| Error("Track not found".into()))?;
        let instrument = track
            .instrument
            .as_mut()
            .ok_or_else(|| Error("Track has no instrument".into()))?;
        if !eligible(instrument, source, destination, component, duplicate) {
            return Err(Error(
                "Component cannot be transferred to this layer".into(),
            ));
        }
        if source == destination
            && !duplicate
            && !matches!(component, InstrumentComponent::Oscillator(_))
        {
            let index = destination_index
                .ok_or_else(|| Error("Component reorder requires an insertion index".into()))?;
            let moved = if source == instrument.main.id {
                instrument.main.move_component(component, index)
            } else {
                instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == source)
                    .unwrap()
                    .move_component(component, index)
            };
            moved.ok_or_else(|| {
                Error("Invalid component insertion index or pinned source".into())
            })?;
            return Ok(component);
        }
        let mut placed = component;
        match component {
            InstrumentComponent::Sampler => {
                let from = instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == source)
                    .unwrap();
                let mut sampler = if duplicate {
                    from.sampler.clone().unwrap()
                } else {
                    from.sampler.take().unwrap()
                };
                if duplicate {
                    sampler.id = Id::new_v4();
                    fresh_routes(&mut sampler.modulations);
                }
                instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == destination)
                    .unwrap()
                    .sampler = Some(sampler);
            }
            InstrumentComponent::Oscillators | InstrumentComponent::Oscillator(_) => {
                transfer_oscillators(
                    instrument,
                    source,
                    destination,
                    destination_index,
                    component,
                    duplicate,
                )?;
            }
            InstrumentComponent::Macros => {
                let remaps = macros::transfer(instrument, source, destination, duplicate);
                if !duplicate {
                    for (old, new) in remaps {
                        remap_consumers(instrument, &mut track.effects, old, new);
                    }
                }
            }
            InstrumentComponent::Amplitude => {
                let from = instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == source)
                    .unwrap();
                let mut amplitude = from.amplitude.clone();
                if duplicate {
                    fresh_routes(&mut amplitude.modulations);
                } else {
                    // Do not leave a second retained destination with the moved route identities.
                    from.amplitude = AmplitudeSettings::default();
                }
                instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == destination)
                    .unwrap()
                    .amplitude = amplitude;
            }
            InstrumentComponent::Envelope(id) | InstrumentComponent::Lfo(id) => {
                let new_id =
                    if duplicate || providers(instrument, destination).any(|unit| unit == id) {
                        Id::new_v4()
                    } else {
                        id
                    };
                placed = match component {
                    InstrumentComponent::Envelope(_) => InstrumentComponent::Envelope(new_id),
                    _ => InstrumentComponent::Lfo(new_id),
                };
                match component {
                    InstrumentComponent::Envelope(_) => {
                        let from = envelopes_mut(instrument, source);
                        let index = from.iter().position(|e| e.id == id).unwrap();
                        let mut unit = if duplicate {
                            from[index].clone()
                        } else {
                            from.remove(index)
                        };
                        unit.id = new_id;
                        if duplicate {
                            fresh_routes(&mut unit.modulations);
                        }
                        envelopes_mut(instrument, destination).push(unit);
                    }
                    InstrumentComponent::Lfo(_) => {
                        let from = lfos_mut(instrument, source);
                        let index = from.iter().position(|e| e.id == id).unwrap();
                        let mut unit = if duplicate {
                            from[index].clone()
                        } else {
                            from.remove(index)
                        };
                        unit.id = new_id;
                        if duplicate {
                            fresh_routes(&mut unit.modulations);
                        }
                        lfos_mut(instrument, destination).push(unit);
                    }
                    _ => unreachable!(),
                }
                if !duplicate {
                    let old = ModulationSourceId {
                        layer: source,
                        unit: id,
                    };
                    let new = ModulationSourceId {
                        layer: destination,
                        unit: new_id,
                    };
                    remap_consumers(instrument, &mut track.effects, old, new);
                }
            }
        }
        for layer in &mut instrument.layers {
            layer.prune_component_order();
        }
        instrument.main.prune_component_order();
        // Strip insertion belongs to Oscillator; other indices describe the unit layout.
        if !placed.is_source()
            && let Some(index) = destination_index
        {
            let moved = if destination == instrument.main.id {
                instrument.main.move_component(placed, index)
            } else {
                instrument
                    .layers
                    .iter_mut()
                    .find(|l| l.id == destination)
                    .unwrap()
                    .move_component(placed, index)
            };
            moved.ok_or_else(|| {
                Error("Invalid component insertion index or pinned source".into())
            })?;
        }
        Ok(placed)
    }
}

pub(super) fn eligible(
    i: &InstrumentSettings,
    from: Id,
    to: Id,
    c: InstrumentComponent,
    copy: bool,
) -> bool {
    if from == to && !copy && !matches!(c, InstrumentComponent::Oscillator(_)) {
        return !c.is_source()
            && if from == i.main.id {
                i.main.has_component(c)
            } else {
                i.layers.iter().any(|l| l.id == from && l.has_component(c))
            };
    }
    let source = i.layers.iter().find(|l| l.id == from);
    let destination = i.layers.iter().find(|l| l.id == to);
    if (from != i.main.id && source.is_none()) || (to != i.main.id && destination.is_none()) {
        return false;
    }
    match c {
        InstrumentComponent::Sampler => {
            source.is_some_and(|l| l.sampler.is_some())
                && destination.is_some_and(|l| l.sampler.is_none() && l.oscillators.is_empty())
        }
        InstrumentComponent::Oscillators => {
            source.is_some_and(|l| !l.oscillators.is_empty())
                && destination.is_some_and(|l| l.sampler.is_none())
        }
        InstrumentComponent::Oscillator(id) => {
            source.is_some_and(|l| l.oscillators.iter().any(|s| s.id == id))
                && destination.is_some_and(|l| l.sampler.is_none())
        }
        InstrumentComponent::Macros => {
            let m = source.map_or(&i.main.macro_module, |l| &l.macro_module);
            m.present && m.controls.iter().any(|c| c.present)
        }
        InstrumentComponent::Amplitude => {
            source.is_some_and(|l| l.amplitude.present)
                && destination.is_some_and(|l| !l.amplitude.present)
        }
        InstrumentComponent::Envelope(id) => {
            let envelopes = source.map_or(&i.main.envelopes, |l| &l.envelopes);
            envelopes.iter().any(|e| {
                e.id == id && e.present && (to != i.main.id || e.mode != EnvelopeMode::PerNote)
            })
        }
        InstrumentComponent::Lfo(id) => source
            .map_or(&i.main.lfos, |l| &l.lfos)
            .iter()
            .any(|e| e.id == id && e.present),
    }
}
fn providers(i: &InstrumentSettings, owner: Id) -> impl Iterator<Item = Id> + '_ {
    let layer = i.layers.iter().find(|l| l.id == owner);
    layer
        .map_or(&i.main.envelopes, |l| &l.envelopes)
        .iter()
        .map(|e| e.id)
        .chain(layer.map_or(&i.main.lfos, |l| &l.lfos).iter().map(|l| l.id))
        .chain(
            layer
                .map_or(&i.main.macro_module, |l| &l.macro_module)
                .controls
                .iter()
                .map(|c| c.id),
        )
}
fn envelopes_mut(i: &mut InstrumentSettings, owner: Id) -> &mut Vec<EnvelopeInstance> {
    if i.main.id == owner {
        &mut i.main.envelopes
    } else {
        &mut i
            .layers
            .iter_mut()
            .find(|l| l.id == owner)
            .unwrap()
            .envelopes
    }
}
fn lfos_mut(i: &mut InstrumentSettings, owner: Id) -> &mut Vec<LfoInstance> {
    if i.main.id == owner {
        &mut i.main.lfos
    } else {
        &mut i.layers.iter_mut().find(|l| l.id == owner).unwrap().lfos
    }
}
fn remap_consumers(
    instrument: &mut InstrumentSettings,
    effects: &mut [EffectInstance],
    old: ModulationSourceId,
    new: ModulationSourceId,
) {
    for layer in &mut instrument.layers {
        for route in layer.modulation_routes_mut() {
            if route.source == Some(old) {
                route.source = Some(new);
            }
        }
    }
    remap_source(&mut instrument.main.macro_module.modulations, old, new);
    for envelope in &mut instrument.main.envelopes {
        remap_source(&mut envelope.modulations, old, new);
    }
    for lfo in &mut instrument.main.lfos {
        remap_source(&mut lfo.modulations, old, new);
    }
    for effect in effects.iter_mut().chain(&mut instrument.main.effects) {
        remap_source(effect.unit.modulations_mut(), old, new);
    }
}
fn remap_source(routes: &mut [Modulation], old: ModulationSourceId, new: ModulationSourceId) {
    for route in routes {
        if route.source == Some(old) {
            route.source = Some(new);
        }
    }
}
fn fresh_routes(routes: &mut [Modulation]) {
    for route in routes {
        route.id = Id::new_v4();
    }
}
fn transfer_oscillators(
    i: &mut InstrumentSettings,
    from: Id,
    to: Id,
    destination_index: Option<usize>,
    c: InstrumentComponent,
    copy: bool,
) -> Result<()> {
    if from == to && !copy {
        let layer = i.layers.iter_mut().find(|l| l.id == from).unwrap();
        let InstrumentComponent::Oscillator(id) = c else {
            unreachable!()
        };
        let old = layer.oscillators.iter().position(|s| s.id == id).unwrap();
        let index = destination_index.unwrap_or(layer.oscillators.len() - 1);
        if index >= layer.oscillators.len() {
            return Err(Error(
                "Oscillator insertion index exceeds the destination".into(),
            ));
        }
        let oscillator = layer.oscillators.remove(old);
        layer.oscillators.insert(index, oscillator);
        return Ok(());
    }
    let source = i.layers.iter_mut().find(|l| l.id == from).unwrap();
    let mut oscillators: Vec<_> = source
        .oscillators
        .iter()
        .filter(|s| {
            c == InstrumentComponent::Oscillators || c == InstrumentComponent::Oscillator(s.id)
        })
        .cloned()
        .collect();
    let ids: Vec<_> = oscillators.iter().map(|s| s.id).collect();
    let mut routes: Vec<_> = source
        .oscillator_modulations
        .iter()
        .filter(|r| r.target.is_some_and(|id| ids.contains(&id)))
        .copied()
        .collect();
    let shell = (
        source.oscillator_enabled,
        source.oscillator_collapsed,
        source.oscillator_modulation_open,
    );
    if !copy {
        source.oscillators.retain(|s| !ids.contains(&s.id));
        source
            .oscillator_modulations
            .retain(|r| !r.target.is_some_and(|id| ids.contains(&id)));
    }
    let destination = i.layers.iter_mut().find(|l| l.id == to).unwrap();
    if destination.oscillators.is_empty() {
        (
            destination.oscillator_enabled,
            destination.oscillator_collapsed,
            destination.oscillator_modulation_open,
        ) = shell;
    }
    for oscillator in &mut oscillators {
        if copy
            || destination
                .oscillators
                .iter()
                .any(|s| s.id == oscillator.id)
        {
            let old = oscillator.id;
            oscillator.id = Id::new_v4();
            for route in &mut routes {
                if route.target == Some(old) {
                    route.target = Some(oscillator.id);
                }
            }
        }
    }
    for route in &mut routes {
        if copy
            || destination
                .oscillator_modulations
                .iter()
                .any(|r| r.id == route.id)
        {
            route.id = Id::new_v4();
        }
    }
    let index = destination_index.unwrap_or(destination.oscillators.len());
    if index > destination.oscillators.len() {
        return Err(Error(
            "Oscillator insertion index exceeds the destination".into(),
        ));
    }
    destination.oscillators.splice(index..index, oscillators);
    destination.oscillator_modulations.extend(routes);
    Ok(())
}

#[cfg(test)]
#[path = "component_transfer_tests.rs"]
mod tests;
