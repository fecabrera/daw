use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Threshold = 101,
    Ratio = 102,
    Attack = 103,
    Release = 104,
    Knee = 105,
    Level = 106,
    DryWet = 107,
    Detection = 108,
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
    pub const COUNT: usize = 9;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Threshold,
        Self::Ratio,
        Self::Attack,
        Self::Release,
        Self::Knee,
        Self::Level,
        Self::DryWet,
        Self::Detection,
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
            Self::Threshold => (
                "threshold",
                "Threshold",
                -60.0,
                0.0,
                -18.0,
                Scale::Linear,
                0,
            ),
            Self::Ratio => ("ratio", "Ratio", 1.0, 20.0, 4.0, Scale::Logarithmic, 0),
            Self::Attack => ("attack", "Attack", 0.1, 200.0, 10.0, Scale::Logarithmic, 0),
            Self::Release => (
                "release",
                "Release",
                10.0,
                2000.0,
                100.0,
                Scale::Logarithmic,
                0,
            ),
            Self::Knee => ("knee", "Knee", 0.0, 24.0, 6.0, Scale::Linear, 0),
            Self::Level => ("level", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry Wet", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::Detection => ("detection", "Detection", 0.0, 1.0, 0.0, Scale::Choice, 1),
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
            Self::Threshold => "dB",
            Self::Ratio => ":1",
            Self::Attack => "ms",
            Self::Release => "ms",
            Self::Knee => "dB",
            Self::Level => "dB",
            Self::DryWet => "%",
            Self::Detection => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Threshold => format!("{value:.3} {}", self.unit()),
            Self::Ratio => format!("{value:.3} {}", self.unit()),
            Self::Attack => format!("{value:.3} {}", self.unit()),
            Self::Release => format!("{value:.3} {}", self.unit()),
            Self::Knee => format!("{value:.3} {}", self.unit()),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::DryWet => format!("{:.0}%", value * 100.0),
            Self::Detection => ["Peak", "RMS"][value as usize].into(),
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
            Self::Threshold => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Ratio => text
                .strip_suffix(":1")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::Attack => text
                .strip_suffix("ms")
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
            Self::Knee => text
                .strip_suffix("db")
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
            Self::Detection => ["peak", "rms"]
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
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::COMPRESSOR_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::COMPRESSOR_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 3] = [
    (
        "Comp",
        &[
            (Parameter::Threshold, "db", "Db"),
            (Parameter::Ratio, "scalar", "Scalar"),
            (Parameter::Detection, "mode", "Mode"),
        ],
    ),
    (
        "Response",
        &[
            (Parameter::Attack, "scalar", "Scalar"),
            (Parameter::Release, "scalar", "Scalar"),
            (Parameter::Knee, "db", "Db"),
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
pub(crate) fn diagram_binding(_p: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(_index: usize) -> Option<Parameter> {
    None
}
