use serde::{Deserialize, Serialize};

pub const DIRECTION_MIXER_TYPE_ID: &str = "daw.direction-mixer";
pub const DIRECTION_MIXER_STATE_VERSION: u32 = 1;

/// Format of the incoming channels; output is always left/right stereo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionMixerInput {
    #[default]
    LeftRight,
    MidSide,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectionMixerSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub input: DirectionMixerInput,
    pub direction_degrees: f32,
    pub spread: f32,
}
impl Default for DirectionMixerSettings {
    fn default() -> Self {
        Self {
            version: DIRECTION_MIXER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            input: DirectionMixerInput::LeftRight,
            direction_degrees: 0.0,
            spread: 1.0,
        }
    }
}
impl DirectionMixerSettings {
    pub fn valid(&self) -> bool {
        self.version == DIRECTION_MIXER_STATE_VERSION
            && self.direction_degrees.is_finite()
            && (-180.0..=180.0).contains(&self.direction_degrees)
            && self.spread.is_finite()
            && (0.0..=2.0).contains(&self.spread)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_validate_ranges_and_future_payloads_remain_missing_effects() {
        let settings = DirectionMixerSettings::default();
        assert!(settings.valid());
        for value in [f32::NAN, f32::INFINITY, -181.0, 181.0] {
            assert!(
                !DirectionMixerSettings {
                    direction_degrees: value,
                    ..settings.clone()
                }
                .valid()
            );
        }
        for value in [f32::NAN, f32::INFINITY, -0.01, 2.01] {
            assert!(
                !DirectionMixerSettings {
                    spread: value,
                    ..settings.clone()
                }
                .valid()
            );
        }
        let mut effect = crate::EffectInstance::direction_mixer();
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(saved).unwrap(),
            effect
        );
        let crate::EffectSettings::DirectionMixer(s) = &mut effect.unit else {
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
