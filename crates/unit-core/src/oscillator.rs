use crate::{SILENT_GAIN_DB, UNIT_STATE_VERSION, Waveform};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OscillatorSettings {
    pub id: uuid::Uuid,
    pub version: u32,
    pub waveform: Waveform,
    /// Semitone transposition. Host MIDI pitch remains separate from source state.
    pub transpose: i8,
    pub gain_db: f32,
    pub muted: bool,
}
impl Default for OscillatorSettings {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            version: UNIT_STATE_VERSION,
            waveform: Waveform::Sine,
            transpose: 0,
            gain_db: -12.0,
            muted: false,
        }
    }
}
impl OscillatorSettings {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.version == UNIT_STATE_VERSION
            && (-48..=48).contains(&self.transpose)
            && self.gain_db.is_finite()
            && (SILENT_GAIN_DB..=12.0).contains(&self.gain_db)
    }
}

// Missing legacy identities are assigned by original position, independently of settings.
// Explicit identities (including invalid nil or duplicates) are never repaired.
#[derive(Deserialize)]
struct SavedOscillator {
    #[serde(default, deserialize_with = "saved_id")]
    id: Option<uuid::Uuid>,
    version: u32,
    waveform: Waveform,
    transpose: i8,
    gain_db: f32,
    #[serde(default)]
    muted: bool,
}
pub(crate) fn saved_id<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<uuid::Uuid>, D::Error> {
    uuid::Uuid::deserialize(d).map(Some)
}
impl SavedOscillator {
    fn settings(self, legacy_id: uuid::Uuid) -> OscillatorSettings {
        OscillatorSettings {
            id: self.id.unwrap_or(legacy_id),
            version: self.version,
            waveform: self.waveform,
            transpose: self.transpose,
            gain_db: self.gain_db,
            muted: self.muted,
        }
    }
}
impl<'de> Deserialize<'de> for OscillatorSettings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SavedOscillator::deserialize(d).map(|saved| saved.settings(uuid::Uuid::from_u128(1)))
    }
}
pub(crate) fn saved_oscillators<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<OscillatorSettings>, D::Error> {
    let saved = Vec::<SavedOscillator>::deserialize(d)?;
    let mut used: std::collections::HashSet<_> = saved.iter().filter_map(|s| s.id).collect();
    let mut candidate = 1;
    Ok(saved
        .into_iter()
        .map(|s| {
            while used.contains(&uuid::Uuid::from_u128(candidate)) {
                candidate += 1;
            }
            let legacy_id = uuid::Uuid::from_u128(candidate);
            if s.id.is_none() {
                used.insert(legacy_id);
            }
            s.settings(legacy_id)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InstrumentSettings, Modulation, ModulationDestinationSettings, ModulationParameter,
    };
    use serde_json::json;

    #[test]
    fn legacy_lists_assign_repeatable_unique_ids_and_preserve_explicit_ids() {
        let source = json!({"version":1,"waveform":"sine","transpose":0,"gain_db":-12.0});
        let saved = json!({"version":1,"oscillators":[source.clone(),source.clone()]});
        let a: InstrumentSettings = serde_json::from_value(saved.clone()).unwrap();
        let b: InstrumentSettings = serde_json::from_value(saved).unwrap();
        assert_eq!(a, b);
        assert_ne!(a.layers[0].oscillators[0].id, a.layers[0].oscillators[1].id);
        assert!(a.valid());
        assert!(!a.layers[0].oscillator_modulation_open);
        assert!(a.layers[0].oscillator_modulations.is_empty());
        let mut mixed = serde_json::to_value(&a).unwrap();
        mixed["layers"][0]["oscillators"][0]
            .as_object_mut()
            .unwrap()
            .remove("id");
        let decoded: InstrumentSettings = serde_json::from_value(mixed).unwrap();
        assert_eq!(decoded, a);
        let mut malformed = serde_json::to_value(a).unwrap();
        malformed["layers"][0]["oscillators"][0]["id"] = json!(null);
        assert!(serde_json::from_value::<InstrumentSettings>(malformed).is_err());
    }

    #[test]
    fn target_validation_and_removal_cannot_redirect_a_surviving_pitch_route() {
        let mut i = InstrumentSettings::default();
        let l = &mut i.layers[0];
        l.oscillators = (0..3).map(|_| OscillatorSettings::default()).collect();
        for oscillator in &l.oscillators {
            let mut r = Modulation::new(ModulationParameter::Pitch);
            r.target = Some(oscillator.id);
            r.amount = -0.25;
            l.oscillator_modulations.push(r);
        }
        assert!(l.valid());
        let retained = l.oscillator_modulations[2];
        l.remove_oscillator(l.oscillators[1].id).unwrap();
        assert_eq!(l.oscillator_modulations[1], retained);
        assert_eq!(l.oscillators[1].id, retained.target.unwrap());
        l.oscillators.push(OscillatorSettings::default());
        assert_ne!(l.oscillators[2].id, retained.target.unwrap());
        let saved = serde_json::to_value(&i).unwrap();
        assert_eq!(
            serde_json::from_value::<InstrumentSettings>(saved).unwrap(),
            i
        );
        let l = &mut i.layers[0];
        l.oscillator_modulations[0].target = None;
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].target = Some(uuid::Uuid::new_v4());
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].target = Some(l.oscillators[0].id);
        l.oscillator_modulations[0].amount = 48.01;
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].amount = f32::NAN;
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].amount = 1.0;
        l.oscillator_modulations[0].parameter = ModulationParameter::Frequency;
        assert!(!l.valid_modulations());
        l.oscillator_modulations.clear();
        l.oscillators.push(l.oscillators[0].clone());
        assert!(!l.valid());
        l.oscillators.pop();
        l.oscillators[0].id = uuid::Uuid::nil();
        assert!(!l.valid());
    }
}
