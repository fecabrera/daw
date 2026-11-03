use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Decay = 101,
    Bloom = 102,
    Tone = 103,
    Motion = 104,
    Width = 105,
    Level = 106,
    DryWet = 107,
    Quality = 108,
    HighPassEnabled = 109,
    HighPassSlope = 110,
    LowPass = 111,
    LowPassEnabled = 112,
    LowPassSlope = 113,
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
    pub const COUNT: usize = 14;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Decay,
        Self::Bloom,
        Self::Tone,
        Self::Motion,
        Self::Width,
        Self::Level,
        Self::DryWet,
        Self::Quality,
        Self::HighPassEnabled,
        Self::HighPassSlope,
        Self::LowPass,
        Self::LowPassEnabled,
        Self::LowPassSlope,
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
            Self::Decay => (
                "decay_seconds",
                "Decay",
                1.0,
                60.0,
                7.5,
                Scale::Logarithmic,
                0,
            ),
            Self::Bloom => ("bloom", "Bloom", 0.0, 1.0, 0.5, Scale::Linear, 0),
            Self::Tone => ("tone", "Frequency", 0.0, 1.0, 0.5, Scale::Linear, 0),
            Self::Motion => ("motion", "Modulation", 0.0, 1.0, 0.4975, Scale::Linear, 0),
            Self::Width => ("width", "Width", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::Level => ("level_db", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry/Wet", 0.0, 1.0, 0.5, Scale::Linear, 0),
            Self::Quality => ("hq", "Quality", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::LowPass => (
                "lowpass_hz",
                "Low-pass",
                500.0,
                15000.0,
                15000.0,
                Scale::Logarithmic,
                0,
            ),
            Self::LowPassEnabled => (
                "lowpass_enabled",
                "Lo-Pass",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
                1,
            ),
            Self::LowPassSlope => (
                "lowpass_slope",
                "Lo-Pass slope",
                0.0,
                2.0,
                1.0,
                Scale::Choice,
                2,
            ),
            Self::HighPassSlope => ("highpass_slope", "Slope", 0.0, 2.0, 0.0, Scale::Choice, 2),
            Self::HighPassEnabled => (
                "highpass_enabled",
                "High-pass",
                0.0,
                1.0,
                0.0,
                Scale::Choice,
                1,
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
            Self::Decay => "s",
            Self::Bloom => "%",
            Self::Tone | Self::LowPass => "Hz",
            Self::Motion => "%",
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
            Self::Quality => if value == 0.0 { "ECO" } else { "HQ" }.into(),
            Self::HighPassEnabled | Self::LowPassEnabled => {
                if value == 0.0 { "Off" } else { "On" }.into()
            }
            Self::HighPassSlope | Self::LowPassSlope => daw_unit_core::AmbientHighPassSlope::ALL
                [value as usize]
                .label()
                .into(),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::Decay => format!("{:.2} s", value),
            Self::Bloom => format!("{:.2} %", value * 100.0),
            Self::LowPass => format!("{value:.2} Hz"),
            Self::Tone => format!("{:.2} Hz", 20.0 + value * 100.0),
            Self::Motion => format!("{:.2} %", value * 100.0),
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
            Self::HighPassSlope | Self::LowPassSlope => {
                if let Some(i) = daw_unit_core::AmbientHighPassSlope::ALL
                    .iter()
                    .position(|s| s.label().eq_ignore_ascii_case(text))
                {
                    i as f64
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::Tone => {
                let hz = text
                    .trim_end_matches("Hz")
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?;
                (hz - 20.0) / 100.0
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
            Self::HighPassEnabled | Self::LowPassEnabled => {
                if text.eq_ignore_ascii_case("Off") {
                    0.0
                } else if text.eq_ignore_ascii_case("On") {
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
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::AMBIENT_REVERB_TYPE_ID;
pub(crate) const SETTINGS_VERSION: u32 = 1;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 5] = [
    ("Options", &[(Parameter::Quality, "mode", "Mode")]),
    (
        "Reverb",
        &[
            (Parameter::Decay, "scalar", "Scalar"),
            (Parameter::Bloom, "percent", "Percent"),
            (Parameter::Motion, "percent", "Percent"),
        ],
    ),
    (
        "Hi-Pass",
        &[
            (Parameter::Tone, "frequency", "Frequency"),
            (Parameter::HighPassSlope, "mode", "Mode"),
        ],
    ),
    (
        "Lo-Pass",
        &[
            (Parameter::LowPass, "frequency", "Frequency"),
            (Parameter::LowPassSlope, "mode", "Mode"),
        ],
    ),
    (
        "Out",
        &[
            (Parameter::Level, "gain", "Gain"),
            (Parameter::Width, "percent", "Percent"),
            (Parameter::DryWet, "percent", "Percent"),
        ],
    ),
];
#[allow(dead_code)]
pub(crate) fn enable_binding(p: Parameter) -> Option<Parameter> {
    match p {
        Parameter::Tone | Parameter::HighPassSlope => Some(Parameter::HighPassEnabled),
        Parameter::LowPass | Parameter::LowPassSlope => Some(Parameter::LowPassEnabled),
        _ => None,
    }
}
#[allow(dead_code)]
pub(crate) fn diagram_binding(_: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(index: usize) -> Option<Parameter> {
    match index {
        2 => Some(Parameter::HighPassEnabled),
        3 => Some(Parameter::LowPassEnabled),
        _ => None,
    }
}
