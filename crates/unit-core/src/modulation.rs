use crate::EnvelopeSettings;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeMode {
    #[default]
    PerNote,
    Retrigger,
    Legato,
}
impl EnvelopeMode {
    pub const ALL: [Self; 3] = [Self::PerNote, Self::Retrigger, Self::Legato];
    pub fn label(self) -> &'static str {
        match self {
            Self::PerNote => "Per note",
            Self::Retrigger => "Retrigger",
            Self::Legato => "Legato",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeInstance {
    pub id: Uuid,
    pub present: bool,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub mode: EnvelopeMode,
    pub settings: EnvelopeSettings,
}
impl Default for EnvelopeInstance {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            present: true,
            enabled: true,
            collapsed: false,
            mode: EnvelopeMode::PerNote,
            settings: EnvelopeSettings::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeSource {
    pub layer: Uuid,
    pub envelope: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulationParameter {
    Amplitude,
    Frequency,
    Resonance,
    Level,
    DryWet,
}
impl ModulationParameter {
    pub const FILTER: [Self; 4] = [Self::Frequency, Self::Resonance, Self::Level, Self::DryWet];
    pub fn label(self) -> &'static str {
        match self {
            Self::Amplitude => "Amplitude",
            Self::Frequency => "Frequency",
            Self::Resonance => "Resonance",
            Self::Level => "Level",
            Self::DryWet => "Dry/Wet",
        }
    }
    pub fn limit(self) -> f32 {
        match self {
            Self::Frequency => 6.0,
            Self::Resonance => 10.0,
            Self::Level => 24.0,
            _ => 1.0,
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::Frequency => "oct",
            Self::Resonance => "Q",
            Self::Level => "dB",
            _ => "",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modulation {
    pub id: Uuid,
    pub source: Option<EnvelopeSource>,
    pub parameter: ModulationParameter,
    pub amount: f32,
    pub enabled: bool,
}
impl Modulation {
    pub fn new(parameter: ModulationParameter) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: None,
            parameter,
            amount: 0.0,
            enabled: true,
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.amount.is_finite()
            && self.amount.abs() <= self.parameter.limit()
            && self
                .source
                .is_none_or(|s| !s.layer.is_nil() && !s.envelope.is_nil())
    }
}
pub fn valid_modulations(routes: &[Modulation], parameters: &[ModulationParameter]) -> bool {
    let mut ids = std::collections::HashSet::new();
    routes
        .iter()
        .all(|r| r.valid() && parameters.contains(&r.parameter) && ids.insert(r.id))
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmplitudeSettings {
    pub present: bool,
    pub enabled: bool,
    pub collapsed: bool,
    /// Saved visibility of this destination's modulation controls.
    #[serde(default)]
    pub modulation_open: bool,
    pub level_db: f32,
    pub modulations: Vec<Modulation>,
}
impl Default for AmplitudeSettings {
    fn default() -> Self {
        Self {
            present: false,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            level_db: 0.0,
            modulations: Vec::new(),
        }
    }
}
impl AmplitudeSettings {
    pub fn valid(&self) -> bool {
        self.level_db.is_finite()
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.level_db)
            && valid_modulations(&self.modulations, &[ModulationParameter::Amplitude])
    }
}
/// Prepared control values are independent of DAW entities and can fan out without advancing DSP twice.
#[derive(Clone, Copy, Debug)]
pub struct ModulationValue {
    pub source: EnvelopeSource,
    pub value: f32,
    pub weight: f32,
    /// Shared-note release age. Hosts can retain only the last released voice's amplitude tail.
    pub release_age: Option<u64>,
}
