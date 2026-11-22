use serde::{Deserialize, Serialize};

pub const TRANSIENT_SHAPER_TYPE_ID: &str = "daw.transient-shaper";
pub const TRANSIENT_SHAPER_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransientShaperClipMode {
    #[default]
    None,
    Soft,
    Hard,
    Tube,
}

impl TransientShaperClipMode {
    pub const ALL: [Self; 4] = [Self::None, Self::Soft, Self::Hard, Self::Tube];

    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Soft => "Soft",
            Self::Hard => "Hard",
            Self::Tube => "Tube",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransientShaperSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub attack_db: f32,
    pub pump_db: f32,
    pub sustain_db: f32,
    pub speed: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub clip_mode: TransientShaperClipMode,
}

impl Default for TransientShaperSettings {
    fn default() -> Self {
        Self {
            version: TRANSIENT_SHAPER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            attack_db: 0.0,
            pump_db: 0.0,
            sustain_db: 0.0,
            speed: 0.5,
            level_db: 0.0,
            dry_wet: 1.0,
            clip_mode: TransientShaperClipMode::None,
        }
    }
}

impl TransientShaperSettings {
    /// Continuous control values in modulation destination order.
    pub fn values(&self) -> [f32; 6] {
        [
            self.attack_db,
            self.pump_db,
            self.sustain_db,
            self.speed,
            self.level_db,
            self.dry_wet,
        ]
    }

    pub fn valid(&self) -> bool {
        self.version == TRANSIENT_SHAPER_STATE_VERSION
            && (-12.0..=12.0).contains(&self.attack_db)
            && (0.0..=12.0).contains(&self.pump_db)
            && (-12.0..=12.0).contains(&self.sustain_db)
            && (0.0..=1.0).contains(&self.speed)
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.level_db)
            && (0.0..=1.0).contains(&self.dry_wet)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
