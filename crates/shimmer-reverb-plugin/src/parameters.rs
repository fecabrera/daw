use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Size = 101,
    PreDelay = 102,
    Sync = 103,
    Note = 104,
    Vowel = 105,
    VowelAmount = 106,
    Pitch1 = 107,
    Amount1 = 108,
    Pitch2 = 109,
    Amount2 = 110,
    Rate = 111,
    Depth = 112,
    LowDamp = 113,
    HighDamp = 114,
    Warm = 115,
    Level = 116,
    DryWet = 117,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Boolean,
    Gain,
    Linear,
    Choice,
    Logarithmic,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterInfo {
    pub parameter: Parameter,
    pub key: &'static str,
    pub name: &'static str,
    pub minimum: f64,
    pub maximum: f64,
    pub default: f64,
    pub scale: Scale,
    pub steps: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParameterError;
impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid reverb parameter")
    }
}
impl std::error::Error for ParameterError {}
impl Parameter {
    pub const COUNT: usize = 18;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Size,
        Self::PreDelay,
        Self::Sync,
        Self::Note,
        Self::Vowel,
        Self::VowelAmount,
        Self::Pitch1,
        Self::Amount1,
        Self::Pitch2,
        Self::Amount2,
        Self::Rate,
        Self::Depth,
        Self::LowDamp,
        Self::HighDamp,
        Self::Warm,
        Self::Level,
        Self::DryWet,
    ];
    pub fn from_id(id: u32) -> Result<Self, ParameterError> {
        Self::ALL
            .into_iter()
            .find(|p| *p as u32 == id)
            .ok_or(ParameterError)
    }
    pub(crate) fn index(self) -> usize {
        self as usize - 100
    }
    pub fn info(self) -> ParameterInfo {
        let (key, name, minimum, maximum, default, scale, steps) = match self {
            Self::Bypass => ("bypass", "Bypass", 0.0, 1.0, 0.0, Scale::Boolean, 1),
            Self::Size => ("size", "Size", 0.0, 1.0, 0.75, Scale::Linear, 0),
            Self::PreDelay => (
                "predelay_ms",
                "Pre-delay",
                0.0,
                1000.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Sync => ("sync", "Mode", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Note => ("note", "Pre-delay note", 0.0, 15.0, 6.0, Scale::Choice, 15),
            Self::Vowel => ("vowel", "Position", 0.0, 1.0, 0.0, Scale::Linear, 0),
            Self::VowelAmount => (
                "vowel_amount",
                "Vowel Amount",
                0.0,
                1.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Pitch1 => ("pitch_1", "Pitch 1", 0.0, 30.0, 15.0, Scale::Choice, 30),
            Self::Amount1 => (
                "amount_1",
                "Shimmer 1 Amount",
                0.0,
                1.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Pitch2 => ("pitch_2", "Pitch 2", 0.0, 30.0, 15.0, Scale::Choice, 30),
            Self::Amount2 => (
                "amount_2",
                "Shimmer 2 Amount",
                0.0,
                1.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Rate => ("rate", "Rate", 0.0, 1.0, 0.390625, Scale::Linear, 0),
            Self::Depth => ("depth", "Depth", 0.0, 1.0, 0.429688, Scale::Linear, 0),
            Self::LowDamp => ("low_damp", "Low Damp", 0.0, 1.0, 0.0, Scale::Linear, 0),
            Self::HighDamp => ("high_damp", "High Damp", 0.0, 1.0, 0.0, Scale::Linear, 0),
            Self::Warm => ("warm", "Warm", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Level => ("level_db", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry/Wet", 0.0, 1.0, 0.5, Scale::Linear, 0),
        };
        ParameterInfo {
            parameter: self,
            key,
            name,
            minimum,
            maximum,
            default,
            scale,
            steps,
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::Level => "dB",
            Self::PreDelay => "ms",
            Self::Pitch1 | Self::Pitch2 => "st",
            Self::Size
            | Self::Vowel
            | Self::VowelAmount
            | Self::Amount1
            | Self::Amount2
            | Self::Rate
            | Self::Depth
            | Self::LowDamp
            | Self::HighDamp
            | Self::DryWet => "%",
            _ => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => {
                if value == 0.0 {
                    "Off".into()
                } else {
                    "On".into()
                }
            }
            Self::Sync => {
                if value == 0.0 {
                    "Time".into()
                } else {
                    "Sync".into()
                }
            }
            Self::Warm => {
                if value == 0.0 {
                    "Off".into()
                } else {
                    "On".into()
                }
            }
            Self::Note => daw_unit_core::ShimmerReverbSettings::NOTE_LABELS[value as usize].into(),
            Self::Pitch1 | Self::Pitch2 => {
                format!("{:+} st", daw_unit_core::shimmer_pitch(value as i8 - 15))
            }
            Self::Level => format!("{} dB", gain::format_db(value)),
            _ => format!(
                "{} {}",
                if self.unit() == "%" {
                    value * 100.0
                } else {
                    value
                },
                self.unit()
            )
            .trim()
            .into(),
        })
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim();
        let value = match self {
            Self::Level => gain::parse_db(text).ok_or(ParameterError)?,
            Self::Bypass | Self::Warm => match text.to_ascii_lowercase().as_str() {
                "off" => 0.0,
                "on" => 1.0,
                _ => text.parse().map_err(|_| ParameterError)?,
            },
            Self::Sync => match text.to_ascii_lowercase().as_str() {
                "time" => 0.0,
                "sync" => 1.0,
                _ => text.parse().map_err(|_| ParameterError)?,
            },
            Self::Note => daw_unit_core::ShimmerReverbSettings::NOTE_LABELS
                .iter()
                .position(|s| s.eq_ignore_ascii_case(text))
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::Pitch1 | Self::Pitch2 => {
                let pitch = text
                    .trim_end_matches("st")
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| ParameterError)?;
                (-15..=15)
                    .find(|i| daw_unit_core::shimmer_pitch(*i) == pitch)
                    .map(|i| f64::from(i + 15))
                    .ok_or(ParameterError)?
            }
            _ => {
                let v = text
                    .trim_end_matches(self.unit())
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?;
                if self.unit() == "%" { v / 100.0 } else { v }
            }
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::SHIMMER_REVERB_TYPE_ID;
pub(crate) const SETTINGS_VERSION: u32 = 1;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 6] = [
    (
        "Reverb",
        &[
            (Parameter::Size, "percent", "Percent"),
            (Parameter::PreDelay, "scalar", "Scalar"),
            (Parameter::Sync, "mode", "Mode"),
            (Parameter::Note, "selector", "Selector"),
        ],
    ),
    (
        "Vowel",
        &[
            (Parameter::Vowel, "percent", "Percent"),
            (Parameter::VowelAmount, "percent", "Percent"),
        ],
    ),
    (
        "Shimmer",
        &[
            (Parameter::Pitch1, "scalar", "Scalar"),
            (Parameter::Amount1, "percent", "Percent"),
            (Parameter::Pitch2, "scalar", "Scalar"),
            (Parameter::Amount2, "percent", "Percent"),
        ],
    ),
    (
        "Motion",
        &[
            (Parameter::Rate, "percent", "Percent"),
            (Parameter::Depth, "percent", "Percent"),
        ],
    ),
    (
        "Damping",
        &[
            (Parameter::LowDamp, "percent", "Percent"),
            (Parameter::HighDamp, "percent", "Percent"),
            (Parameter::Warm, "mode", "Mode"),
        ],
    ),
    (
        "Out",
        &[
            (Parameter::Level, "gain", "Gain"),
            (Parameter::DryWet, "percent", "Percent"),
        ],
    ),
];
#[allow(dead_code)]
pub(crate) fn enable_binding(_: Parameter) -> Option<Parameter> {
    None
}
#[allow(dead_code)]
pub(crate) fn diagram_binding(_: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(_: usize) -> Option<Parameter> {
    None
}
