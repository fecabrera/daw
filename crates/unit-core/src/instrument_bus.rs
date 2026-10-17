//! Instrument output bus: shared modulators and post-layer stereo mix controls.
use crate::{
    EnvelopeInstance, EnvelopeMode, LfoInstance, MacroModuleSettings, ModulationSourceInfo,
    ModulationSourceSettings,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstrumentBusSettings {
    pub id: Uuid,
    #[serde(default)]
    pub component_order: Vec<crate::InstrumentComponent>,
    pub collapsed: bool,
    pub gain_db: f32,
    pub pan: f32,
    pub envelopes: Vec<EnvelopeInstance>,
    pub lfos: Vec<LfoInstance>,
    #[serde(default)]
    pub macro_module: MacroModuleSettings,
}
impl Default for InstrumentBusSettings {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            component_order: Vec::new(),
            collapsed: true,
            gain_db: 0.0,
            pan: 0.0,
            envelopes: Vec::new(),
            lfos: Vec::new(),
            macro_module: MacroModuleSettings::default(),
        }
    }
}
impl InstrumentBusSettings {
    /// Missing legacy state must migrate repeatably without colliding with a saved layer owner.
    pub(crate) fn legacy(layers: &[crate::InstrumentLayerSettings]) -> Self {
        let mut id = Uuid::from_u128(u128::MAX);
        while layers.iter().any(|layer| layer.id == id) {
            id = Uuid::from_u128(id.as_u128() - 1);
        }
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        !self.id.is_nil()
            && self.gain_db.is_finite()
            && 10.0_f32.powf(self.gain_db / 20.0).is_finite()
            && (-1.0..=1.0).contains(&self.pan)
            && self.envelopes.iter().all(|e| {
                !e.id.is_nil()
                    && ids.insert(e.id)
                    && e.mode != EnvelopeMode::PerNote
                    && e.settings.valid()
            })
            && self.lfos.iter().all(|l| ids.insert(l.id) && l.valid())
            && self.macro_module.valid()
            && self.valid_component_order()
            && self.macro_module.controls.iter().all(|c| ids.insert(c.id))
    }
    pub fn modulation_sources(&self) -> impl Iterator<Item = ModulationSourceInfo<'_>> {
        self.envelopes
            .iter()
            .enumerate()
            .map(|(i, e)| e.modulation_source(self.id, "Main", i))
            .chain(
                self.lfos
                    .iter()
                    .enumerate()
                    .map(|(i, l)| l.modulation_source(self.id, "Main", i)),
            )
            .chain(self.macro_module.sources(self.id, "Main"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InstrumentSettings, ModulationScope};
    #[test]
    fn main_defaults_are_neutral_and_missing_legacy_data_migrates_without_accepting_null() {
        let instrument = InstrumentSettings::default();
        assert!(instrument.main.collapsed);
        assert_eq!((instrument.main.gain_db, instrument.main.pan), (0.0, 0.0));
        assert!(instrument.main.envelopes.is_empty() && instrument.main.lfos.is_empty());
        let mut json = serde_json::to_value(&instrument).unwrap();
        json.as_object_mut().unwrap().remove("main");
        let loaded: InstrumentSettings = serde_json::from_value(json.clone()).unwrap();
        assert!(loaded.valid());
        assert_eq!(loaded.layers, instrument.layers);
        let repeated: InstrumentSettings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(repeated.main, loaded.main);
        let mut collision = json.clone();
        collision["layers"][0]["id"] = serde_json::to_value(loaded.main.id).unwrap();
        let collision: InstrumentSettings = serde_json::from_value(collision).unwrap();
        assert!(collision.valid());
        assert_ne!(collision.main.id, collision.layers[0].id);
        json["main"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<InstrumentSettings>(json).is_err());
    }
    #[test]
    fn bus_sources_share_discovery_and_reject_per_note_modes_and_identity_collisions() {
        let mut instrument = InstrumentSettings::default();
        instrument.main.envelopes.push(EnvelopeInstance {
            mode: EnvelopeMode::Legato,
            ..Default::default()
        });
        instrument.main.lfos.push(Default::default());
        assert!(instrument.valid());
        let sources: Vec<_> = instrument
            .modulation_sources()
            .filter(|s| s.id.layer == instrument.main.id)
            .collect();
        assert_eq!(sources.len(), 2);
        assert!(
            sources
                .iter()
                .all(|s| s.layer_name == "Main" && s.scope == ModulationScope::Shared)
        );
        instrument.main.envelopes[0].mode = EnvelopeMode::PerNote;
        assert!(!instrument.valid());
        instrument.main.envelopes[0].mode = EnvelopeMode::Retrigger;
        instrument.main.lfos[0].id = instrument.main.envelopes[0].id;
        assert!(!instrument.valid());
        instrument.main.lfos.clear();
        instrument.main.id = instrument.layers[0].id;
        assert!(!instrument.valid());
    }
}
