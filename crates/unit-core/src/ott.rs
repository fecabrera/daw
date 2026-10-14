use crate::{CompressorDetection, SILENT_GAIN_DB};
use serde::{Deserialize, Serialize};

pub const OTT_TYPE_ID: &str = "daw.ott";
pub const OTT_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OttBandSettings {
    /// Bypass compression only; input/output gains still apply.
    pub enabled: bool,
    pub depth: f32,
    pub input_db: f32,
    pub output_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}
impl Default for OttBandSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            depth: 1.0,
            input_db: 0.0,
            output_db: 0.0,
            attack_ms: 10.0,
            release_ms: 100.0,
        }
    }
}
impl OttBandSettings {
    pub fn valid(&self) -> bool {
        bounded(self.depth, 0.0, 1.0)
            && bounded(self.input_db, -24.0, 24.0)
            && bounded(self.output_db, -24.0, 24.0)
            && bounded(self.attack_ms, 0.1, 200.0)
            && bounded(self.release_ms, 10.0, 2000.0)
    }
}
fn bounded(value: f32, min: f32, max: f32) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OttSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub low_frequency_hz: f32,
    pub high_frequency_hz: f32,
    pub bands: [OttBandSettings; 3],
    pub level_db: f32,
    pub dry_wet: f32,
    pub detection: CompressorDetection,
}
impl Default for OttSettings {
    fn default() -> Self {
        Self {
            version: OTT_STATE_VERSION,
            enabled: true,
            collapsed: false,
            low_frequency_hz: 250.0,
            high_frequency_hz: 2500.0,
            bands: [OttBandSettings::default(); 3],
            level_db: 0.0,
            dry_wet: 1.0,
            detection: CompressorDetection::Peak,
        }
    }
}
impl OttSettings {
    pub fn valid(&self) -> bool {
        self.version == OTT_STATE_VERSION
            && bounded(self.low_frequency_hz, 20.0, 2000.0)
            && bounded(self.high_frequency_hz, 200.0, 20_000.0)
            && self.low_frequency_hz < self.high_frequency_hz
            && self.bands.iter().all(OttBandSettings::valid)
            && bounded(self.level_db, SILENT_GAIN_DB, 12.0)
            && bounded(self.dry_wet, 0.0, 1.0)
    }
}
