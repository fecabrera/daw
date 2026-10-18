use serde::{Deserialize, Serialize};

pub const BITCRUSHER_TYPE_ID: &str = "daw.bitcrusher";
pub const BITCRUSHER_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BitcrusherSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub drive_db: f32,
    pub resolution_bits: u8,
    pub downsampling: u8,
    pub level_db: f32,
    pub dry_wet: f32,
}
impl Default for BitcrusherSettings {
    fn default() -> Self {
        Self {
            version: BITCRUSHER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            drive_db: 0.0,
            resolution_bits: 24,
            downsampling: 1,
            level_db: 0.0,
            dry_wet: 1.0,
        }
    }
}
impl BitcrusherSettings {
    pub fn valid(&self) -> bool {
        self.version == BITCRUSHER_STATE_VERSION
            && self.drive_db.is_finite()
            && (0.0..=24.0).contains(&self.drive_db)
            && (1..=24).contains(&self.resolution_bits)
            && (1..=40).contains(&self.downsampling)
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
    fn bounds_integer_fields_and_future_payloads_are_validated() {
        let s = BitcrusherSettings::default();
        assert!(s.valid());
        for v in [f32::NAN, f32::INFINITY, -0.1, 24.1] {
            assert!(
                !BitcrusherSettings {
                    drive_db: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for v in [f32::NAN, f32::INFINITY, crate::SILENT_GAIN_DB - 1.0, 12.1] {
            assert!(
                !BitcrusherSettings {
                    level_db: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for v in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(
                !BitcrusherSettings {
                    dry_wet: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for v in [0, 25, 255] {
            assert!(
                !BitcrusherSettings {
                    resolution_bits: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        for v in [0, 41, 255] {
            assert!(
                !BitcrusherSettings {
                    downsampling: v,
                    ..s.clone()
                }
                .valid()
            );
        }
        let mut effect = crate::EffectInstance::bitcrusher();
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(saved).unwrap(),
            effect
        );
        for field in ["resolution_bits", "downsampling"] {
            let mut saved = serde_json::to_value(&s).unwrap();
            saved[field] = serde_json::json!(1.5);
            assert!(serde_json::from_value::<BitcrusherSettings>(saved).is_err());
        }
        let crate::EffectSettings::Bitcrusher(s) = &mut effect.unit else {
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
