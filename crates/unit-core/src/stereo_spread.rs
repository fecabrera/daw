use serde::{Deserialize, Serialize};

pub const STEREO_SPREAD_TYPE_ID: &str = "daw.stereo-spread";
pub const STEREO_SPREAD_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StereoSpreadSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub lower_intensity: f32,
    pub upper_intensity: f32,
    pub lower_frequency_hz: f32,
    pub upper_frequency_hz: f32,
    pub order: u8,
}
impl Default for StereoSpreadSettings {
    fn default() -> Self {
        Self {
            version: STEREO_SPREAD_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            lower_intensity: 0.0,
            upper_intensity: 0.0,
            lower_frequency_hz: 20.0,
            upper_frequency_hz: 20_000.0,
            order: 8,
        }
    }
}
impl StereoSpreadSettings {
    /// Keep a one-Hz gap, including after modulation. Upper takes precedence.
    pub fn constrained_frequencies(lower: f32, upper: f32) -> [f32; 2] {
        let upper = upper.clamp(21.0, 20_000.0);
        [lower.clamp(20.0, upper - 1.0), upper]
    }
    pub fn valid(&self) -> bool {
        self.version == STEREO_SPREAD_STATE_VERSION
            && [self.lower_intensity, self.upper_intensity]
                .iter()
                .all(|v| v.is_finite() && (-1.0..=1.0).contains(v))
            && [self.lower_frequency_hz, self.upper_frequency_hz]
                .iter()
                .all(|v| v.is_finite() && (20.0..=20_000.0).contains(v))
            && self.upper_frequency_hz - self.lower_frequency_hz >= 1.0
            && (2..=12).contains(&self.order)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_validate_bounds_and_retain_future_effect_data() {
        let s = StereoSpreadSettings::default();
        assert!(s.valid());
        for v in [f32::NAN, f32::INFINITY, -1.1, 1.1] {
            assert!(
                !StereoSpreadSettings {
                    lower_intensity: v,
                    ..s.clone()
                }
                .valid()
            );
            assert!(
                !StereoSpreadSettings {
                    upper_intensity: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for v in [f32::NAN, f32::INFINITY, 0.0, 20001.0] {
            assert!(
                !StereoSpreadSettings {
                    lower_frequency_hz: v,
                    ..s.clone()
                }
                .valid()
            );
            assert!(
                !StereoSpreadSettings {
                    upper_frequency_hz: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for order in [0, 1, 13, 255] {
            assert!(!StereoSpreadSettings { order, ..s.clone() }.valid());
        }
        for upper in [19.0, 20.0, 20.5] {
            assert!(
                !StereoSpreadSettings {
                    upper_frequency_hz: upper,
                    ..s.clone()
                }
                .valid()
            );
        }
        let mut effect = crate::EffectInstance::stereo_spread();
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(saved).unwrap(),
            effect
        );
        let crate::EffectSettings::StereoSpread(s) = &mut effect.unit else {
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
