mod graph;
mod protocol;
mod runtime;
pub use graph::*;
pub use protocol::*;
pub use runtime::*;

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
/// Stable provider identity; the historical JSON key is retained for existing projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulationSourceId {
    pub layer: Uuid,
    #[serde(rename = "envelope", alias = "unit")]
    pub unit: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulationParameter {
    Amplitude,
    Pitch,
    Frequency,
    Resonance,
    Level,
    DryWet,
    Volume,
    DriveGain,
    Tone,
    ColorStrength,
    ColorFrequency,
    LfoDepth,
    LfoRate,
}
impl ModulationParameter {
    pub const LFO: [Self; 2] = [Self::LfoDepth, Self::LfoRate];
    pub const DRIVE: [Self; 5] = [
        Self::Volume,
        Self::DriveGain,
        Self::Tone,
        Self::ColorStrength,
        Self::ColorFrequency,
    ];
    pub const FILTER: [Self; 4] = [Self::Frequency, Self::Resonance, Self::Level, Self::DryWet];
    pub fn label(self) -> &'static str {
        match self {
            Self::LfoDepth => "Depth",
            Self::LfoRate => "Rate",
            Self::Amplitude => "Amplitude",
            Self::Pitch => "Pitch",
            Self::Frequency => "Frequency",
            Self::Resonance => "Resonance",
            Self::Level => "Level",
            Self::DryWet => "Dry/Wet",
            Self::Volume => "Volume",
            Self::DriveGain => "Gain",
            Self::Tone => "Tone",
            Self::ColorStrength => "Color strength",
            Self::ColorFrequency => "Color frequency",
        }
    }
    pub fn limit(self) -> f32 {
        match self {
            Self::Pitch => 48.0,
            Self::Frequency | Self::LfoRate => 6.0,
            Self::ColorFrequency => 3.0,
            Self::Resonance => 10.0,
            Self::Level | Self::Volume => 24.0,
            _ => 1.0,
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::Pitch => "semitones",
            Self::Frequency | Self::ColorFrequency | Self::LfoRate => "oct",
            Self::Resonance => "Q",
            Self::Level | Self::Volume => "dB",
            _ => "",
        }
    }
}
/// Optional instance identity for destinations that own repeated units.
pub type ModulationTargetId = Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modulation {
    pub id: Uuid,
    pub source: Option<ModulationSourceId>,
    pub parameter: ModulationParameter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ModulationTargetId>,
    pub amount: f32,
    pub enabled: bool,
}
impl Modulation {
    pub fn new(parameter: ModulationParameter) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: None,
            parameter,
            target: None,
            amount: 0.0,
            enabled: true,
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.target.is_none_or(|id| !id.is_nil())
            && self.amount.is_finite()
            && self.amount.abs() <= self.parameter.limit()
            && self
                .source
                .is_none_or(|s| !s.layer.is_nil() && !s.unit.is_nil())
    }
}
pub fn valid_modulations(routes: &[Modulation], parameters: &[ModulationParameter]) -> bool {
    let mut ids = std::collections::HashSet::new();
    routes.iter().all(|r| {
        r.valid() && r.target.is_none() && parameters.contains(&r.parameter) && ids.insert(r.id)
    })
}
/// Validate repeated destinations without interpreting instance IDs as list positions.
pub fn valid_targeted_modulations(
    routes: &[Modulation],
    parameters: &[ModulationParameter],
    targets: &[ModulationTargetId],
) -> bool {
    let mut ids = std::collections::HashSet::new();
    routes.iter().all(|r| {
        r.valid()
            && parameters.contains(&r.parameter)
            && r.target.is_some_and(|id| targets.contains(&id))
            && ids.insert(r.id)
    })
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
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
/// Prepared control values are independent of DAW entities and can fan out without advancing DSP twice.
#[derive(Clone, Copy, Debug)]
pub struct ModulationValue {
    pub range: ModulationRange,
    pub behavior: ModulationBehavior,
    pub source: ModulationSourceId,
    pub value: f32,
    pub weight: f32,
    /// Shared-note release age. Hosts can retain only the last released voice's amplitude tail.
    pub release_age: Option<u64>,
}

#[cfg(test)]
mod tests;

impl ModulationValue {
    /// Only a note contour can carry the final released voice. Continuous sources never do.
    pub fn owns_release(&self, age: Option<u64>) -> bool {
        self.behavior == ModulationBehavior::Contour
            && self.release_age.is_some()
            && self.release_age == age
            && self.value > 0.0
            && self.weight > 0.0
    }
}
