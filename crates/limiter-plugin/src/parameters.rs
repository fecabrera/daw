use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Input = 101,
    Release = 102,
    Lookahead = 103,
    Ceiling = 104,
    Mode = 105,
    Routing = 106,
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
    pub const COUNT: usize = 7;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Input,
        Self::Release,
        Self::Lookahead,
        Self::Ceiling,
        Self::Mode,
        Self::Routing,
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
            Self::Input => ("input", "Input", -24.0, 24.0, 0.0, Scale::Linear, 0),
            Self::Release => (
                "release",
                "Release",
                1.0,
                2000.0,
                100.0,
                Scale::Logarithmic,
                0,
            ),
            Self::Lookahead => ("lookahead", "Lookahead", 0.0, 10.0, 1.0, Scale::Linear, 0),
            Self::Ceiling => ("ceiling", "Ceiling", -24.0, 0.0, -1.0, Scale::Linear, 0),
            Self::Mode => ("mode", "Mode", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Routing => ("routing", "Routing", 0.0, 1.0, 0.0, Scale::Choice, 1),
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
            Self::Input => "dB",
            Self::Release => "ms",
            Self::Lookahead => "ms",
            Self::Ceiling => "dB",
            Self::Mode => "",
            Self::Routing => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Input => format!("{value:.3} {}", self.unit()),
            Self::Release => format!("{value:.3} {}", self.unit()),
            Self::Lookahead => format!("{value:.3} {}", self.unit()),
            Self::Ceiling => format!("{value:.3} {}", self.unit()),
            Self::Mode => ["Peak", "True Peak"][value as usize].into(),
            Self::Routing => ["L/R", "M/S"][value as usize].into(),
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
            Self::Input => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Release => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Lookahead => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Ceiling => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Mode => ["peak", "true peak"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
            Self::Routing => ["l/r", "m/s"]
                .iter()
                .position(|s| *s == text)
                .map(|i| i as f64)
                .or_else(|| text.parse().ok())
                .ok_or(ParameterError)?,
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::LIMITER_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::LIMITER_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 2] = [
    (
        "Limit",
        &[
            (Parameter::Input, "db", "Db"),
            (Parameter::Release, "scalar", "Scalar"),
            (Parameter::Lookahead, "scalar", "Scalar"),
        ],
    ),
    (
        "Out",
        &[
            (Parameter::Ceiling, "db", "Db"),
            (Parameter::Mode, "mode", "Mode"),
            (Parameter::Routing, "mode", "Mode"),
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
