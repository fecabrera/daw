use crate::{FilterShape, Modulation, ModulationDestinationSettings};
use serde::{Deserialize, Serialize};

pub const FILTER_TYPE_ID: &str = "daw.filter";
pub const FILTER_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    pub shape: FilterShape,
    pub frequency_hz: f32,
    pub q: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
}
impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            version: FILTER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            shape: FilterShape::LowPass,
            frequency_hz: 1000.0,
            q: std::f32::consts::FRAC_1_SQRT_2,
            level_db: 0.0,
            dry_wet: 1.0,
            modulations: Vec::new(),
        }
    }
}
impl FilterSettings {
    pub fn valid(&self) -> bool {
        fn bounded(value: f32, min: f32, max: f32) -> bool {
            value.is_finite() && (min..=max).contains(&value)
        }
        self.version == FILTER_STATE_VERSION
            && bounded(self.frequency_hz, 20.0, 20_000.0)
            && bounded(self.q, 0.1, 10.0)
            && bounded(self.level_db, crate::SILENT_GAIN_DB, 12.0)
            && bounded(self.dry_wet, 0.0, 1.0)
            && self.valid_modulations()
    }
}
