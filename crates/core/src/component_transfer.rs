//! Instrument-local component transfers. Project::edit validates the complete result atomically.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentComponent {
    Oscillators,
    Oscillator(Id),
    Envelope(Id),
    Lfo(Id),
    Amplitude,
}

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
    ) -> Result<()> {
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
        match component {
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
                    for layer in &mut instrument.layers {
                        remap_source(&mut layer.oscillator_modulations, old, new);
                        remap_source(&mut layer.amplitude.modulations, old, new);
                    }
                    for lfo in instrument
                        .layers
                        .iter_mut()
                        .flat_map(|l| &mut l.lfos)
                        .chain(&mut instrument.main.lfos)
                    {
                        remap_source(&mut lfo.modulations, old, new);
                    }
                    for effect in &mut track.effects {
                        remap_source(effect.unit.modulations_mut(), old, new);
                    }
                }
            }
        }
        Ok(())
    }
}

fn eligible(i: &InstrumentSettings, from: Id, to: Id, c: InstrumentComponent, copy: bool) -> bool {
    if from == to && !copy && !matches!(c, InstrumentComponent::Oscillator(_)) {
        return false;
    }
    let source = i.layers.iter().find(|l| l.id == from);
    let destination = i.layers.iter().find(|l| l.id == to);
    if (from != i.main.id && source.is_none()) || (to != i.main.id && destination.is_none()) {
        return false;
    }
    match c {
        InstrumentComponent::Oscillators => {
            source.is_some_and(|l| !l.oscillators.is_empty()) && destination.is_some()
        }
        InstrumentComponent::Oscillator(id) => {
            source.is_some_and(|l| l.oscillators.iter().any(|s| s.id == id))
                && destination.is_some()
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
