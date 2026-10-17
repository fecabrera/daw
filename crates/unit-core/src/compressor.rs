use serde::{Deserialize, Serialize};

pub const COMPRESSOR_TYPE_ID: &str = "daw.compressor";
pub const COMPRESSOR_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressorDetection {
    #[default]
    Peak,
    Rms,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompressorSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub detection: CompressorDetection,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Soft-knee width in dB, centered on threshold. Zero selects a hard knee.
    pub knee_db: f32,
    pub level_db: f32,
    pub dry_wet: f32,
}
impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            version: COMPRESSOR_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            threshold_db: -18.0,
            ratio: 4.0,
            detection: CompressorDetection::Peak,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            level_db: 0.0,
            dry_wet: 1.0,
        }
    }
}
impl CompressorSettings {
    pub fn valid(&self) -> bool {
        fn bounded(value: f32, min: f32, max: f32) -> bool {
            value.is_finite() && (min..=max).contains(&value)
        }
        self.version == COMPRESSOR_STATE_VERSION
            && bounded(self.threshold_db, -60.0, 0.0)
            && bounded(self.ratio, 1.0, 20.0)
            && bounded(self.attack_ms, 0.1, 200.0)
            && bounded(self.release_ms, 10.0, 2000.0)
            && bounded(self.knee_db, 0.0, 24.0)
            && bounded(self.level_db, crate::SILENT_GAIN_DB, 12.0)
            && bounded(self.dry_wet, 0.0, 1.0)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
