use crate::{SILENT_GAIN_DB, UNIT_STATE_VERSION, Waveform};
use serde::{Deserialize, Serialize};
/// Prepared voice count. Off retains the original single-oscillator path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OscillatorUnison {
    #[default]
    Off,
    Two,
    Four,
    Eight,
}
impl OscillatorUnison {
    pub const ALL: [Self; 4] = [Self::Off, Self::Two, Self::Four, Self::Eight];
    pub const fn voices(self) -> usize {
        match self {
            Self::Off => 1,
            Self::Two => 2,
            Self::Four => 4,
            Self::Eight => 8,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Two => "2 voices",
            Self::Four => "4 voices",
            Self::Eight => "8 voices",
        }
    }
}
fn default_detune() -> f32 {
    10.0
}
fn default_width() -> f32 {
    100.0
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OscillatorSettings {
    pub id: uuid::Uuid,
    pub version: u32,
    pub waveform: Waveform,
    /// Semitone transposition. Host MIDI pitch remains separate from source state.
    pub transpose: i8,
    pub cents: f32,
    pub unison: OscillatorUnison,
    pub detune_cents: f32,
    pub width: f32,
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
            cents: 0.0,
            unison: OscillatorUnison::Off,
            detune_cents: default_detune(),
            width: default_width(),
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
            && self.cents.is_finite()
            && (-100.0..=100.0).contains(&self.cents)
            && self.detune_cents.is_finite()
            && (0.0..=100.0).contains(&self.detune_cents)
            && self.width.is_finite()
            && (0.0..=100.0).contains(&self.width)
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
    #[serde(default)]
    cents: f32,
    #[serde(default)]
    unison: OscillatorUnison,
    #[serde(default = "default_detune")]
    detune_cents: f32,
    #[serde(default = "default_width")]
    width: f32,
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
            cents: self.cents,
            unison: self.unison,
            detune_cents: self.detune_cents,
            width: self.width,
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
    fn oscillator_cents_width_routes_validate_roundtrip_duplicate_and_remove_by_id() {
        let mut layer = crate::InstrumentLayerSettings {
            oscillators: vec![OscillatorSettings::default()],
            ..Default::default()
        };
        for parameter in [
            ModulationParameter::OscillatorCents,
            ModulationParameter::OscillatorWidth,
        ] {
            let mut route = Modulation::new(parameter);
            route.target = Some(layer.oscillators[0].id);
            route.amount = -100.0;
            layer.oscillator_modulations.push(route);
        }
        let mut output = Modulation::new(ModulationParameter::OscillatorCents);
        output.amount = 100.0;
        layer.oscillator_modulations.push(output);
        assert!(layer.valid());
        assert_eq!(
            serde_json::from_value::<crate::InstrumentLayerSettings>(
                serde_json::to_value(&layer).unwrap()
            )
            .unwrap(),
            layer
        );
        let copy = layer.duplicate();
        assert!(copy.valid());
        for route in &copy.oscillator_modulations[..2] {
            assert_eq!(route.target, Some(copy.oscillators[0].id));
        }
        assert_eq!(copy.oscillator_modulations[2].target, None);
        layer.oscillator_modulations[1].target = None;
        assert!(!layer.valid());
        layer.oscillator_modulations[1].target = Some(layer.oscillators[0].id);
        for invalid in [f32::NAN, f32::INFINITY, -100.1, 100.1] {
            layer.oscillator_modulations[0].amount = invalid;
            assert!(!layer.valid());
        }
        layer.oscillator_modulations[0].amount = 100.0;
        layer.remove_oscillator(layer.oscillators[0].id);
        assert_eq!(layer.oscillator_modulations, vec![output]);
    }

    #[test]
    fn oscillator_fine_tuning_and_unison_round_trip_and_reject_invalid_bounds() {
        let legacy: OscillatorSettings = serde_json::from_value(json!({
            "version":1,"waveform":"sine","transpose":0,"gain_db":-12.0
        }))
        .unwrap();
        assert_eq!(
            (
                legacy.cents,
                legacy.unison,
                legacy.detune_cents,
                legacy.width
            ),
            (0.0, OscillatorUnison::Off, 10.0, 100.0)
        );
        let mut instrument = InstrumentSettings::default();
        instrument.layers[0].oscillator_cents = -37.5;
        for mode in OscillatorUnison::ALL {
            instrument.layers[0].oscillators = vec![OscillatorSettings {
                cents: 72.5,
                unison: mode,
                detune_cents: 23.5,
                width: 61.0,
                ..Default::default()
            }];
            assert!(instrument.valid());
            let copy = instrument.layers[0].duplicate();
            assert_eq!(copy.oscillator_cents, -37.5);
            let mut copied_source = copy.oscillators[0].clone();
            assert_ne!(copied_source.id, instrument.layers[0].oscillators[0].id);
            copied_source.id = instrument.layers[0].oscillators[0].id;
            assert_eq!(copied_source, instrument.layers[0].oscillators[0]);
            let saved = serde_json::to_value(&instrument).unwrap();
            assert_eq!(
                serde_json::from_value::<InstrumentSettings>(saved).unwrap(),
                instrument
            );
        }
        let mut saved = serde_json::to_value(&instrument).unwrap();
        saved["layers"][0]
            .as_object_mut()
            .unwrap()
            .remove("oscillator_cents");
        assert_eq!(
            serde_json::from_value::<InstrumentSettings>(saved)
                .unwrap()
                .layers[0]
                .oscillator_cents,
            0.0
        );
        for value in [f32::NAN, f32::INFINITY, -100.1, 100.1] {
            instrument.layers[0].oscillator_cents = value;
            assert!(!instrument.valid());
            let source = OscillatorSettings {
                cents: value,
                ..Default::default()
            };
            assert!(!source.valid());
        }
        for value in [f32::NAN, f32::INFINITY, -0.1, 100.1] {
            assert!(
                !OscillatorSettings {
                    detune_cents: value,
                    ..Default::default()
                }
                .valid()
            );
            assert!(
                !OscillatorSettings {
                    width: value,
                    ..Default::default()
                }
                .valid()
            );
        }
    }

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
        assert!(l.valid_modulations());
        l.oscillator_modulations[0].target = Some(uuid::Uuid::new_v4());
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].target = Some(l.oscillators[0].id);
        l.oscillator_modulations[0].amount = 48.01;
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].amount = f32::NAN;
        assert!(!l.valid_modulations());
        l.oscillator_modulations[0].amount = 1.0;
        l.oscillator_modulations[0].parameter = ModulationParameter::OscillatorGain;
        l.oscillator_modulations[0].amount = 24.0;
        assert!(l.valid_modulations());
        let saved = serde_json::to_string(&l).unwrap();
        let loaded: crate::InstrumentLayerSettings = serde_json::from_str(&saved).unwrap();
        assert_eq!(&loaded, l);
        l.oscillator_modulations[0].amount = 24.01;
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
