use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    LowFrequency = 101,
    HighFrequency = 102,
    LowDepth = 103,
    LowInputDb = 104,
    LowOutputDb = 105,
    LowAttackMs = 106,
    LowReleaseMs = 107,
    LowEnabled = 108,
    MidDepth = 109,
    MidInputDb = 110,
    MidOutputDb = 111,
    MidAttackMs = 112,
    MidReleaseMs = 113,
    MidEnabled = 114,
    HighDepth = 115,
    HighInputDb = 116,
    HighOutputDb = 117,
    HighAttackMs = 118,
    HighReleaseMs = 119,
    HighEnabled = 120,
    Level = 121,
    DryWet = 122,
    Detection = 123,
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
    pub const COUNT: usize = 24;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::LowFrequency,
        Self::HighFrequency,
        Self::LowDepth,
        Self::LowInputDb,
        Self::LowOutputDb,
        Self::LowAttackMs,
        Self::LowReleaseMs,
        Self::LowEnabled,
        Self::MidDepth,
        Self::MidInputDb,
        Self::MidOutputDb,
        Self::MidAttackMs,
        Self::MidReleaseMs,
        Self::MidEnabled,
        Self::HighDepth,
        Self::HighInputDb,
        Self::HighOutputDb,
        Self::HighAttackMs,
        Self::HighReleaseMs,
        Self::HighEnabled,
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
            Self::LowFrequency => (
                "low_frequency",
                "Low Frequency",
                20.0,
                2000.0,
                250.0,
                Scale::Logarithmic,
                0,
            ),
            Self::HighFrequency => (
                "high_frequency",
                "High Frequency",
                200.0,
                20000.0,
                2500.0,
                Scale::Logarithmic,
                0,
            ),
            Self::LowDepth => ("low_depth", "Low Depth", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::LowInputDb => (
                "low_input_db",
                "Low Input Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::LowOutputDb => (
                "low_output_db",
                "Low Output Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::LowAttackMs => (
                "low_attack_ms",
                "Low Attack Ms",
                0.1,
                200.0,
                10.0,
                Scale::Logarithmic,
                0,
            ),
            Self::LowReleaseMs => (
                "low_release_ms",
                "Low Release Ms",
                10.0,
                2000.0,
                100.0,
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
            Self::MidDepth => ("mid_depth", "Mid Depth", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::MidInputDb => (
                "mid_input_db",
                "Mid Input Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::MidOutputDb => (
                "mid_output_db",
                "Mid Output Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::MidAttackMs => (
                "mid_attack_ms",
                "Mid Attack Ms",
                0.1,
                200.0,
                10.0,
                Scale::Logarithmic,
                0,
            ),
            Self::MidReleaseMs => (
                "mid_release_ms",
                "Mid Release Ms",
                10.0,
                2000.0,
                100.0,
                Scale::Logarithmic,
                0,
            ),
            Self::MidEnabled => (
                "mid_enabled",
                "Mid Enabled",
                0.0,
                1.0,
                1.0,
                Scale::Boolean,
                1,
            ),
            Self::HighDepth => ("high_depth", "High Depth", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::HighInputDb => (
                "high_input_db",
                "High Input Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::HighOutputDb => (
                "high_output_db",
                "High Output Db",
                -24.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::HighAttackMs => (
                "high_attack_ms",
                "High Attack Ms",
                0.1,
                200.0,
                10.0,
                Scale::Logarithmic,
                0,
            ),
            Self::HighReleaseMs => (
                "high_release_ms",
                "High Release Ms",
                10.0,
                2000.0,
                100.0,
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
            Self::LowFrequency => "Hz",
            Self::HighFrequency => "Hz",
            Self::LowDepth => "%",
            Self::LowInputDb => "dB",
            Self::LowOutputDb => "dB",
            Self::LowAttackMs => "ms",
            Self::LowReleaseMs => "ms",
            Self::LowEnabled => "",
            Self::MidDepth => "%",
            Self::MidInputDb => "dB",
            Self::MidOutputDb => "dB",
            Self::MidAttackMs => "ms",
            Self::MidReleaseMs => "ms",
            Self::MidEnabled => "",
            Self::HighDepth => "%",
            Self::HighInputDb => "dB",
            Self::HighOutputDb => "dB",
            Self::HighAttackMs => "ms",
            Self::HighReleaseMs => "ms",
            Self::HighEnabled => "",
            Self::Level => "dB",
            Self::DryWet => "%",
            Self::Detection => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::LowFrequency => format!("{value:.3} {}", self.unit()),
            Self::HighFrequency => format!("{value:.3} {}", self.unit()),
            Self::LowDepth => format!("{:.0}%", value * 100.0),
            Self::LowInputDb => format!("{value:.3} {}", self.unit()),
            Self::LowOutputDb => format!("{value:.3} {}", self.unit()),
            Self::LowAttackMs => format!("{value:.3} {}", self.unit()),
            Self::LowReleaseMs => format!("{value:.3} {}", self.unit()),
            Self::LowEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::MidDepth => format!("{:.0}%", value * 100.0),
            Self::MidInputDb => format!("{value:.3} {}", self.unit()),
            Self::MidOutputDb => format!("{value:.3} {}", self.unit()),
            Self::MidAttackMs => format!("{value:.3} {}", self.unit()),
            Self::MidReleaseMs => format!("{value:.3} {}", self.unit()),
            Self::MidEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::HighDepth => format!("{:.0}%", value * 100.0),
            Self::HighInputDb => format!("{value:.3} {}", self.unit()),
            Self::HighOutputDb => format!("{value:.3} {}", self.unit()),
            Self::HighAttackMs => format!("{value:.3} {}", self.unit()),
            Self::HighReleaseMs => format!("{value:.3} {}", self.unit()),
            Self::HighEnabled => if value == 0.0 { "Off" } else { "On" }.into(),
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
            Self::LowFrequency => text
                .strip_suffix("hz")
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
            Self::LowDepth => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::LowInputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowOutputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowAttackMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowReleaseMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::LowEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::MidDepth => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::MidInputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::MidOutputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::MidAttackMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::MidReleaseMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::MidEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::HighDepth => {
                text.trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::HighInputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighOutputDb => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighAttackMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighReleaseMs => text
                .strip_suffix("ms")
                .unwrap_or(&text)
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?,
            Self::HighEnabled => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
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
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::OTT_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::OTT_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 4] = [
    (
        "Low",
        &[
            (Parameter::LowFrequency, "frequency", "Frequency"),
            (Parameter::LowDepth, "percent", "Percent"),
            (Parameter::LowInputDb, "db", "Db"),
            (Parameter::LowOutputDb, "db", "Db"),
            (Parameter::LowAttackMs, "scalar", "Scalar"),
            (Parameter::LowReleaseMs, "scalar", "Scalar"),
        ],
    ),
    (
        "Mid",
        &[
            (Parameter::MidDepth, "percent", "Percent"),
            (Parameter::MidInputDb, "db", "Db"),
            (Parameter::MidOutputDb, "db", "Db"),
            (Parameter::MidAttackMs, "scalar", "Scalar"),
            (Parameter::MidReleaseMs, "scalar", "Scalar"),
        ],
    ),
    (
        "High",
        &[
            (Parameter::HighFrequency, "frequency", "Frequency"),
            (Parameter::HighDepth, "percent", "Percent"),
            (Parameter::HighInputDb, "db", "Db"),
            (Parameter::HighOutputDb, "db", "Db"),
            (Parameter::HighAttackMs, "scalar", "Scalar"),
            (Parameter::HighReleaseMs, "scalar", "Scalar"),
        ],
    ),
    (
        "Out",
        &[
            (Parameter::Level, "gain", "Gain"),
            (Parameter::DryWet, "percent", "Percent"),
            (Parameter::Detection, "mode", "Mode"),
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
