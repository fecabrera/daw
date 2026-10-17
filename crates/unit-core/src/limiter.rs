use serde::{Deserialize, Serialize};

pub const LIMITER_TYPE_ID: &str = "daw.limiter";
pub const LIMITER_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimiterMode {
    #[default]
    Peak,
    TruePeak,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimiterRouting {
    #[default]
    LeftRight,
    MidSide,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LimiterSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub input_db: f32,
    pub release_ms: f32,
    pub lookahead_ms: f32,
    pub ceiling_db: f32,
    pub mode: LimiterMode,
    pub routing: LimiterRouting,
}
impl Default for LimiterSettings {
    fn default() -> Self {
        Self {
            version: LIMITER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            input_db: 0.0,
            release_ms: 100.0,
            lookahead_ms: 1.0,
            ceiling_db: -1.0,
            mode: LimiterMode::Peak,
            routing: LimiterRouting::LeftRight,
        }
    }
}
impl LimiterSettings {
    pub fn valid(&self) -> bool {
        fn bounded(value: f32, min: f32, max: f32) -> bool {
            value.is_finite() && (min..=max).contains(&value)
        }
        self.version == LIMITER_STATE_VERSION
            && bounded(self.input_db, -24.0, 24.0)
            && bounded(self.release_ms, 1.0, 2000.0)
            && bounded(self.lookahead_ms, 0.0, 10.0)
            && bounded(self.ceiling_db, -24.0, 0.0)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
