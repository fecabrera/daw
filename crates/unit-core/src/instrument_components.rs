//! Saved presentation order. Audio sources lead; DSP topology and provider list order stay separate.
use crate::{InstrumentBusSettings, InstrumentLayerSettings};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum InstrumentComponent {
    Oscillators,
    /// An individual source strip can transfer, but is not a component unit in this order.
    Oscillator(Uuid),
    Envelope(Uuid),
    Lfo(Uuid),
    Amplitude,
    Macros,
}
impl InstrumentComponent {
    pub fn is_source(self) -> bool {
        matches!(self, Self::Oscillators | Self::Oscillator(_))
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
    // Sources are pinned even when legacy or partial saved orders omit them.
    if let Some(index) = order.iter().position(|c| c.is_source()) {
        let source = order.remove(index);
        order.insert(0, source);
    }
    order
}
fn valid(saved: &[InstrumentComponent], inventory: &[InstrumentComponent]) -> bool {
    saved.iter().enumerate().all(|(index, unit)| {
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
    let old = visible.iter().position(|c| *c == unit)?;
    let first = visible.iter().take_while(|c| c.is_source()).count();
    if unit.is_source() || index < first || index >= visible.len() {
        return None;
    }
    if old == index {
        return Some(false);
    }
    let mut order = ordered(saved, inventory);
    order.retain(|c| *c != unit);
    let mut remaining = visible.to_vec();
    remaining.remove(old);
    let at = remaining
        .get(index)
        .and_then(|next| order.iter().position(|c| c == next))
        .unwrap_or(order.len());
    order.insert(at, unit);
    *saved = order;
    Some(true)
}

impl InstrumentLayerSettings {
    fn component_inventory(&self) -> Vec<InstrumentComponent> {
        std::iter::once(InstrumentComponent::Oscillators)
            .chain(
                self.envelopes
                    .iter()
                    .map(|e| InstrumentComponent::Envelope(e.id)),
            )
            .chain(self.lfos.iter().map(|l| InstrumentComponent::Lfo(l.id)))
            .chain([InstrumentComponent::Macros, InstrumentComponent::Amplitude])
            .collect()
    }
    pub fn has_component(&self, unit: InstrumentComponent) -> bool {
        match unit {
            InstrumentComponent::Oscillators => !self.oscillators.is_empty(),
            InstrumentComponent::Envelope(id) => {
                self.envelopes.iter().any(|e| e.id == id && e.present)
            }
            InstrumentComponent::Lfo(id) => self.lfos.iter().any(|l| l.id == id && l.present),
            InstrumentComponent::Macros => self.macro_module.present,
            InstrumentComponent::Amplitude => self.amplitude.present,
            InstrumentComponent::Oscillator(_) => false,
        }
    }
    pub fn visible_components(&self) -> Vec<InstrumentComponent> {
        ordered(&self.component_order, &self.component_inventory())
            .into_iter()
            .filter(|c| self.has_component(*c))
            .collect()
    }
    pub fn valid_component_order(&self) -> bool {
        valid(&self.component_order, &self.component_inventory())
    }
    /// Final visible index; source units cannot move or be preceded by another unit.
    pub fn move_component(&mut self, unit: InstrumentComponent, index: usize) -> Option<bool> {
        let inventory = self.component_inventory();
        let visible = self.visible_components();
        move_unit(&mut self.component_order, &inventory, &visible, unit, index)
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
        ordered(&self.component_order, &self.component_inventory())
            .into_iter()
            .filter(|c| self.has_component(*c))
            .collect()
    }
    pub fn valid_component_order(&self) -> bool {
        valid(&self.component_order, &self.component_inventory())
    }
    pub fn move_component(&mut self, unit: InstrumentComponent, index: usize) -> Option<bool> {
        let inventory = self.component_inventory();
        let visible = self.visible_components();
        move_unit(&mut self.component_order, &inventory, &visible, unit, index)
    }
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
