use crate::{
    AUTO_FILTER_STATE_VERSION, AUTO_FILTER_TYPE_ID, AutoFilterSettings, COMPRESSOR_STATE_VERSION,
    COMPRESSOR_TYPE_ID, CompressorSettings, DRIVE_STATE_VERSION, DRIVE_TYPE_ID, DriveSettings,
};
use crate::{BITCRUSHER_STATE_VERSION, BITCRUSHER_TYPE_ID, BitcrusherSettings};
use crate::{DIRECTION_MIXER_STATE_VERSION, DIRECTION_MIXER_TYPE_ID, DirectionMixerSettings};
use crate::{LIMITER_STATE_VERSION, LIMITER_TYPE_ID, LimiterSettings};
use crate::{OTT_STATE_VERSION, OTT_TYPE_ID, OttSettings};
use crate::{STEREO_SPREAD_STATE_VERSION, STEREO_SPREAD_TYPE_ID, StereoSpreadSettings};
use serde::{Deserialize, Serialize};

pub const EQ_TYPE_ID: &str = "daw.eq3";
pub const EQ_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EqBandSettings {
    pub gain_db: f32,
    pub frequency_hz: f32,
    pub q: f32,
    /// Off cuts this range rather than bypassing the filter.
    pub enabled: bool,
}
impl EqBandSettings {
    pub fn valid(&self) -> bool {
        self.gain_db.is_finite()
            && (-24.0..=24.0).contains(&self.gain_db)
            && self.frequency_hz.is_finite()
            && (20.0..=20_000.0).contains(&self.frequency_hz)
            && self.q.is_finite()
            && (0.1..=10.0).contains(&self.q)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EqSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    /// Low, middle, and high shelves, in serial order.
    /// Mid frequency/Q are retained for legacy data; processing uses Low/High controls.
    pub bands: [EqBandSettings; 3],
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
}
impl Default for EqSettings {
    fn default() -> Self {
        Self {
            modulation_open: false,
            modulations: Vec::new(),
            version: EQ_STATE_VERSION,
            enabled: true,
            collapsed: false,
            bands: [250.0, 1000.0, 2500.0].map(|frequency_hz| EqBandSettings {
                gain_db: 0.0,
                frequency_hz,
                q: std::f32::consts::FRAC_1_SQRT_2,
                enabled: true,
            }),
        }
    }
}
impl EqSettings {
    pub fn valid(&self) -> bool {
        self.version == EQ_STATE_VERSION
            && self.bands.iter().all(EqBandSettings::valid)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

/// Unknown effect payloads survive save/open and pass audio through.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectSettings {
    Eq3(EqSettings),
    Drive(DriveSettings),
    AutoFilter(AutoFilterSettings),
    Compressor(CompressorSettings),
    Ott(OttSettings),
    Limiter(LimiterSettings),
    DirectionMixer(DirectionMixerSettings),
    StereoSpread(StereoSpreadSettings),
    Bitcrusher(BitcrusherSettings),
    Missing {
        type_id: String,
        state: serde_json::Value,
    },
}
#[derive(Serialize, Deserialize)]
struct StoredEffect {
    type_id: String,
    state: serde_json::Value,
}
impl Serialize for EffectSettings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let stored = match self {
            Self::StereoSpread(settings) => StoredEffect {
                type_id: STEREO_SPREAD_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Bitcrusher(settings) => StoredEffect {
                type_id: BITCRUSHER_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::DirectionMixer(settings) => StoredEffect {
                type_id: DIRECTION_MIXER_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Limiter(settings) => StoredEffect {
                type_id: LIMITER_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Ott(settings) => StoredEffect {
                type_id: OTT_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Compressor(settings) => StoredEffect {
                type_id: COMPRESSOR_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Eq3(eq) => StoredEffect {
                type_id: EQ_TYPE_ID.into(),
                state: serde_json::to_value(eq).map_err(serde::ser::Error::custom)?,
            },
            Self::AutoFilter(settings) => StoredEffect {
                type_id: AUTO_FILTER_TYPE_ID.into(),
                state: serde_json::to_value(settings).map_err(serde::ser::Error::custom)?,
            },
            Self::Drive(drive) => StoredEffect {
                type_id: DRIVE_TYPE_ID.into(),
                state: serde_json::to_value(drive).map_err(serde::ser::Error::custom)?,
            },
            Self::Missing { type_id, state } => StoredEffect {
                type_id: type_id.clone(),
                state: state.clone(),
            },
        };
        stored.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for EffectSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = StoredEffect::deserialize(deserializer)?;
        if stored.type_id == BITCRUSHER_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(BITCRUSHER_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Bitcrusher)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == STEREO_SPREAD_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(STEREO_SPREAD_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::StereoSpread)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == DIRECTION_MIXER_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(DIRECTION_MIXER_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::DirectionMixer)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == LIMITER_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(LIMITER_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Limiter)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == OTT_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(OTT_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Ott)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == EQ_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(EQ_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Eq3)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == DRIVE_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(DRIVE_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Drive)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == AUTO_FILTER_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(AUTO_FILTER_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::AutoFilter)
                .map_err(serde::de::Error::custom)
        } else if stored.type_id == COMPRESSOR_TYPE_ID
            && stored
                .state
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|version| version <= u64::from(COMPRESSOR_STATE_VERSION))
        {
            serde_json::from_value(stored.state)
                .map(Self::Compressor)
                .map_err(serde::de::Error::custom)
        } else {
            Ok(Self::Missing {
                type_id: stored.type_id,
                state: stored.state,
            })
        }
    }
}
impl EffectSettings {
    /// Destination-owned routes; all current effect destinations require shared sources.
    pub fn modulations(&self) -> &[crate::Modulation] {
        match self {
            Self::AutoFilter(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Drive(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Eq3(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Compressor(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Limiter(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::DirectionMixer(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::StereoSpread(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Bitcrusher(s) => crate::ModulationDestinationSettings::modulations(s),
            Self::Ott(s) => crate::ModulationDestinationSettings::modulations(s),
            _ => &[],
        }
    }

    /// Mutable routes for identity remapping during host editing operations.
    pub fn modulations_mut(&mut self) -> &mut [crate::Modulation] {
        match self {
            Self::AutoFilter(s) => &mut s.modulations,
            Self::Drive(s) => &mut s.modulations,
            Self::Eq3(s) => &mut s.modulations,
            Self::Compressor(s) => &mut s.modulations,
            Self::Limiter(s) => &mut s.modulations,
            Self::DirectionMixer(s) => &mut s.modulations,
            Self::StereoSpread(s) => &mut s.modulations,
            Self::Bitcrusher(s) => &mut s.modulations,
            Self::Ott(s) => &mut s.modulations,
            _ => &mut [],
        }
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::Eq3(eq) => eq.valid(),
            Self::Drive(drive) => drive.valid(),
            Self::AutoFilter(settings) => settings.valid(),
            Self::Compressor(settings) => settings.valid(),
            Self::Ott(settings) => settings.valid(),
            Self::Limiter(settings) => settings.valid(),
            Self::DirectionMixer(settings) => settings.valid(),
            Self::StereoSpread(settings) => settings.valid(),
            Self::Bitcrusher(settings) => settings.valid(),
            Self::Missing { type_id, state } => {
                let known_version = match type_id.as_str() {
                    EQ_TYPE_ID => Some(EQ_STATE_VERSION),
                    DRIVE_TYPE_ID => Some(DRIVE_STATE_VERSION),
                    AUTO_FILTER_TYPE_ID => Some(AUTO_FILTER_STATE_VERSION),
                    COMPRESSOR_TYPE_ID => Some(COMPRESSOR_STATE_VERSION),
                    OTT_TYPE_ID => Some(OTT_STATE_VERSION),
                    LIMITER_TYPE_ID => Some(LIMITER_STATE_VERSION),
                    DIRECTION_MIXER_TYPE_ID => Some(DIRECTION_MIXER_STATE_VERSION),
                    STEREO_SPREAD_TYPE_ID => Some(STEREO_SPREAD_STATE_VERSION),
                    BITCRUSHER_TYPE_ID => Some(BITCRUSHER_STATE_VERSION),
                    _ => None,
                };
                !type_id.trim().is_empty()
                    && known_version.is_none_or(|known| {
                        state
                            .get("version")
                            .and_then(serde_json::Value::as_u64)
                            .is_some_and(|version| version > u64::from(known))
                    })
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectInstance {
    pub id: uuid::Uuid,
    pub unit: EffectSettings,
}
impl EffectInstance {
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = uuid::Uuid::new_v4();
        for route in copy.unit.modulations_mut() {
            route.id = uuid::Uuid::new_v4();
        }
        copy
    }

    pub fn stereo_spread() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::StereoSpread(StereoSpreadSettings::default()),
        }
    }
    pub fn bitcrusher() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Bitcrusher(BitcrusherSettings::default()),
        }
    }
    pub fn direction_mixer() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::DirectionMixer(DirectionMixerSettings::default()),
        }
    }
    pub fn limiter() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Limiter(LimiterSettings::default()),
        }
    }
    pub fn ott() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Ott(OttSettings::default()),
        }
    }
    pub fn compressor() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Compressor(CompressorSettings::default()),
        }
    }
    pub fn auto_filter() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::AutoFilter(AutoFilterSettings::default()),
        }
    }
    pub fn drive() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Drive(DriveSettings::default()),
        }
    }
    pub fn equalizer() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            unit: EffectSettings::Eq3(EqSettings::default()),
        }
    }
}
