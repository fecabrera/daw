use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    PreDelay = 101,
    Decay = 102,
    HighPass = 103,
    LowPass = 104,
    Drive = 105,
    Width = 106,
    Level = 107,
    DryWet = 108,
    Gate = 109,
    Color = 110,
    DriveEnabled = 111,
    Quality = 112,
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
    pub const COUNT: usize = 13;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::PreDelay,
        Self::Decay,
        Self::HighPass,
        Self::LowPass,
        Self::Drive,
        Self::Width,
        Self::Level,
        Self::DryWet,
        Self::Gate,
        Self::Color,
        Self::DriveEnabled,
        Self::Quality,
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
            Self::PreDelay => (
                "predelay_ms",
                "Pre-delay",
                0.0,
                300.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Decay => (
                "decay_seconds",
                "Decay",
                0.3,
                15.0,
                3.0,
                Scale::Logarithmic,
                0,
            ),
            Self::HighPass => (
                "highpass_hz",
                "High-pass",
                20.0,
                1000.0,
                20.0,
                Scale::Logarithmic,
                0,
            ),
            Self::LowPass => (
                "lowpass_hz",
                "Low-pass",
                500.0,
                15000.0,
                15000.0,
                Scale::Logarithmic,
                0,
            ),
            Self::Drive => ("drive_db", "Drive", -12.0, 12.0, 0.0, Scale::Linear, 0),
            Self::Width => ("width", "Width", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::Level => ("level_db", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry/Wet", 0.0, 1.0, 0.5, Scale::Linear, 0),
            Self::Gate => ("gated", "Tail", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Color => ("dark", "Color", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::DriveEnabled => ("drive_enabled", "Drive", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Quality => ("hq", "Quality", 0.0, 1.0, 0.0, Scale::Choice, 1),
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
            Self::PreDelay => "ms",
            Self::Decay => "s",
            Self::HighPass => "Hz",
            Self::LowPass => "Hz",
            Self::Drive => "dB",
            Self::Width => "%",
            Self::Level => "dB",
            Self::DryWet => "%",
            _ => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Gate => if value == 0.0 { "Normal" } else { "Gated" }.into(),
            Self::Color => if value == 0.0 { "Bright" } else { "Dark" }.into(),
            Self::DriveEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Quality => if value == 0.0 { "ECO" } else { "HQ" }.into(),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::PreDelay => format!("{:.2} ms", value),
            Self::Decay => format!("{:.2} s", value),
            Self::HighPass => format!("{:.2} Hz", value),
            Self::LowPass => format!("{:.2} Hz", value),
            Self::Drive => format!("{:.2} dB", value),
            Self::Width => format!("{:.2} %", value * 100.0),
            Self::DryWet => format!("{:.2} %", value * 100.0),
        })
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim();
        let value = match self {
            Self::Level => gain::parse_db(text).ok_or(ParameterError)?,
            Self::Bypass => {
                if text.eq_ignore_ascii_case("Off") {
                    0.0
                } else if text.eq_ignore_ascii_case("On") {
                    1.0
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::Gate => {
                if text.eq_ignore_ascii_case("Normal") {
                    0.0
                } else if text.eq_ignore_ascii_case("Gated") {
                    1.0
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::Color => {
                if text.eq_ignore_ascii_case("Bright") {
                    0.0
                } else if text.eq_ignore_ascii_case("Dark") {
                    1.0
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::DriveEnabled => {
                if text.eq_ignore_ascii_case("Off") {
                    0.0
                } else if text.eq_ignore_ascii_case("On") {
                    1.0
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::Quality => {
                if text.eq_ignore_ascii_case("ECO") {
                    0.0
                } else if text.eq_ignore_ascii_case("HQ") {
                    1.0
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            _ => {
                let unit = self.unit();
                let value = text
                    .trim_end_matches(unit)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?;
                if unit == "%" { value / 100.0 } else { value }
            }
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::PLATE_REVERB_TYPE_ID;
pub(crate) const SETTINGS_VERSION: u32 = 1;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 4] = [
    (
        "Reverb",
        &[
            (Parameter::PreDelay, "scalar", "Scalar"),
            (Parameter::Decay, "scalar", "Scalar"),
            (Parameter::Gate, "mode", "Mode"),
        ],
    ),
    (
        "Filters",
        &[
            (Parameter::HighPass, "scalar", "Scalar"),
            (Parameter::LowPass, "scalar", "Scalar"),
            (Parameter::Color, "mode", "Mode"),
        ],
    ),
    (
        "Character",
        &[
            (Parameter::Drive, "db", "Db"),
            (Parameter::DriveEnabled, "mode", "Mode"),
            (Parameter::Quality, "mode", "Mode"),
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
pub(crate) fn enable_binding(p: Parameter) -> Option<Parameter> {
    if p == Parameter::Drive {
        Some(Parameter::DriveEnabled)
    } else {
        None
    }
}
#[allow(dead_code)]
pub(crate) fn diagram_binding(_: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(_: usize) -> Option<Parameter> {
    None
}
