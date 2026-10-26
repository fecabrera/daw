use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    LowGain = 101,
    LowFrequency = 102,
    LowQ = 103,
    LowEnabled = 104,
    MidGain = 105,
    MidEnabled = 106,
    HighGain = 107,
    HighFrequency = 108,
    HighQ = 109,
    HighEnabled = 110,
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
    pub const COUNT: usize = 11;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::LowGain,
        Self::LowFrequency,
        Self::LowQ,
        Self::LowEnabled,
        Self::MidGain,
        Self::MidEnabled,
        Self::HighGain,
        Self::HighFrequency,
        Self::HighQ,
        Self::HighEnabled,
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
            Self::LowGain => ("low_gain", "Low Gain", -24.0, 24.0, 0.0, Scale::Linear, 0),
            Self::LowFrequency => (
                "low_frequency",
                "Low Frequency",
                20.0,
                20000.0,
                250.0,
                Scale::Logarithmic,
                0,
            ),
            Self::LowQ => (
                "low_q",
                "Low Q",
                0.1,
                10.0,
                0.7071067690849304,
                Scale::Logarithmic,
                0,
            ),
            Self::LowEnabled => (
                "low_enabled",
                "Low Enabled",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
                1,
            ),
            Self::MidGain => ("mid_gain", "Mid Gain", -24.0, 24.0, 0.0, Scale::Linear, 0),
            Self::MidEnabled => (
                "mid_enabled",
                "Mid Enabled",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
                1,
            ),
            Self::HighGain => ("high_gain", "High Gain", -24.0, 24.0, 0.0, Scale::Linear, 0),
            Self::HighFrequency => (
                "high_frequency",
                "High Frequency",
                20.0,
                20000.0,
                2500.0,
                Scale::Logarithmic,
                0,
            ),
            Self::HighQ => (
                "high_q",
                "High Q",
                0.1,
                10.0,
                0.7071067690849304,
                Scale::Logarithmic,
                0,
            ),
            Self::HighEnabled => (
                "high_enabled",
                "High Enabled",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
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
            Self::Bypass => "",
            Self::LowGain => "dB",
            Self::LowFrequency => "Hz",
            Self::LowQ => "Q",
            Self::LowEnabled => "",
            Self::MidGain => "dB",
            Self::MidEnabled => "",
            Self::HighGain => "dB",
            Self::HighFrequency => "Hz",
            Self::HighQ => "Q",
            Self::HighEnabled => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::LowGain => format!("{value:.3} {}", self.unit()),
            Self::LowFrequency => format!("{value:.3} {}", self.unit()),
            Self::LowQ => format!("{value:.3} {}", self.unit()),
            Self::LowEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::MidGain => format!("{value:.3} {}", self.unit()),
            Self::MidEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::HighGain => format!("{value:.3} {}", self.unit()),
            Self::HighFrequency => format!("{value:.3} {}", self.unit()),
            Self::HighQ => format!("{value:.3} {}", self.unit()),
            Self::HighEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
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
            Self::LowGain => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowFrequency => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowQ => text
                .strip_suffix("q")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::MidGain => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::MidEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::HighGain => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighFrequency => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighQ => text
                .strip_suffix("q")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::EQ_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::EQ_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 3] = [
    (
        "Low",
        &[
            (Parameter::LowGain, "db", "Db"),
            (Parameter::LowFrequency, "frequency", "Frequency"),
            (Parameter::LowQ, "scalar", "Scalar"),
        ],
    ),
    ("Mid", &[(Parameter::MidGain, "db", "Db")]),
    (
        "High",
        &[
            (Parameter::HighGain, "db", "Db"),
            (Parameter::HighFrequency, "frequency", "Frequency"),
            (Parameter::HighQ, "scalar", "Scalar"),
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
pub(crate) fn footer_power(index: usize) -> Option<Parameter> {
    match index {
        0 => Some(Parameter::LowEnabled),
        1 => Some(Parameter::MidEnabled),
        2 => Some(Parameter::HighEnabled),
        _ => None,
    }
}
