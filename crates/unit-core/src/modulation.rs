mod index;
pub use index::ModulationSourceIndex;
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
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
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
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl EnvelopeInstance {
    pub fn valid(&self) -> bool {
        self.settings.valid() && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
/// Stable provider identity; the historical JSON key is retained for existing projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ModulationSourceId {
    pub layer: Uuid,
    #[serde(rename = "envelope", alias = "unit")]
    pub unit: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulationParameter {
    EnvelopeAttack,
    EnvelopeDecay,
    EnvelopeSustain,
    EnvelopeRelease,
    EnvelopeAttackCurve,
    EnvelopeDecayCurve,
    EnvelopeReleaseCurve,
    EnvelopeAttackNote,
    EnvelopeDecayNote,
    OttLowFrequency,
    OttHighFrequency,
    OttLowDepth,
    OttLowInput,
    OttLowOutput,
    OttLowAttack,
    OttLowRelease,
    OttMidDepth,
    OttMidInput,
    OttMidOutput,
    OttMidAttack,
    OttMidRelease,
    OttHighDepth,
    OttHighInput,
    OttHighOutput,
    OttHighAttack,
    OttHighRelease,

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
    MacroValue,
    EqLowGain,
    EqLowFrequency,
    EqLowQ,
    EqMidGain,
    EqHighGain,
    EqHighFrequency,
    EqHighQ,
    DirectionMixerDirection,
    DirectionMixerSpread,
    SaturatorDrive,
    BitcrusherDrive,
    BitcrusherResolution,
    BitcrusherDownsampling,
    StereoSpreadLowerIntensity,
    StereoSpreadUpperIntensity,
    StereoSpreadLowerFrequency,
    StereoSpreadUpperFrequency,
    LimiterInput,
    LimiterRelease,
    LimiterLookahead,
    LimiterCeiling,
    SamplerGain,
    OscillatorGain,
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    CompressorKnee,
}
impl ModulationParameter {
    pub const SATURATOR: [Self; 3] = [Self::SaturatorDrive, Self::Level, Self::DryWet];
    pub const OSCILLATOR: [Self; 2] = [Self::Pitch, Self::OscillatorGain];
    pub const ENVELOPE: [Self; 9] = [
        Self::EnvelopeAttack,
        Self::EnvelopeDecay,
        Self::EnvelopeSustain,
        Self::EnvelopeRelease,
        Self::EnvelopeAttackCurve,
        Self::EnvelopeDecayCurve,
        Self::EnvelopeReleaseCurve,
        Self::EnvelopeAttackNote,
        Self::EnvelopeDecayNote,
    ];
    pub const BITCRUSHER: [Self; 5] = [
        Self::BitcrusherDrive,
        Self::BitcrusherResolution,
        Self::BitcrusherDownsampling,
        Self::Level,
        Self::DryWet,
    ];
    pub const STEREO_SPREAD: [Self; 4] = [
        Self::StereoSpreadLowerIntensity,
        Self::StereoSpreadUpperIntensity,
        Self::StereoSpreadLowerFrequency,
        Self::StereoSpreadUpperFrequency,
    ];
    pub const DIRECTION_MIXER: [Self; 2] =
        [Self::DirectionMixerDirection, Self::DirectionMixerSpread];
    /// Fixed OTT identities: crossovers, Low/Mid/High band controls, Level, Dry/Wet.
    pub const OTT: [Self; 19] = [
        Self::OttLowFrequency,
        Self::OttHighFrequency,
        Self::OttLowDepth,
        Self::OttLowInput,
        Self::OttLowOutput,
        Self::OttLowAttack,
        Self::OttLowRelease,
        Self::OttMidDepth,
        Self::OttMidInput,
        Self::OttMidOutput,
        Self::OttMidAttack,
        Self::OttMidRelease,
        Self::OttHighDepth,
        Self::OttHighInput,
        Self::OttHighOutput,
        Self::OttHighAttack,
        Self::OttHighRelease,
        Self::Level,
        Self::DryWet,
    ];
    /// Continuous Limiter controls in UI/telemetry order; mode and routing remain discrete.
    pub const LIMITER: [Self; 4] = [
        Self::LimiterInput,
        Self::LimiterRelease,
        Self::LimiterLookahead,
        Self::LimiterCeiling,
    ];
    /// Continuous Compressor controls, in UI/telemetry order. Detection remains discrete.
    pub const COMPRESSOR: [Self; 7] = [
        Self::CompressorThreshold,
        Self::CompressorRatio,
        Self::CompressorAttack,
        Self::CompressorRelease,
        Self::CompressorKnee,
        Self::Level,
        Self::DryWet,
    ];
    /// Fixed band controls, in UI/telemetry order. Mid uses the outer boundaries and Q.
    pub const EQUALIZER: [Self; 7] = [
        Self::EqLowGain,
        Self::EqLowFrequency,
        Self::EqLowQ,
        Self::EqMidGain,
        Self::EqHighGain,
        Self::EqHighFrequency,
        Self::EqHighQ,
    ];
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
            Self::EnvelopeAttack => "Attack time",
            Self::EnvelopeDecay => "Decay time",
            Self::EnvelopeSustain => "Sustain",
            Self::EnvelopeRelease => "Release time",
            Self::EnvelopeAttackCurve => "Attack curve",
            Self::EnvelopeDecayCurve => "Decay curve",
            Self::EnvelopeReleaseCurve => "Release curve",
            Self::EnvelopeAttackNote => "Attack · Note",
            Self::EnvelopeDecayNote => "Decay · Note",
            Self::OttLowFrequency => "Low · Frequency",
            Self::OttHighFrequency => "High · Frequency",
            Self::OttLowDepth => "Low · Depth",
            Self::OttLowInput => "Low · Input",
            Self::OttLowOutput => "Low · Output",
            Self::OttLowAttack => "Low · Attack",
            Self::OttLowRelease => "Low · Release",
            Self::OttMidDepth => "Mid · Depth",
            Self::OttMidInput => "Mid · Input",
            Self::OttMidOutput => "Mid · Output",
            Self::OttMidAttack => "Mid · Attack",
            Self::OttMidRelease => "Mid · Release",
            Self::OttHighDepth => "High · Depth",
            Self::OttHighInput => "High · Input",
            Self::OttHighOutput => "High · Output",
            Self::OttHighAttack => "High · Attack",
            Self::OttHighRelease => "High · Release",
            Self::DirectionMixerDirection => "Direction",
            Self::DirectionMixerSpread => "Spread",
            Self::SaturatorDrive | Self::BitcrusherDrive => "Drive",
            Self::BitcrusherResolution => "Resolution",
            Self::BitcrusherDownsampling => "Downsampling",
            Self::StereoSpreadLowerIntensity => "Lower · Intensity",
            Self::StereoSpreadUpperIntensity => "Upper · Intensity",
            Self::StereoSpreadLowerFrequency => "Lower · Frequency",
            Self::StereoSpreadUpperFrequency => "Upper · Frequency",
            Self::LimiterInput => "Input",
            Self::LimiterRelease => "Release",
            Self::LimiterLookahead => "Lookahead",
            Self::LimiterCeiling => "Ceiling",
            Self::SamplerGain | Self::OscillatorGain => "Gain",
            Self::CompressorThreshold => "Threshold",
            Self::CompressorRatio => "Ratio",
            Self::CompressorAttack => "Attack",
            Self::CompressorRelease => "Release",
            Self::CompressorKnee => "Knee",
            Self::EqLowGain => "Low · Gain",
            Self::EqLowFrequency => "Low · Frequency",
            Self::EqLowQ => "Low · Q",
            Self::EqMidGain => "Mid · Gain",
            Self::EqHighGain => "High · Gain",
            Self::EqHighFrequency => "High · Frequency",
            Self::EqHighQ => "High · Q",
            Self::MacroValue => "Value",
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
            Self::EnvelopeAttack | Self::EnvelopeDecay | Self::EnvelopeRelease => 6.0,
            Self::OttLowFrequency
            | Self::OttHighFrequency
            | Self::OttLowAttack
            | Self::OttLowRelease
            | Self::OttMidAttack
            | Self::OttMidRelease
            | Self::OttHighAttack
            | Self::OttHighRelease => 6.0,
            Self::OttLowDepth | Self::OttMidDepth | Self::OttHighDepth => 1.0,
            Self::OttLowInput
            | Self::OttLowOutput
            | Self::OttMidInput
            | Self::OttMidOutput
            | Self::OttHighInput
            | Self::OttHighOutput => 24.0,

            Self::LimiterInput | Self::LimiterCeiling => 24.0,
            Self::LimiterRelease => 6.0,
            Self::LimiterLookahead => 10.0,
            Self::SaturatorDrive => 36.0,
            Self::BitcrusherDrive => 24.0,
            Self::BitcrusherResolution => 23.0,
            Self::BitcrusherDownsampling => 39.0,
            Self::DirectionMixerDirection => 180.0,
            Self::DirectionMixerSpread => 2.0,
            Self::StereoSpreadLowerFrequency | Self::StereoSpreadUpperFrequency => 6.0,
            Self::CompressorThreshold => 60.0,
            Self::CompressorRatio => 19.0,
            Self::CompressorKnee => 24.0,
            Self::CompressorAttack | Self::CompressorRelease => 6.0,
            Self::Pitch => 48.0,
            Self::Frequency | Self::LfoRate | Self::EqLowFrequency | Self::EqHighFrequency => 6.0,
            Self::ColorFrequency => 3.0,
            Self::Resonance | Self::EqLowQ | Self::EqHighQ => 10.0,
            Self::OscillatorGain
            | Self::SamplerGain
            | Self::Level
            | Self::Volume
            | Self::EqLowGain
            | Self::EqMidGain
            | Self::EqHighGain => 24.0,
            _ => 1.0,
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::EnvelopeAttack | Self::EnvelopeDecay | Self::EnvelopeRelease => "oct",
            Self::OttLowFrequency
            | Self::OttHighFrequency
            | Self::OttLowAttack
            | Self::OttLowRelease
            | Self::OttMidAttack
            | Self::OttMidRelease
            | Self::OttHighAttack
            | Self::OttHighRelease => "oct",
            Self::OttLowDepth | Self::OttMidDepth | Self::OttHighDepth => "",
            Self::OttLowInput
            | Self::OttLowOutput
            | Self::OttMidInput
            | Self::OttMidOutput
            | Self::OttHighInput
            | Self::OttHighOutput => "dB",
            Self::LimiterInput | Self::LimiterCeiling => "dB",
            Self::LimiterRelease => "oct",
            Self::LimiterLookahead => "ms",
            Self::SaturatorDrive | Self::BitcrusherDrive => "dB",
            Self::BitcrusherResolution => "bits",
            Self::BitcrusherDownsampling => "×",
            Self::DirectionMixerDirection => "°",
            Self::DirectionMixerSpread => "",
            Self::StereoSpreadLowerFrequency | Self::StereoSpreadUpperFrequency => "oct",
            Self::CompressorThreshold | Self::CompressorKnee => "dB",
            Self::CompressorRatio => ":1",
            Self::CompressorAttack | Self::CompressorRelease => "oct",
            Self::Pitch => "semitones",
            Self::Frequency
            | Self::ColorFrequency
            | Self::LfoRate
            | Self::EqLowFrequency
            | Self::EqHighFrequency => "oct",
            Self::Resonance | Self::EqLowQ | Self::EqHighQ => "Q",
            Self::OscillatorGain
            | Self::SamplerGain
            | Self::Level
            | Self::Volume
            | Self::EqLowGain
            | Self::EqMidGain
            | Self::EqHighGain => "dB",
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
