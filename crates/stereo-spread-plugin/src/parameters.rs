use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    LowerIntensity = 101,
    LowerFrequency = 102,
    UpperIntensity = 103,
    UpperFrequency = 104,
    Order = 105,
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
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::LowerIntensity,
        Self::LowerFrequency,
        Self::UpperIntensity,
        Self::UpperFrequency,
        Self::Order,
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
            Self::LowerIntensity => (
                "lower_intensity",
                "Lower Intensity",
                -1.0,
                1.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::LowerFrequency => (
                "lower_frequency",
                "Lower Frequency",
                20.0,
                20000.0,
                20.0,
                Scale::Logarithmic,
                0,
            ),
            Self::UpperIntensity => (
                "upper_intensity",
                "Upper Intensity",
                -1.0,
                1.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::UpperFrequency => (
                "upper_frequency",
                "Upper Frequency",
                20.0,
                20000.0,
                20000.0,
                Scale::Logarithmic,
                0,
            ),
            Self::Order => ("order", "Order", 2.0, 12.0, 8.0, Scale::Choice, 10),
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
            Self::LowerIntensity => "%",
            Self::LowerFrequency => "Hz",
            Self::UpperIntensity => "%",
            Self::UpperFrequency => "Hz",
            Self::Order => "bands",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::LowerIntensity => format!("{:.0}%", value * 100.0),
            Self::LowerFrequency => format!("{value:.3} {}", self.unit()),
            Self::UpperIntensity => format!("{:.0}%", value * 100.0),
            Self::UpperFrequency => format!("{value:.3} {}", self.unit()),
            Self::Order => format!("{value:.3} {}", self.unit()),
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
            Self::LowerIntensity => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::LowerFrequency => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::UpperIntensity => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::UpperFrequency => text
                .strip_suffix("hz")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Order => text
                .strip_suffix("bands")
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
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::STEREO_SPREAD_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::STEREO_SPREAD_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 3] = [
    (
        "Lower",
        &[
            (Parameter::LowerIntensity, "percent", "Percent"),
            (Parameter::LowerFrequency, "frequency", "Frequency"),
        ],
    ),
    (
        "Upper",
        &[
            (Parameter::UpperIntensity, "percent", "Percent"),
            (Parameter::UpperFrequency, "frequency", "Frequency"),
        ],
    ),
    ("Spread", &[(Parameter::Order, "integer", "Integer")]),
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
