//! Saved presentation order. Audio sources lead; DSP topology and provider list order stay separate.
use crate::{InstrumentBusSettings, InstrumentLayerSettings};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum InstrumentComponent {
    Oscillators,
    Sampler,
    /// An individual source strip can transfer, but is not a component unit in this order.
    Oscillator(Uuid),
    Envelope(Uuid),
    Lfo(Uuid),
    Amplitude,
    Macros,
    /// Saved inline effect position; transfers use the effect-chain API.
    Effect(Uuid),
}
impl InstrumentComponent {
    pub fn is_source(self) -> bool {
        matches!(
            self,
            Self::Oscillators | Self::Sampler | Self::Oscillator(_)
        )
    }
}

fn ordered(
    saved: &[InstrumentComponent],
    inventory: &[InstrumentComponent],
) -> Vec<InstrumentComponent> {
    let mut order = Vec::with_capacity(inventory.len());
    for unit in saved.iter().chain(inventory) {
        if inventory.contains(unit) && !order.contains(unit) {
            order.push(*unit);
        }
    }
    // Hidden source positions cannot put the active source behind another component.
    order.sort_by_key(|c| !c.is_source());
    order
}
fn valid(saved: &[InstrumentComponent], inventory: &[InstrumentComponent]) -> bool {
    // Missing effect positions append, so saved positions must be a chain prefix.
    let effect_order = saved
        .iter()
        .filter(|c| matches!(c, InstrumentComponent::Effect(_)));
    let chain_order = inventory
        .iter()
        .filter(|c| matches!(c, InstrumentComponent::Effect(_)));
    effect_order.zip(chain_order).all(|(a, b)| a == b)
        && saved.iter().enumerate().all(|(index, unit)| {
            inventory.contains(unit)
                && !saved[..index].contains(unit)
                && (!unit.is_source() || index == 0)
        })
}
fn move_unit(
    saved: &mut Vec<InstrumentComponent>,
    inventory: &[InstrumentComponent],
    visible: &[InstrumentComponent],
    unit: InstrumentComponent,
    index: usize,
) -> Option<bool> {
    if unit.is_source() || matches!(unit, InstrumentComponent::Effect(_)) || index >= visible.len()
    {
        return None;
    }
    place_units(saved, inventory, visible, &[unit], index)
}

fn place_units(
    saved: &mut Vec<InstrumentComponent>,
    inventory: &[InstrumentComponent],
    visible: &[InstrumentComponent],
    units: &[InstrumentComponent],
    index: usize,
) -> Option<bool> {
    if units.is_empty()
        || units.iter().enumerate().any(|(n, c)| {
            matches!(c, InstrumentComponent::Effect(_))
                || !visible.contains(c)
                || units[..n].contains(c)
        })
    {
        return None;
    }
    let moving: Vec<_> = units.iter().copied().filter(|c| !c.is_source()).collect();
    let mut rest: Vec<_> = visible
        .iter()
        .copied()
        .filter(|c| !moving.contains(c))
        .collect();
    let first = rest.iter().take_while(|c| c.is_source()).count();
    if index < first || index > rest.len() {
        return None;
    }
    let anchor = rest.get(index).copied();
    rest.splice(index..index, moving.iter().copied());
    if rest == visible {
        return Some(false);
    }
    let mut order = ordered(saved, inventory);
    order.retain(|c| !moving.contains(c));
    let at = anchor
        .and_then(|c| order.iter().position(|entry| *entry == c))
        .unwrap_or(order.len());
    order.splice(at..at, moving);
    *saved = order;
    Some(true)
}

/// Both layer types retain the same provider settings when their panels are removed.
fn remove_providers(
    units: &[InstrumentComponent],
    envelopes: &mut [crate::EnvelopeInstance],
    lfos: &mut [crate::LfoInstance],
    macros: &mut crate::MacroModuleSettings,
) {
    for unit in units {
        match *unit {
            InstrumentComponent::Envelope(id) => {
                envelopes.iter_mut().find(|e| e.id == id).unwrap().present = false
            }
            InstrumentComponent::Lfo(id) => {
                lfos.iter_mut().find(|l| l.id == id).unwrap().present = false
            }
            InstrumentComponent::Macros => macros.present = false,
            InstrumentComponent::Sampler
            | InstrumentComponent::Oscillators
            | InstrumentComponent::Oscillator(_)
            | InstrumentComponent::Amplitude
            | InstrumentComponent::Effect(_) => {}
        }
    }
}

impl InstrumentLayerSettings {
    fn component_inventory(&self) -> Vec<InstrumentComponent> {
        std::iter::once(InstrumentComponent::Oscillators)
            .chain(self.sampler.as_ref().map(|_| InstrumentComponent::Sampler))
            .chain(
                self.envelopes
                    .iter()
                    .map(|e| InstrumentComponent::Envelope(e.id)),
            )
            .chain(self.lfos.iter().map(|l| InstrumentComponent::Lfo(l.id)))
            .chain([InstrumentComponent::Macros, InstrumentComponent::Amplitude])
            .chain(
                self.effects
                    .iter()
                    .map(|e| InstrumentComponent::Effect(e.id)),
            )
            .collect()
    }
    pub fn has_component(&self, unit: InstrumentComponent) -> bool {
        match unit {
            InstrumentComponent::Sampler => self.sampler.is_some(),
            InstrumentComponent::Oscillators => !self.oscillators.is_empty(),
            InstrumentComponent::Envelope(id) => {
                self.envelopes.iter().any(|e| e.id == id && e.present)
            }
            InstrumentComponent::Lfo(id) => self.lfos.iter().any(|l| l.id == id && l.present),
            InstrumentComponent::Macros => self.macro_module.present,
            InstrumentComponent::Amplitude => self.amplitude.present,
            InstrumentComponent::Oscillator(_) | InstrumentComponent::Effect(_) => false,
        }
    }
    pub fn visible_components(&self) -> Vec<InstrumentComponent> {
        self.visible_units()
            .into_iter()
            .filter(|c| !matches!(c, InstrumentComponent::Effect(_)))
            .collect()
    }
    /// Shared display order. Effect subsequences follow the serial processing chain.
    pub fn visible_units(&self) -> Vec<InstrumentComponent> {
        self.retained_units()
            .into_iter()
            .filter(|c| matches!(c, InstrumentComponent::Effect(_)) || self.has_component(*c))
            .collect()
    }
    /// Includes hidden retained controls, but only installed audio sources.
    pub fn retained_units(&self) -> Vec<InstrumentComponent> {
        ordered(&self.component_order, &self.component_inventory())
            .into_iter()
            .filter(|c| !c.is_source() || self.has_component(*c))
            .collect()
    }
    /// Remove installed units together. Retained providers/stages keep their IDs and settings.
    /// Oscillator removal follows source-strip removal: discard sources and their pitch routes.
    pub fn remove_components(&mut self, units: &[InstrumentComponent]) -> Option<bool> {
        if units
            .iter()
            .enumerate()
            .any(|(n, c)| !self.has_component(*c) || units[..n].contains(c))
        {
            return None;
        }
        remove_providers(
            units,
            &mut self.envelopes,
            &mut self.lfos,
            &mut self.macro_module,
        );
        if units.contains(&InstrumentComponent::Oscillators) {
            self.oscillators.clear();
            self.oscillator_modulations.clear();
        }
        if units.contains(&InstrumentComponent::Sampler) {
            self.sampler = None;
            self.prune_component_order();
        }
        if units.contains(&InstrumentComponent::Amplitude) {
            self.amplitude.present = false;
        }
        Some(!units.is_empty())
    }
    pub fn valid_component_order(&self) -> bool {
        valid(&self.component_order, &self.component_inventory())
    }
    /// Final visible index; source units cannot move or be preceded by another unit.
    pub fn move_component(&mut self, unit: InstrumentComponent, index: usize) -> Option<bool> {
        let inventory = self.retained_units();
        let visible = self.visible_units();
        move_unit(&mut self.component_order, &inventory, &visible, unit, index)
    }
    /// Place a complete group at a final visible index after removing its movable units.
    /// Sources remain pinned; retained hidden unit positions and DSP vectors stay separate.
    pub fn place_components(
        &mut self,
        units: &[InstrumentComponent],
        index: usize,
    ) -> Option<bool> {
        let inventory = self.retained_units();
        let visible = self.visible_units();
        place_units(
            &mut self.component_order,
            &inventory,
            &visible,
            units,
            index,
        )
    }
    /// Transfers remove concrete providers; retained hidden units keep their layout slot.
    pub fn prune_component_order(&mut self) {
        let inventory = self.component_inventory();
        self.component_order.retain(|c| inventory.contains(c));
    }
}
impl InstrumentBusSettings {
    fn component_inventory(&self) -> Vec<InstrumentComponent> {
        self.envelopes
            .iter()
            .map(|e| InstrumentComponent::Envelope(e.id))
            .chain(self.lfos.iter().map(|l| InstrumentComponent::Lfo(l.id)))
            .chain([InstrumentComponent::Macros])
            .chain(
                self.effects
                    .iter()
                    .map(|e| InstrumentComponent::Effect(e.id)),
            )
            .collect()
    }
    pub fn has_component(&self, unit: InstrumentComponent) -> bool {
        match unit {
            InstrumentComponent::Envelope(id) => {
                self.envelopes.iter().any(|e| e.id == id && e.present)
            }
            InstrumentComponent::Lfo(id) => self.lfos.iter().any(|l| l.id == id && l.present),
            InstrumentComponent::Macros => self.macro_module.present,
            _ => false,
        }
    }
    pub fn visible_components(&self) -> Vec<InstrumentComponent> {
        self.visible_units()
            .into_iter()
            .filter(|c| !matches!(c, InstrumentComponent::Effect(_)))
            .collect()
    }
    /// Shared display order. Effect subsequences follow the serial processing chain.
    pub fn visible_units(&self) -> Vec<InstrumentComponent> {
        self.retained_units()
            .into_iter()
            .filter(|c| matches!(c, InstrumentComponent::Effect(_)) || self.has_component(*c))
            .collect()
    }
    /// Includes hidden retained controls, but only installed audio sources.
    pub fn retained_units(&self) -> Vec<InstrumentComponent> {
        ordered(&self.component_order, &self.component_inventory())
            .into_iter()
            .filter(|c| !c.is_source() || self.has_component(*c))
            .collect()
    }
    /// Main retains removed provider settings and identities for catalog restoration.
    pub fn remove_components(&mut self, units: &[InstrumentComponent]) -> Option<bool> {
        if units
            .iter()
            .enumerate()
            .any(|(n, c)| !self.has_component(*c) || units[..n].contains(c))
        {
            return None;
        }
        remove_providers(
            units,
            &mut self.envelopes,
            &mut self.lfos,
            &mut self.macro_module,
        );
        Some(!units.is_empty())
    }
    pub fn valid_component_order(&self) -> bool {
        valid(&self.component_order, &self.component_inventory())
    }
    pub fn move_component(&mut self, unit: InstrumentComponent, index: usize) -> Option<bool> {
        let inventory = self.retained_units();
        let visible = self.visible_units();
        move_unit(&mut self.component_order, &inventory, &visible, unit, index)
    }
    /// Place a complete group at a final visible index after removing its movable units.
    /// Sources remain pinned; retained hidden unit positions and DSP vectors stay separate.
    pub fn place_components(
        &mut self,
        units: &[InstrumentComponent],
        index: usize,
    ) -> Option<bool> {
        let inventory = self.retained_units();
        let visible = self.visible_units();
        place_units(
            &mut self.component_order,
            &inventory,
            &visible,
            units,
            index,
        )
    }
    /// Transfers remove concrete providers; retained hidden units keep their layout slot.
    pub fn prune_component_order(&mut self) {
        let inventory = self.component_inventory();
        self.component_order.retain(|c| inventory.contains(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    #[test]
    fn removal_keeps_retained_settings_and_layout_and_rejects_invalid_groups_atomically() {
        use InstrumentComponent::*;
        let mut layer = InstrumentLayerSettings::default();
        layer.oscillators.push(Default::default());
        layer.envelopes[0].present = true;
        layer.lfos.push(Default::default());
        layer.macro_module.add();
        layer.amplitude.present = true;
        layer.oscillator_modulations.push(Modulation {
            target: Some(layer.oscillators[0].id),
            amount: 12.0,
            ..Modulation::new(ModulationParameter::Pitch)
        });
        let envelope = Envelope(layer.envelopes[0].id);
        layer.move_component(Amplitude, 1).unwrap();
        let before = layer.clone();
        for units in [
            vec![envelope, Lfo(Uuid::new_v4())],
            vec![envelope, envelope],
            vec![Oscillator(layer.oscillators[0].id)],
        ] {
            assert_eq!(layer.remove_components(&units), None);
            assert_eq!(layer, before);
        }
        assert_eq!(layer.remove_components(&[]), Some(false));
        let all = layer.visible_components();
        assert_eq!(layer.remove_components(&all), Some(true));
        assert!(layer.visible_components().is_empty());
        assert!(layer.oscillators.is_empty() && layer.oscillator_modulations.is_empty());
        layer = serde_json::from_str(&serde_json::to_string(&layer).unwrap()).unwrap();
        layer.envelopes[0].present = true;
        layer.lfos[0].present = true;
        layer.macro_module.present = true;
        layer.amplitude.present = true;
        let mut expected = before;
        expected.oscillators.clear();
        expected.oscillator_modulations.clear();
        assert_eq!(layer, expected);
        assert!(layer.valid());
        let mut bus = InstrumentBusSettings {
            envelopes: layer.envelopes.clone(),
            lfos: layer.lfos.clone(),
            macro_module: layer.macro_module.clone(),
            ..Default::default()
        };
        for envelope in &mut bus.envelopes {
            envelope.mode = EnvelopeMode::Legato;
        }
        let before = bus.clone();
        assert_eq!(bus.remove_components(&[envelope, Amplitude]), None);
        assert_eq!(bus, before);
        let all = bus.visible_components();
        assert_eq!(bus.remove_components(&all), Some(true));
        assert!(bus.visible_components().is_empty());
        bus = serde_json::from_str(&serde_json::to_string(&bus).unwrap()).unwrap();
        bus.envelopes[0].present = true;
        bus.lfos[0].present = true;
        bus.macro_module.present = true;
        assert_eq!(bus, before);
        assert!(bus.valid());
    }
    #[test]
    fn component_layout_pins_sources_and_retains_hidden_identity_without_changing_units() {
        use InstrumentComponent::*;
        let mut layer = InstrumentLayerSettings::default();
        layer.oscillators.push(Default::default());
        layer.envelopes[0].present = true;
        layer.lfos.push(Default::default());
        layer.macro_module.add();
        layer.amplitude.present = true;
        let envelope = Envelope(layer.envelopes[0].id);
        let lfo = Lfo(layer.lfos[0].id);
        assert_eq!(
            layer.visible_components(),
            [Oscillators, envelope, lfo, Macros, Amplitude]
        );
        let before = layer.clone();
        assert_eq!(layer.move_component(Amplitude, 1), Some(true));
        assert_eq!(
            layer.visible_components(),
            [Oscillators, Amplitude, envelope, lfo, Macros]
        );
        let order = layer.component_order.clone();
        let mut expected = before;
        expected.component_order = order.clone();
        assert_eq!(layer, expected);
        for (unit, index) in [(Oscillators, 1), (Macros, 0), (Macros, 5)] {
            assert_eq!(layer.move_component(unit, index), None);
            assert_eq!(layer, expected);
        }
        assert_eq!(layer.move_component(Amplitude, 1), Some(false));
        layer.envelopes[0].present = false;
        assert!(!layer.visible_components().contains(&envelope));
        layer.envelopes[0].present = true;
        assert_eq!(layer, expected);
        layer.lfos.push(Default::default());
        assert_eq!(
            layer.visible_components().last(),
            Some(&Lfo(layer.lfos[1].id))
        );
        let id = layer.envelopes.remove(0).id;
        layer.prune_component_order();
        assert!(!layer.component_order.contains(&Envelope(id)));
        assert!(layer.valid());
    }
    #[test]
    fn saved_layout_defaults_legacy_order_and_rejects_invalid_targets_duplicates_and_source_positions()
     {
        use InstrumentComponent::*;
        let mut instrument = InstrumentSettings::default();
        instrument.layers[0].oscillators.push(Default::default());
        instrument.layers[0].amplitude.present = true;
        instrument.main.lfos.push(Default::default());
        instrument.main.macro_module.add();
        assert_eq!(instrument.main.move_component(Macros, 0), Some(true));
        let saved = serde_json::to_value(&instrument).unwrap();
        assert_eq!(
            serde_json::from_value::<InstrumentSettings>(saved.clone()).unwrap(),
            instrument
        );
        let mut legacy = saved.clone();
        legacy["layers"][0]
            .as_object_mut()
            .unwrap()
            .remove("component_order");
        legacy["main"]
            .as_object_mut()
            .unwrap()
            .remove("component_order");
        let loaded = serde_json::from_value::<InstrumentSettings>(legacy).unwrap();
        assert!(
            loaded.layers[0].component_order.is_empty() && loaded.main.component_order.is_empty()
        );
        assert!(loaded.valid());
        for order in [
            vec![Amplitude, Oscillators],
            vec![Amplitude, Amplitude],
            vec![Envelope(Uuid::new_v4())],
            vec![Oscillator(instrument.layers[0].oscillators[0].id)],
        ] {
            let mut bad = instrument.clone();
            bad.layers[0].component_order = order;
            assert!(!bad.valid());
        }
        let mut bad = instrument.clone();
        bad.main.component_order = vec![Oscillators];
        assert!(!bad.valid());
        let mut bad = saved;
        bad["main"]["component_order"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<InstrumentSettings>(bad).is_err());
    }
}
