use serde::{Deserialize, Serialize};
pub const SATURATOR_TYPE_ID: &str = "daw.saturator";
pub const SATURATOR_STATE_VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaturatorShape {
    #[default]
    AnalogClip,
    SoftSine,
    BassShaper,
    MediumCurve,
    HardCurve,
}
impl SaturatorShape {
    pub const ALL: [Self; 5] = [
        Self::AnalogClip,
        Self::SoftSine,
        Self::BassShaper,
        Self::MediumCurve,
        Self::HardCurve,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::AnalogClip => "Analog Clip",
            Self::SoftSine => "Soft Sine",
            Self::BassShaper => "Bass Shaper",
            Self::MediumCurve => "Medium Curve",
            Self::HardCurve => "Hard Curve",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaturatorClipMode {
    #[default]
    NoClip,
    SoftClip,
    HardClip,
}
impl SaturatorClipMode {
    pub const ALL: [Self; 3] = [Self::NoClip, Self::SoftClip, Self::HardClip];
    pub fn label(self) -> &'static str {
        match self {
            Self::NoClip => "No Clip",
            Self::SoftClip => "Soft Clip",
            Self::HardClip => "Hard Clip",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaturatorSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub shape: SaturatorShape,
    pub clip_mode: SaturatorClipMode,
    pub drive_db: f32,
    pub level_db: f32,
    pub dry_wet: f32,
}
impl Default for SaturatorSettings {
    fn default() -> Self {
        Self {
            version: SATURATOR_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            shape: Default::default(),
            clip_mode: Default::default(),
            drive_db: 0.0,
            level_db: 0.0,
            dry_wet: 1.0,
        }
    }
}
impl SaturatorSettings {
    pub fn valid(&self) -> bool {
        self.version == SATURATOR_STATE_VERSION
            && self.drive_db.is_finite()
            && (-36.0..=36.0).contains(&self.drive_db)
            && self.level_db.is_finite()
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.level_db)
            && self.dry_wet.is_finite()
            && (0.0..=1.0).contains(&self.dry_wet)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_routes_and_future_versions_follow_the_effect_contract() {
        let mut s = SaturatorSettings::default();
        assert!(s.valid());
        for v in [f32::NAN, f32::INFINITY, -36.1, 36.1] {
            s.drive_db = v;
            assert!(!s.valid());
        }
        s.drive_db = 0.0;
        for v in [f32::NAN, crate::SILENT_GAIN_DB - 1.0, 12.1] {
            s.level_db = v;
            assert!(!s.valid());
        }
        s.level_db = 0.0;
        for v in [f32::NAN, -0.1, 1.1] {
            s.dry_wet = v;
            assert!(!s.valid());
        }
        s.dry_wet = 1.0;
        let mut effect = crate::EffectInstance::saturator();
        for shape in SaturatorShape::ALL {
            for clip in SaturatorClipMode::ALL {
                let crate::EffectSettings::Saturator(s) = &mut effect.unit else {
                    unreachable!()
                };
                s.shape = shape;
                s.clip_mode = clip;
                let saved = serde_json::to_value(&effect).unwrap();
                assert_eq!(
                    serde_json::from_value::<crate::EffectInstance>(saved).unwrap(),
                    effect
                );
            }
        }
        let crate::EffectSettings::Saturator(s) = &mut effect.unit else {
            unreachable!()
        };
        s.version = 99;
        let saved = serde_json::to_value(&effect.unit).unwrap();
        let loaded: crate::EffectSettings = serde_json::from_value(saved.clone()).unwrap();
        assert!(matches!(loaded, crate::EffectSettings::Missing { .. }));
        assert!(loaded.valid());
        assert_eq!(serde_json::to_value(loaded).unwrap(), saved);
    }
}
