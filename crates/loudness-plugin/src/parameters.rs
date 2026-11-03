use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Target = 101,
    Quality = 102,
    Clock = 103,
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
        f.write_str("invalid loudness parameter")
    }
}
impl std::error::Error for ParameterError {}
impl Parameter {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; 4] = [Self::Bypass, Self::Target, Self::Quality, Self::Clock];
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
        let (key, name, maximum, default, scale) = match self {
            Self::Bypass => ("bypass", "Bypass", 1.0, 0.0, Scale::Boolean),
            Self::Target => ("target", "Target", 2.0, 1.0, Scale::Choice),
            Self::Quality => ("hq", "TP quality", 1.0, 1.0, Scale::Choice),
            Self::Clock => ("continuous", "Clock", 1.0, 0.0, Scale::Choice),
        };
        ParameterInfo {
            parameter: self,
            key,
            name,
            minimum: 0.0,
            maximum,
            default,
            scale,
            steps: maximum as u32,
        }
    }
    pub fn unit(self) -> &'static str {
        ""
    }
    fn choices(self) -> &'static [&'static str] {
        match self {
            Self::Bypass => &["Off", "On"],
            Self::Target => &["Off", "−14 LUFS", "−23 LUFS"],
            Self::Quality => &["ECO", "HQ"],
            Self::Clock => &["Transport", "Continuous"],
        }
    }
    pub fn format(self, v: f64) -> Result<String, ParameterError> {
        self.validate(v)?;
        Ok(self.choices()[v as usize].into())
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let v = self
            .choices()
            .iter()
            .position(|s| s.eq_ignore_ascii_case(text.trim()))
            .map(|i| i as f64)
            .or_else(|| text.trim().parse().ok())
            .ok_or(ParameterError)?;
        self.validate(v)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::LOUDNESS_TYPE_ID;
pub(crate) const SETTINGS_VERSION: u32 = 1;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 7] = [
    (
        "Options",
        &[
            (Parameter::Target, "mode", "Mode"),
            (Parameter::Quality, "mode", "Mode"),
            (Parameter::Clock, "mode", "Mode"),
        ],
    ),
    ("Loudness", &[]),
    ("Maxima", &[]),
    ("Range", &[]),
    ("Dynamics", &[]),
    ("History", &[]),
    ("Out", &[]),
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
