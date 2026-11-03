use serde::{Deserialize, Serialize};
pub const AMBIENT_REVERB_TYPE_ID: &str = "daw.ambient-reverb";
pub const AMBIENT_REVERB_STATE_VERSION: u32 = 1;
/// Discrete reverb filter slopes. Ambient defaults to 6 dB/oct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ReverbFilterSlope {
    #[default]
    Db6 = 0,
    Db12 = 1,
    Db24 = 2,
}
/// Compatibility name for Ambient's original high-pass selector.
pub type AmbientHighPassSlope = ReverbFilterSlope;
impl ReverbFilterSlope {
    pub const ALL: [Self; 3] = [Self::Db6, Self::Db12, Self::Db24];
    pub fn label(self) -> &'static str {
        match self {
            Self::Db6 => "6 dB/oct",
            Self::Db12 => "12 dB/oct",
            Self::Db24 => "24 dB/oct",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientReverbSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub decay_seconds: f32,
    pub bloom: f32,
    pub tone: f32,
    pub motion: f32,
    pub width: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub hq: bool,
    pub highpass_enabled: bool,
    #[serde(default)]
    pub highpass_slope: AmbientHighPassSlope,
    #[serde(default = "lowpass_frequency")]
    pub lowpass_hz: f32,
    #[serde(default)]
    pub lowpass_enabled: bool,
    #[serde(default = "lowpass_slope")]
    pub lowpass_slope: ReverbFilterSlope,
}
fn lowpass_frequency() -> f32 {
    15000.0
}
fn lowpass_slope() -> ReverbFilterSlope {
    ReverbFilterSlope::Db12
}
impl Default for AmbientReverbSettings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            decay_seconds: 7.50000f32,
            bloom: 0.50000f32,
            tone: 0.50000f32,
            motion: 0.49750f32,
            width: 1.00000f32,
            level_db: 0.00000f32,
            dry_wet: 0.50000f32,
            hq: false,
            highpass_enabled: false,
            highpass_slope: AmbientHighPassSlope::default(),
            lowpass_hz: lowpass_frequency(),
            lowpass_enabled: true,
            lowpass_slope: lowpass_slope(),
        }
    }
}
impl AmbientReverbSettings {
    pub const MAX_TAIL_SECONDS: u32 = 180;
    pub fn tail_frames(&self, rate: u32) -> u64 {
        if !self.enabled {
            0
        } else {
            u64::from(rate) * u64::from(Self::MAX_TAIL_SECONDS)
        }
    }
    pub fn values(&self) -> [f32; 8] {
        [
            self.decay_seconds,
            self.bloom,
            self.tone,
            self.motion,
            self.width,
            self.level_db,
            self.dry_wet,
            self.lowpass_hz,
        ]
    }
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.decay_seconds.is_finite()
            && (1.0f32..=60.0f32).contains(&self.decay_seconds)
            && self.bloom.is_finite()
            && (0.0f32..=1.0f32).contains(&self.bloom)
            && self.tone.is_finite()
            && (0.0f32..=1.0f32).contains(&self.tone)
            && self.motion.is_finite()
            && (0.0f32..=1.0f32).contains(&self.motion)
            && self.width.is_finite()
            && (0.0f32..=1.0f32).contains(&self.width)
            && self.level_db.is_finite()
            && (-1000.0f32..=12.0f32).contains(&self.level_db)
            && self.dry_wet.is_finite()
            && (0.0f32..=1.0f32).contains(&self.dry_wet)
            && self.lowpass_hz.is_finite()
            && (500.0..=15000.0).contains(&self.lowpass_hz)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_default_to_six_db_and_all_slopes_roundtrip() {
        let mut value = serde_json::to_value(AmbientReverbSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("highpass_slope");
        let legacy: AmbientReverbSettings = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.highpass_slope, AmbientHighPassSlope::Db6);
        for slope in AmbientHighPassSlope::ALL {
            let settings = AmbientReverbSettings {
                highpass_slope: slope,
                ..legacy.clone()
            };
            let json = serde_json::to_string(&settings).unwrap();
            assert_eq!(
                serde_json::from_str::<AmbientReverbSettings>(&json).unwrap(),
                settings
            );
            assert!(settings.valid());
        }
    }
    #[test]
    fn missing_lowpass_fields_preserve_legacy_audio() {
        let mut value = serde_json::to_value(AmbientReverbSettings::default()).unwrap();
        for key in ["lowpass_enabled", "lowpass_hz", "lowpass_slope"] {
            value.as_object_mut().unwrap().remove(key);
        }
        let old: AmbientReverbSettings = serde_json::from_value(value).unwrap();
        assert!(!old.lowpass_enabled);
        assert_eq!(old.lowpass_hz, 15000.0);
        assert_eq!(old.lowpass_slope, ReverbFilterSlope::Db12);
        assert!(AmbientReverbSettings::default().lowpass_enabled);
    }
    #[test]
    fn roundtrip_bounds_and_future_payload() {
        let effect = crate::EffectInstance::ambient_reverb();
        let value = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(value).unwrap(),
            effect
        );
        let settings = AmbientReverbSettings {
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
