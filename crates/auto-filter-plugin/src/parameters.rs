use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Shape = 101,
    Frequency = 102,
    Q = 103,
    Level = 104,
    DryWet = 105,
    LfoEnabled = 106,
    Wave = 107,
    RateMode = 108,
    RateHz = 109,
    NoteRate = 110,
    Strength = 111,
    SyncPhase = 112,
    StereoPhase = 113,
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
    pub const COUNT: usize = 14;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Shape,
        Self::Frequency,
        Self::Q,
        Self::Level,
        Self::DryWet,
        Self::LfoEnabled,
        Self::Wave,
        Self::RateMode,
        Self::RateHz,
        Self::NoteRate,
        Self::Strength,
        Self::SyncPhase,
        Self::StereoPhase,
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
            Self::Shape => ("shape", "Shape", 0.0, 3.0, 0.0, Scale::Choice, 3),
            Self::Frequency => (
                "frequency",
                "Frequency",
                20.0,
                20000.0,
                1000.0,
                Scale::Logarithmic,
                0,
            ),
            Self::Q => (
                "q",
                "Q",
                0.1,
                10.0,
                0.7071067690849304,
                Scale::Logarithmic,
                0,
            ),
            Self::Level => ("level", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry Wet", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::LfoEnabled => (
                "lfo_enabled",
                "Lfo Enabled",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
                1,
            ),
            Self::Wave => ("wave", "Wave", 0.0, 4.0, 0.0, Scale::Choice, 4),
            Self::RateMode => ("rate_mode", "Rate Mode", 0.0, 1.0, 1.0, Scale::Choice, 1),
            Self::RateHz => ("rate_hz", "Rate Hz", 0.01, 20.0, 2.0, Scale::Logarithmic, 0),
            Self::NoteRate => ("note_rate", "Note Rate", 0.0, 26.0, 12.0, Scale::Choice, 26),
            Self::Strength => ("strength", "Strength", 0.0, 6.0, 1.0, Scale::Linear, 0),
            Self::SyncPhase => (
                "sync_phase",
                "Sync Phase",
                -180.0,
                180.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::StereoPhase => (
                "stereo_phase",
                "Stereo Phase",
                -180.0,
                180.0,
                0.0,
                Scale::Linear,
                0,
            ),
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
            Self::Shape => "",
            Self::Frequency => "Hz",
            Self::Q => "Q",
            Self::Level => "dB",
            Self::DryWet => "%",
            Self::LfoEnabled => "",
            Self::Wave => "",
            Self::RateMode => "",
            Self::RateHz => "Hz",
            Self::NoteRate => "note",
            Self::Strength => "oct",
            Self::SyncPhase => "deg",
            Self::StereoPhase => "deg",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Shape => ["Low-pass", "High-pass", "Band-pass", "Notch"][value as usize].into(),
            Self::Frequency => format!("{value:.3} {}", self.unit()),
            Self::Q => format!("{value:.3} {}", self.unit()),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::DryWet => format!("{:.0}%", value * 100.0),
            Self::LfoEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Wave => {
                ["Sine", "Triangle", "Saw up", "Saw down", "Square"][value as usize].into()
            }
            Self::RateMode => ["Hz", "Note"][value as usize].into(),
            Self::RateHz => format!("{value:.3} {}", self.unit()),
            Self::NoteRate => daw_unit_core::NoteRate(value as u8).label(),
            Self::Strength => format!("{value:.3} {}", self.unit()),
            Self::SyncPhase => format!("{value:.3} {}", self.unit()),
            Self::StereoPhase => format!("{value:.3} {}", self.unit()),
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
            Self::Shape => ["low-pass", "high-pass", "band-pass", "notch"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::Frequency => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Q => text
                .strip_suffix("q")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Level => gain::parse_db(&text).ok_or(ParameterError)?,
            Self::DryWet => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::LfoEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::Wave => ["sine", "triangle", "saw up", "saw down", "square"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::RateMode => ["hz", "note"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::RateHz => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::NoteRate => (0..daw_unit_core::NoteRate::COUNT)
                .find(|i| daw_unit_core::NoteRate(*i).label().to_lowercase() == text)
                .map(f64::from)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::Strength => text
                .strip_suffix("oct")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::SyncPhase => text
                .strip_suffix("deg")
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
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::AUTO_FILTER_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::AUTO_FILTER_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 4] = [
    (
        "Filter",
        &[
            (Parameter::Shape, "shape", "Shape"),
            (Parameter::Frequency, "frequency", "Frequency"),
            (Parameter::Q, "scalar", "Scalar"),
        ],
    ),
    (
        "LFO",
        &[
            (Parameter::Wave, "shape", "Shape"),
            (Parameter::RateMode, "mode", "Mode"),
            (Parameter::RateHz, "frequency", "Frequency"),
            (Parameter::NoteRate, "integer", "Integer"),
        ],
    ),
    (
        "Amount",
        &[
            (Parameter::Strength, "scalar", "Scalar"),
            (Parameter::SyncPhase, "scalar", "Scalar"),
            (Parameter::StereoPhase, "scalar", "Scalar"),
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
pub(crate) fn diagram_binding(p: Parameter) -> Option<&'static str> {
    match p {
        Parameter::Shape => Some("filter_response"),
        Parameter::Wave => Some("lfo_wave"),
        _ => None,
    }
}
#[allow(dead_code)]
pub(crate) fn footer_power(index: usize) -> Option<Parameter> {
    match index {
        1 => Some(Parameter::LfoEnabled),
        _ => None,
    }
}
