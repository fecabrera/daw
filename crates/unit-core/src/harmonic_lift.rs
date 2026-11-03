//! Native six-branch harmonic enhancement and linked maximization.
use crate::{Modulation, ModulationDestinationSettings, ModulationParameter as P, ModulationScope};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const HARMONIC_LIFT_TYPE_ID: &str = "daw.harmonic-lift";
pub const HARMONIC_LIFT_STATE_VERSION: u32 = 1;
pub const HARMONIC_BAND_NAMES: [&str; 6] =
    ["Low", "Low Mid", "Mid", "High Mid", "Presence", "High"];
pub const HARMONIC_FREQUENCY_RANGES: [[f32; 2]; 6] = [
    [20., 250.],
    [250., 500.],
    [500., 2000.],
    [2000., 4000.],
    [4000., 6000.],
    [6000., 12000.],
];
pub const HARMONIC_FREQUENCY_DEFAULTS: [f32; 6] = [135., 375., 1250., 3000., 5000., 9000.];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarmonicQuality {
    #[default]
    Normal,
    Hq,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarmonicBand {
    pub id: Uuid,
    pub enabled: bool,
    pub frequency_hz: f32,
    pub gain_db: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarmonicLiftSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub bands: [HarmonicBand; 6],
    pub input_db: f32,
    pub drive: f32,
    pub push: f32,
    pub limit_db: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub quality: HarmonicQuality,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
}
impl Default for HarmonicLiftSettings {
    fn default() -> Self {
        Self {
            version: HARMONIC_LIFT_STATE_VERSION,
            enabled: true,
            collapsed: false,
            bands: HARMONIC_FREQUENCY_DEFAULTS.map(|frequency_hz| HarmonicBand {
                id: Uuid::new_v4(),
                enabled: false,
                frequency_hz,
                gain_db: 0.,
            }),
            input_db: 0.,
            drive: 0.,
            push: 0.,
            limit_db: 0.,
            level_db: 0.,
            dry_wet: 1.,
            quality: HarmonicQuality::Normal,
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl HarmonicLiftSettings {
    pub fn valid(&self) -> bool {
        let bounded = |v: f32, a: f32, b: f32| v.is_finite() && (a..=b).contains(&v);
        let mut ids = std::collections::HashSet::new();
        self.version == HARMONIC_LIFT_STATE_VERSION
            && self.bands.iter().enumerate().all(|(i, b)| {
                !b.id.is_nil()
                    && ids.insert(b.id)
                    && bounded(
                        b.frequency_hz,
                        HARMONIC_FREQUENCY_RANGES[i][0],
                        HARMONIC_FREQUENCY_RANGES[i][1],
                    )
                    && bounded(b.gain_db, 0., 24.)
            })
            && bounded(self.input_db, -60., 12.)
            && bounded(self.drive, 0., 1.)
            && bounded(self.push, 0., 1.)
            && bounded(self.limit_db, -24., 0.)
            && bounded(self.level_db, crate::SILENT_GAIN_DB, 12.)
            && bounded(self.dry_wet, 0., 1.)
            && self.valid_modulations()
    }
}
impl ModulationDestinationSettings for HarmonicLiftSettings {
    const MODULATION_PARAMETERS: &'static [P] = &P::HARMONIC_LIFT;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        crate::valid_mixed_modulations(
            &self.modulations,
            &[P::EqBandFrequency, P::EqBandGain],
            &self.bands.iter().map(|b| b.id).collect::<Vec<_>>(),
            &P::HARMONIC_LIFT,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_state_roundtrips_and_rejects_invalid_bands() {
        let s = HarmonicLiftSettings::default();
        assert!(s.valid());
        assert_eq!(
            serde_json::from_str::<HarmonicLiftSettings>(&serde_json::to_string(&s).unwrap())
                .unwrap(),
            s
        );
        let mut bad = s.clone();
        bad.bands[0].frequency_hz = 251.0;
        assert!(!bad.valid());
        bad = s.clone();
        bad.bands[1].id = bad.bands[0].id;
        assert!(!bad.valid());
        bad = s.clone();
        bad.push = f32::NAN;
        assert!(!bad.valid());
    }
    #[test]
    fn duplicate_remaps_band_targets_and_unknown_versions_survive() {
        let mut e = crate::EffectInstance::harmonic_lift();
        let crate::EffectSettings::HarmonicLift(s) = &mut e.unit else {
            unreachable!()
        };
        let id = s.bands[2].id;
        let mut r = crate::Modulation::new(P::EqBandGain);
        r.target = Some(id);
        s.modulations.push(r);
        let copy = e.duplicate();
        let crate::EffectSettings::HarmonicLift(s) = &copy.unit else {
            unreachable!()
        };
        assert_ne!(e.id, copy.id);
        assert_ne!(s.bands[2].id, id);
        assert_eq!(s.modulations[0].target, Some(s.bands[2].id));
        assert!(s.valid());
        let mut stored = serde_json::to_value(&e.unit).unwrap();
        stored["state"]["version"] = 999.into();
        let value = serde_json::from_value::<crate::EffectSettings>(stored.clone());
        assert!(matches!(value, Ok(crate::EffectSettings::Missing { .. })));
        assert_eq!(serde_json::to_value(value.unwrap()).unwrap(), stored);
    }
}
