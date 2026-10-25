//! Variable-band equalizer settings. Band identities own their modulation routes.
use crate::{Modulation, ModulationDestinationSettings, ModulationParameter as P, ModulationScope};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MULTIBAND_EQ_TYPE_ID: &str = "daw.multiband-eq";
pub const MULTIBAND_EQ_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EqBandMode {
    #[default]
    Bell,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
}
impl EqBandMode {
    pub const ALL: [Self; 6] = [
        Self::Bell,
        Self::LowShelf,
        Self::HighShelf,
        Self::LowPass,
        Self::HighPass,
        Self::Notch,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::LowShelf => "Low shelf",
            Self::HighShelf => "High shelf",
            Self::LowPass => "Low pass",
            Self::HighPass => "High pass",
            Self::Notch => "Notch",
        }
    }
    pub fn uses_gain(self) -> bool {
        matches!(self, Self::Bell | Self::LowShelf | Self::HighShelf)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultibandEqBand {
    pub id: Uuid,
    pub mode: EqBandMode,
    pub enabled: bool,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}
impl Default for MultibandEqBand {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            mode: EqBandMode::Bell,
            enabled: true,
            frequency_hz: 1000.0,
            gain_db: 0.0,
            q: std::f32::consts::FRAC_1_SQRT_2,
        }
    }
}
impl MultibandEqBand {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && crate::EqBandSettings {
                frequency_hz: self.frequency_hz,
                gain_db: self.gain_db,
                q: self.q,
                enabled: self.enabled,
            }
            .valid()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultibandEqSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub bands: Vec<MultibandEqBand>,
    pub output_gain_db: f32,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
}
impl Default for MultibandEqSettings {
    fn default() -> Self {
        Self {
            version: MULTIBAND_EQ_STATE_VERSION,
            enabled: true,
            collapsed: false,
            bands: vec![MultibandEqBand::default()],
            output_gain_db: 0.0,
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl MultibandEqSettings {
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.version == MULTIBAND_EQ_STATE_VERSION
            && self.bands.iter().all(|b| b.valid() && ids.insert(b.id))
            && self.output_gain_db.is_finite()
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.output_gain_db)
            && self.valid_modulations()
    }
    pub fn remove_band(&mut self, id: Uuid) {
        self.bands.retain(|b| b.id != id);
        self.modulations.retain(|r| r.target != Some(id));
    }
}
impl ModulationDestinationSettings for MultibandEqSettings {
    const MODULATION_PARAMETERS: &'static [P] =
        &[P::EqBandGain, P::EqBandFrequency, P::EqBandQ, P::Level];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        crate::valid_mixed_modulations(
            &self.modulations,
            &P::EQ_BAND,
            &self.bands.iter().map(|b| b.id).collect::<Vec<_>>(),
            &[P::Level],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectInstance, EffectSettings};
    #[test]
    fn multiband_roundtrip_duplicate_removal_and_validation_keep_targets_stable() {
        let mut effect = EffectInstance::multiband_equalizer();
        let EffectSettings::MultibandEq(s) = &mut effect.unit else {
            unreachable!()
        };
        s.bands.extend((0..64).map(|_| MultibandEqBand::default()));
        let mut route = Modulation::new(P::EqBandFrequency);
        route.target = Some(s.bands[40].id);
        s.modulations.push(route);
        s.modulations.push(Modulation::new(P::Level));
        assert!(s.valid());
        let serialized = serde_json::to_string(&effect).unwrap();
        assert_eq!(
            serde_json::from_str::<EffectInstance>(&serialized).unwrap(),
            effect
        );
        let copy = effect.duplicate();
        let (EffectSettings::MultibandEq(original), EffectSettings::MultibandEq(copied)) =
            (&effect.unit, &copy.unit)
        else {
            unreachable!()
        };
        assert_ne!(effect.id, copy.id);
        for (a, b) in original.bands.iter().zip(&copied.bands) {
            assert_ne!(a.id, b.id);
        }
        assert_eq!(copied.modulations[0].target, Some(copied.bands[40].id));
        assert_ne!(original.modulations[0].id, copied.modulations[0].id);
        assert!(copied.valid());
        let mut s = original.clone();
        s.bands[1].id = s.bands[0].id;
        assert!(!s.valid());
        s = original.clone();
        s.modulations[0].target = Some(Uuid::new_v4());
        assert!(!s.valid());
        s = original.clone();
        s.modulations[1].target = Some(s.bands[0].id);
        assert!(!s.valid());
        s = original.clone();
        s.bands[0].frequency_hz = f32::NAN;
        assert!(!s.valid());
        s = original.clone();
        s.remove_band(s.bands[40].id);
        assert!(s.valid());
        assert_eq!(s.modulations.len(), 1);
        s.bands.clear();
        assert!(s.valid());
        let mut future: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        future["unit"]["state"]["version"] = 999.into();
        let future = serde_json::from_value::<EffectInstance>(future).unwrap();
        assert!(matches!(future.unit, EffectSettings::Missing { .. }));
        assert!(future.unit.valid());
    }
}
