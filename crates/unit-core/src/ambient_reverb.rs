use serde::{Deserialize, Serialize};
pub const AMBIENT_REVERB_TYPE_ID: &str = "daw.ambient-reverb";
pub const AMBIENT_REVERB_STATE_VERSION: u32 = 1;
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
    pub fn values(&self) -> [f32; 7] {
        [
            self.decay_seconds,
            self.bloom,
            self.tone,
            self.motion,
            self.width,
            self.level_db,
            self.dry_wet,
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
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
