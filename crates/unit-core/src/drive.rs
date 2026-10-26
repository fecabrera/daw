use serde::{Deserialize, Serialize};

pub const DRIVE_TYPE_ID: &str = "daw.drive";
pub const DRIVE_STATE_VERSION: u32 = 1;
pub const DRIVE_MAX_VOLUME_DB: f32 = 12.0;
pub const DRIVE_MIN_FREQUENCY_HZ: f32 = 250.0;
pub const DRIVE_MAX_FREQUENCY_HZ: f32 = 2000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveColorShape {
    #[default]
    Off,
    MidScoop,
    MidHump,
    MidHumpBassCut,
}
impl DriveColorShape {
    pub const ALL: [Self; 4] = [
        Self::Off,
        Self::MidScoop,
        Self::MidHump,
        Self::MidHumpBassCut,
    ];
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|shape| *shape == self)
            .unwrap_or(0)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::MidScoop => "Mid scoop",
            Self::MidHump => "Mid hump",
            Self::MidHumpBassCut => "Mid hump + bass cut",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DriveSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub volume_db: f32,
    /// Normalized 0–1 drive intensity. Zero is clean.
    pub gain: f32,
    /// Normalized 0–1 post-clipping brightness. 0.5 is neutral.
    pub tone: f32,
    pub color_shape: DriveColorShape,
    pub color_strength: f32,
    pub color_frequency_hz: f32,
}
impl Default for DriveSettings {
    fn default() -> Self {
        Self {
            version: DRIVE_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            volume_db: 0.0,
            gain: 0.2,
            tone: 0.5,
            color_shape: DriveColorShape::Off,
            color_strength: 0.5,
            color_frequency_hz: 750.0,
        }
    }
}
impl DriveSettings {
    pub fn valid(&self) -> bool {
        self.version == DRIVE_STATE_VERSION
            && self.volume_db.is_finite()
            && (crate::SILENT_GAIN_DB..=DRIVE_MAX_VOLUME_DB).contains(&self.volume_db)
            && [self.gain, self.tone, self.color_strength]
                .into_iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            && crate::ModulationDestinationSettings::valid_modulations(self)
            && self.color_frequency_hz.is_finite()
            && (DRIVE_MIN_FREQUENCY_HZ..=DRIVE_MAX_FREQUENCY_HZ).contains(&self.color_frequency_hz)
    }
}
