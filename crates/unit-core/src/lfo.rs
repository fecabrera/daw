//! Saved, portable LFO component settings. Rate choices are shared with Auto-filter.
use crate::{LfoRateMode, LfoWave, ModulationRange, NoteRate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LFO_TYPE_ID: &str = "daw.lfo";
pub const LFO_STATE_VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoRestart {
    #[default]
    Transport,
    Retrigger,
    Legato,
}
impl LfoRestart {
    pub const ALL: [Self; 3] = [Self::Transport, Self::Retrigger, Self::Legato];
    pub fn label(self) -> &'static str {
        match self {
            Self::Transport => "Transport",
            Self::Retrigger => "Retrigger",
            Self::Legato => "Legato",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoPolarity {
    #[default]
    Unipolar,
    Bipolar,
}
impl LfoPolarity {
    pub fn range(self) -> ModulationRange {
        match self {
            Self::Unipolar => ModulationRange::Unipolar,
            Self::Bipolar => ModulationRange::Bipolar,
        }
    }
}
fn full_depth() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LfoSettings {
    pub version: u32,
    pub wave: LfoWave,
    pub rate_mode: LfoRateMode,
    pub rate_hz: f32,
    pub note_rate: NoteRate,
    pub phase_degrees: f32,
    pub polarity: LfoPolarity,
    pub smooth_ms: f32,
    #[serde(default = "full_depth")]
    pub depth: f32,
    pub restart: LfoRestart,
}
impl Default for LfoSettings {
    fn default() -> Self {
        Self {
            version: LFO_STATE_VERSION,
            wave: LfoWave::Sine,
            rate_mode: LfoRateMode::Note,
            rate_hz: 2.0,
            note_rate: NoteRate::default(),
            phase_degrees: 0.0,
            polarity: LfoPolarity::Unipolar,
            smooth_ms: 1.0,
            depth: 1.0,
            restart: LfoRestart::Transport,
        }
    }
}
impl LfoSettings {
    pub fn base_rate_hz(&self, tempo_bpm: f32) -> f64 {
        match self.rate_mode {
            LfoRateMode::Hz => f64::from(self.rate_hz),
            LfoRateMode::Note => f64::from(tempo_bpm) / (60.0 * self.note_rate.beats()),
        }
    }
    pub fn rate_bounds(&self, tempo_bpm: f32) -> [f64; 2] {
        match self.rate_mode {
            LfoRateMode::Hz => [0.01, 20.0],
            LfoRateMode::Note => {
                [NoteRate(1), NoteRate(26)].map(|r| f64::from(tempo_bpm) / (60.0 * r.beats()))
            }
        }
    }
    pub fn valid(&self) -> bool {
        self.version == LFO_STATE_VERSION
            && self.depth.is_finite()
            && (0.0..=1.0).contains(&self.depth)
            && self.rate_hz.is_finite()
            && (0.01..=20.0).contains(&self.rate_hz)
            && self.note_rate.0 < NoteRate::COUNT
            && self.phase_degrees.is_finite()
            && (-180.0..=180.0).contains(&self.phase_degrees)
            && self.smooth_ms.is_finite()
            && (0.0..=100.0).contains(&self.smooth_ms)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LfoInstance {
    pub id: Uuid,
    pub present: bool,
    pub enabled: bool,
    pub collapsed: bool,
    pub settings: LfoSettings,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
}
impl Default for LfoInstance {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            present: true,
            enabled: true,
            collapsed: false,
            settings: LfoSettings::default(),
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl crate::ModulationSourceSettings for LfoInstance {
    const MODULATION_OUTPUT: crate::ModulationOutputDescriptor =
        crate::ModulationOutputDescriptor {
            name: "LFO",
            range: ModulationRange::Unipolar,
            behavior: crate::ModulationBehavior::Continuous,
        };
    fn modulation_output(&self) -> crate::ModulationOutputDescriptor {
        crate::ModulationOutputDescriptor {
            range: self.settings.polarity.range(),
            ..Self::MODULATION_OUTPUT
        }
    }
    fn modulation_source_id(&self) -> Uuid {
        self.id
    }
    fn modulation_scope(&self) -> crate::ModulationScope {
        crate::ModulationScope::Shared
    }
    fn modulation_present(&self) -> bool {
        self.present
    }
    fn modulation_enabled(&self) -> bool {
        self.enabled
    }
}

impl LfoInstance {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.settings.valid()
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
impl crate::ModulationDestinationSettings for LfoInstance {
    const MODULATION_PARAMETERS: &'static [crate::ModulationParameter] =
        &crate::ModulationParameter::LFO;
    const MODULATION_SCOPE: crate::ModulationScope = crate::ModulationScope::Shared;
    fn modulations(&self) -> &[crate::Modulation] {
        &self.modulations
    }
}
