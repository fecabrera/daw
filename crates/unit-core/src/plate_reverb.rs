use serde::{Deserialize, Serialize};
pub const PLATE_REVERB_TYPE_ID: &str = "daw.plate-reverb";
pub const PLATE_REVERB_STATE_VERSION: u32 = 1;
/// Discrete Plate output-drive curves. Soft retains the original transfer function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlateDriveMode {
    #[default]
    Soft = 0,
    Hard = 1,
    Tube = 2,
}
impl PlateDriveMode {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Hard, Self::Tube];
    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Hard => "Hard",
            Self::Tube => "Tube",
        }
    }
}
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
    #[serde(deserialize_with = "read_drive")]
    pub drive_db: f32,
    #[serde(default = "neutral_tone")]
    pub drive_tone: f32,
    pub width: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub gated: bool,
    pub dark: bool,
    pub drive_enabled: bool,
    #[serde(default)]
    pub drive_mode: PlateDriveMode,
    pub hq: bool,
    #[serde(default = "filter_enabled")]
    pub highpass_enabled: bool,
    #[serde(default = "filter_enabled")]
    pub lowpass_enabled: bool,
    #[serde(default = "filter_slope")]
    pub highpass_slope: crate::ReverbFilterSlope,
    #[serde(default = "filter_slope")]
    pub lowpass_slope: crate::ReverbFilterSlope,
}
fn neutral_tone() -> f32 {
    0.5
}
fn read_drive<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(d)?;
    Ok(if (-12.0..0.0).contains(&value) {
        0.0
    } else {
        value
    })
}
fn filter_slope() -> crate::ReverbFilterSlope {
    crate::ReverbFilterSlope::Db12
}
fn filter_enabled() -> bool {
    true
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
            drive_db: 2.4,
            drive_tone: neutral_tone(),
            width: 1.00000f32,
            level_db: 0.00000f32,
            dry_wet: 0.50000f32,
            gated: false,
            dark: false,
            drive_enabled: false,
            drive_mode: PlateDriveMode::default(),
            hq: false,
            highpass_enabled: true,
            lowpass_enabled: true,
            highpass_slope: filter_slope(),
            lowpass_slope: filter_slope(),
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
    pub fn values(&self) -> [f32; 9] {
        [
            self.predelay_ms,
            self.decay_seconds,
            self.highpass_hz,
            self.lowpass_hz,
            self.drive_db,
            self.width,
            self.level_db,
            self.dry_wet,
            self.drive_tone,
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
            && (0.0f32..=12.0f32).contains(&self.drive_db)
            && self.drive_tone.is_finite()
            && (0.0..=1.0).contains(&self.drive_tone)
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
    fn legacy_filters_stay_enabled_and_each_power_roundtrips() {
        let mut json = serde_json::to_value(PlateReverbSettings::default()).unwrap();
        json.as_object_mut().unwrap().remove("highpass_enabled");
        json.as_object_mut().unwrap().remove("lowpass_enabled");
        json.as_object_mut().unwrap().remove("highpass_slope");
        json.as_object_mut().unwrap().remove("lowpass_slope");
        let legacy: PlateReverbSettings = serde_json::from_value(json).unwrap();
        assert!(legacy.highpass_enabled && legacy.lowpass_enabled);
        assert_eq!(legacy.highpass_slope, crate::ReverbFilterSlope::Db12);
        assert_eq!(legacy.lowpass_slope, crate::ReverbFilterSlope::Db12);
        for highpass_enabled in [false, true] {
            for lowpass_enabled in [false, true] {
                let settings = PlateReverbSettings {
                    highpass_enabled,
                    lowpass_enabled,
                    ..legacy.clone()
                };
                let json = serde_json::to_string(&settings).unwrap();
                assert_eq!(
                    serde_json::from_str::<PlateReverbSettings>(&json).unwrap(),
                    settings
                );
            }
        }
    }
    #[test]
    fn filter_slopes_roundtrip_independently() {
        for highpass_slope in crate::ReverbFilterSlope::ALL {
            for lowpass_slope in crate::ReverbFilterSlope::ALL {
                let settings = PlateReverbSettings {
                    highpass_slope,
                    lowpass_slope,
                    ..Default::default()
                };
                let value = serde_json::to_value(&settings).unwrap();
                assert_eq!(
                    serde_json::from_value::<PlateReverbSettings>(value).unwrap(),
                    settings
                );
            }
        }
    }
    #[test]
    fn legacy_drive_defaults_to_soft_and_all_modes_roundtrip() {
        let mut value = serde_json::to_value(PlateReverbSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("drive_mode");
        let legacy: PlateReverbSettings = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.drive_mode, PlateDriveMode::Soft);
        for drive_mode in PlateDriveMode::ALL {
            let settings = PlateReverbSettings {
                drive_mode,
                ..legacy.clone()
            };
            let value = serde_json::to_value(&settings).unwrap();
            assert_eq!(
                serde_json::from_value::<PlateReverbSettings>(value).unwrap(),
                settings
            );
        }
    }
    #[test]
    fn drive_amount_defaults_and_legacy_negative_values_and_tone() {
        let settings = PlateReverbSettings::default();
        assert_eq!(settings.drive_db, 2.4);
        assert_eq!(settings.drive_tone, 0.5);
        let mut value = serde_json::to_value(&settings).unwrap();
        value["drive_db"] = (-6.0).into();
        value.as_object_mut().unwrap().remove("drive_tone");
        let old: PlateReverbSettings = serde_json::from_value(value).unwrap();
        assert_eq!(old.drive_db, 0.0);
        assert_eq!(old.drive_tone, 0.5);
        assert!(old.valid());
        assert!(
            !PlateReverbSettings {
                drive_db: -1.0,
                ..settings.clone()
            }
            .valid()
        );
        assert!(
            !PlateReverbSettings {
                drive_tone: 1.1,
                ..settings
            }
            .valid()
        );
    }
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
