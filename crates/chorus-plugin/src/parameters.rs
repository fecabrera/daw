use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Delay = 101,
    Depth = 102,
    Feedback = 103,
    RateHz = 104,
    StereoPhase = 105,
    Width = 106,
    Level = 107,
    DryWet = 108,
    Voices = 109,
    RateMode = 110,
    NoteRate = 111,
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
        f.write_str("unknown effect parameter or invalid value")
    }
}
impl std::error::Error for ParameterError {}

impl Parameter {
    pub const COUNT: usize = 12;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Delay,
        Self::Depth,
        Self::Feedback,
        Self::RateHz,
        Self::StereoPhase,
        Self::Width,
        Self::Level,
        Self::DryWet,
        Self::Voices,
        Self::RateMode,
        Self::NoteRate,
    ];
    pub fn from_id(id: u32) -> Result<Self, ParameterError> {
        id.checked_sub(100)
            .and_then(|i| Self::ALL.get(i as usize))
            .copied()
            .ok_or(ParameterError)
    }
    #[allow(dead_code)]
    pub(crate) fn index(self) -> usize {
        self as usize - 100
    }
    pub fn info(self) -> ParameterInfo {
        let (key, name, minimum, maximum, default, scale, steps) = match self {
            Self::Bypass => ("bypass", "Bypass", 0.0, 1.0, 0.0, Scale::Boolean, 1),
            Self::Delay => ("delay", "Delay", 5.0, 30.0, 15.0, Scale::Linear, 0),
            Self::Depth => ("depth", "Depth", 0.0, 1.0, 0.25, Scale::Linear, 0),
            Self::Feedback => ("feedback", "Feedback", -0.8, 0.8, 0.0, Scale::Linear, 0),
            Self::RateHz => ("rate_hz", "Rate Hz", 0.05, 10.0, 0.8, Scale::Logarithmic, 0),
            Self::StereoPhase => (
                "stereo_phase",
                "Stereo Phase",
                -180.0,
                180.0,
                90.0,
                Scale::Linear,
                0,
            ),
            Self::Width => ("width", "Width", 0.0, 2.0, 1.0, Scale::Linear, 0),
            Self::Level => ("level", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry Wet", 0.0, 1.0, 0.35, Scale::Linear, 0),
            Self::Voices => ("voices", "Voices", 1.0, 4.0, 2.0, Scale::Choice, 3),
            Self::RateMode => ("rate_mode", "Rate Mode", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::NoteRate => ("note_rate", "Note Rate", 0.0, 26.0, 12.0, Scale::Choice, 26),
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
            Self::Bypass => "",
            Self::Delay => "ms",
            Self::Depth => "%",
            Self::Feedback => "%",
            Self::RateHz => "Hz",
            Self::StereoPhase => "deg",
            Self::Width => "%",
            Self::Level => "dB",
            Self::DryWet => "%",
            Self::Voices => "",
            Self::RateMode => "",
            Self::NoteRate => "note",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Delay => format!("{value:.3} {}", self.unit()),
            Self::Depth => format!("{:.0}%", value * 100.0),
            Self::Feedback => format!("{:.0}%", value * 100.0),
            Self::RateHz => format!("{value:.3} {}", self.unit()),
            Self::StereoPhase => format!("{value:.3} {}", self.unit()),
            Self::Width => format!("{:.0}%", value * 100.0),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::DryWet => format!("{:.0}%", value * 100.0),
            Self::Voices => format!("{value:.3} {}", self.unit()),
            Self::RateMode => ["Hz", "Note"][value as usize].into(),
            Self::NoteRate => daw_unit_core::NoteRate(value as u8).label(),
        })
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim().to_lowercase();
        let value = match self {
            Self::Bypass => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::Delay => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Depth => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::Feedback => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::RateHz => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::StereoPhase => text
                .strip_suffix("deg")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Width => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::Level => gain::parse_db(&text).ok_or(ParameterError)?,
            Self::DryWet => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::Voices => text
                .strip_suffix("")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::RateMode => ["hz", "note"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::NoteRate => (0..daw_unit_core::NoteRate::COUNT)
                .find(|i| daw_unit_core::NoteRate(*i).label().to_lowercase() == text)
                .map(f64::from)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::CHORUS_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::CHORUS_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 3] = [
    (
        "Chorus",
        &[
            (Parameter::Delay, "scalar", "Scalar"),
            (Parameter::Depth, "percent", "Percent"),
            (Parameter::Feedback, "percent", "Percent"),
        ],
    ),
    (
        "Voices",
        &[
            (Parameter::RateMode, "mode", "Mode"),
            (Parameter::RateHz, "frequency", "Frequency"),
            (Parameter::NoteRate, "integer", "Integer"),
            (Parameter::StereoPhase, "scalar", "Scalar"),
            (Parameter::Voices, "integer", "Integer"),
        ],
    ),
    (
        "Out",
        &[
            (Parameter::Width, "percent", "Percent"),
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
pub(crate) fn diagram_binding(_p: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(_index: usize) -> Option<Parameter> {
    None
}
