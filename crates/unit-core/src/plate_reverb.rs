use serde::{Deserialize, Serialize};
pub const PLATE_REVERB_TYPE_ID: &str = "daw.plate-reverb";
pub const PLATE_REVERB_STATE_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlateReverbSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub predelay_ms: f32,
    pub decay_seconds: f32,
    pub highpass_hz: f32,
    pub lowpass_hz: f32,
    pub drive_db: f32,
    pub width: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub gated: bool,
    pub dark: bool,
    pub drive_enabled: bool,
    pub hq: bool,
}
impl Default for PlateReverbSettings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            predelay_ms: 0.00000f32,
            decay_seconds: 3.00000f32,
            highpass_hz: 20.00000f32,
            lowpass_hz: 15000.00000f32,
            drive_db: 0.00000f32,
            width: 1.00000f32,
            level_db: 0.00000f32,
            dry_wet: 0.50000f32,
            gated: false,
            dark: false,
            drive_enabled: false,
            hq: false,
        }
    }
}
impl PlateReverbSettings {
    pub const MAX_TAIL_SECONDS: u32 = 45;
    pub fn tail_frames(&self, rate: u32) -> u64 {
        if !self.enabled {
            0
        } else {
            u64::from(rate)
                * u64::from(if self.gated {
                    2
                } else {
                    Self::MAX_TAIL_SECONDS
                })
        }
    }
    pub fn values(&self) -> [f32; 8] {
        [
            self.predelay_ms,
            self.decay_seconds,
            self.highpass_hz,
            self.lowpass_hz,
            self.drive_db,
            self.width,
            self.level_db,
            self.dry_wet,
        ]
    }
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.predelay_ms.is_finite()
            && (0.0f32..=300.0f32).contains(&self.predelay_ms)
            && self.decay_seconds.is_finite()
            && (0.3f32..=15.0f32).contains(&self.decay_seconds)
            && self.highpass_hz.is_finite()
            && (20.0f32..=1000.0f32).contains(&self.highpass_hz)
            && self.lowpass_hz.is_finite()
            && (500.0f32..=15000.0f32).contains(&self.lowpass_hz)
            && self.drive_db.is_finite()
            && (-12.0f32..=12.0f32).contains(&self.drive_db)
            && self.width.is_finite()
            && (0.0f32..=1.0f32).contains(&self.width)
            && self.level_db.is_finite()
            && (-1000.0f32..=12.0f32).contains(&self.level_db)
            && self.dry_wet.is_finite()
            && (0.0f32..=1.0f32).contains(&self.dry_wet)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_reverb_destination_fits_the_shared_offset_storage() {
        let inputs = crate::ModulationInputs::neutral(1.0);
        for p in crate::ModulationParameter::PLATE_REVERB
            .into_iter()
            .chain(crate::ModulationParameter::AMBIENT_REVERB)
        {
            assert_eq!(inputs.offset(p), 0.0);
        }
    }
    #[test]
    fn roundtrip_bounds_and_future_payload() {
        let effect = crate::EffectInstance::plate_reverb();
        let value = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(value).unwrap(),
            effect
        );
        let settings = PlateReverbSettings {
            decay_seconds: f32::NAN,
            ..Default::default()
        };
        assert!(!settings.valid());
        let mut future = serde_json::to_value(&effect.unit).unwrap();
        future["state"]["version"] = 99.into();
        assert!(matches!(
            serde_json::from_value::<crate::EffectSettings>(future).unwrap(),
            crate::EffectSettings::Missing { .. }
        ));
    }
}
