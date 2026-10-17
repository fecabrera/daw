//! Prepare one instrument group transaction, reusing individual transfer and reference rules.
use crate::*;

pub struct ComponentTransfer {
    pub edit: Edit,
    /// Resolved destination identities, in source presentation order. Copies use fresh IDs.
    pub components: Vec<InstrumentComponent>,
    pub changed: bool,
}

impl Project {
    /// Preview eligibility without constructors, new IDs, or allocations.
    pub fn can_transfer_components(
        &self,
        track: Id,
        source: Id,
        destination: Id,
        components: &[InstrumentComponent],
        duplicate: bool,
    ) -> bool {
        let Some(i) = self
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_ref())
        else {
            return false;
        };
        !components.is_empty()
            && components.iter().enumerate().all(|(n, c)| {
                !matches!(c, InstrumentComponent::Oscillator(_))
                    && !components[..n].contains(c)
                    && if source == destination && !duplicate && components.len() > 1 {
                        if source == i.main.id {
                            i.main.has_component(*c)
                        } else {
                            i.layers
                                .iter()
                                .any(|l| l.id == source && l.has_component(*c))
                        }
                    } else {
                        crate::component_transfer::eligible(i, source, destination, *c, duplicate)
                    }
            })
    }

    /// Resolve IDs once, validate the complete group, and return one reusable Batch edit.
    /// The insertion index refers to the destination after removing units placed by the group.
    pub fn component_group_transfer(
        &self,
        track: Id,
        source: Id,
        destination: Id,
        components: &[InstrumentComponent],
        index: usize,
        duplicate: bool,
    ) -> Result<ComponentTransfer> {
        if !self.can_transfer_components(track, source, destination, components, duplicate) {
            return Err(Error("Selected components cannot enter this layer".into()));
        }
        let original = self.tracks.iter().find(|t| t.id == track).unwrap();
        let i = original.instrument.as_ref().unwrap();
        let order = if source == i.main.id {
            i.main.visible_components()
        } else {
            i.layers
                .iter()
                .find(|l| l.id == source)
                .unwrap()
                .visible_components()
        };
        let units: Vec<_> = order
            .into_iter()
            .filter(|c| components.contains(c))
            .collect();
        let mut next = self.clone();
        let mut placed = Vec::with_capacity(units.len());
        if source == destination && !duplicate {
            placed.extend(units);
        } else {
            for unit in units {
                placed.push(next.apply_component_transfer(
                    track,
                    source,
                    destination,
                    None,
                    unit,
                    duplicate,
                )?);
            }
        }
        let t = next.tracks.iter_mut().find(|t| t.id == track).unwrap();
        let i = t.instrument.as_mut().unwrap();
        let result = if destination == i.main.id {
            i.main.place_components(&placed, index)
        } else {
            i.layers
                .iter_mut()
                .find(|l| l.id == destination)
                .unwrap()
                .place_components(&placed, index)
        };
        result.ok_or_else(|| Error("Invalid component group position or pinned source".into()))?;
        next.validate()?;
        let t = next.tracks.iter().find(|t| t.id == track).unwrap();
        Ok(ComponentTransfer {
            changed: t.instrument != original.instrument || t.effects != original.effects,
            components: placed,
            edit: Edit::Batch(vec![
                Edit::SetInstrument {
                    track_id: track,
                    instrument: t.instrument.clone(),
                },
                Edit::SetEffects {
                    track_id: track,
                    effects: t.effects.clone(),
                    compact: t.effects_compact,
                },
            ]),
        })
    }
}
