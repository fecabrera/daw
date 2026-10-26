use daw_unit_core::{BitcrusherSettings, SILENT_GAIN_DB, gain};
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Level = 101,
    Drive = 102,
    Resolution = 103,
    Downsampling = 104,
    DryWet = 105,
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
        Self::Level,
        Self::Drive,
        Self::Resolution,
        Self::Downsampling,
        Self::DryWet,
    ];
    pub fn from_id(id: u32) -> Result<Self, ParameterError> {
        Self::ALL
            .into_iter()
            .find(|p| *p as u32 == id)
            .ok_or(ParameterError)
    }
    #[allow(dead_code)]
    pub(crate) fn index(self) -> usize {
        self as usize - 100
    }
    pub fn info(self) -> ParameterInfo {
        let defaults = BitcrusherSettings::default();
        let (key, name, minimum, maximum, default, scale, steps) = match self {
            Self::Bypass => (
                "bypass",
                "Bypass",
                0.0,
                1.0,
                f64::from(!defaults.enabled),
                Scale::Boolean,
                1,
            ),
            Self::Level => (
                "level",
                "Output level",
                f64::from(SILENT_GAIN_DB),
                12.0,
                f64::from(defaults.level_db),
                Scale::Gain,
                0,
            ),
            Self::Drive => (
                "drive",
                "Drive",
                0.0,
                24.0,
                f64::from(defaults.drive_db),
                Scale::Linear,
                0,
            ),
            Self::Resolution => (
                "resolution",
                "Resolution",
                1.0,
                24.0,
                f64::from(defaults.resolution_bits),
                Scale::Choice,
                23,
            ),
            Self::Downsampling => (
                "downsampling",
                "Downsampling",
                1.0,
                40.0,
                f64::from(defaults.downsampling),
                Scale::Choice,
                39,
            ),
            Self::DryWet => (
                "dry_wet",
                "Dry/Wet",
                0.0,
                1.0,
                f64::from(defaults.dry_wet),
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
            Self::Level | Self::Drive => "dB",
            Self::DryWet => "%",
            Self::Resolution => "bits",
            Self::Downsampling => "×",
            _ => "",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::Drive => format!("{value:.1} dB"),
            Self::DryWet => format!("{:.0}%", value * 100.0),
            Self::Resolution => format!("{value:.0} bits"),
            Self::Downsampling => format!("{value:.0}×"),
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
            Self::Level => gain::parse_db(&text).ok_or(ParameterError)?,
            Self::Drive => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse()
                .map_err(|_| ParameterError)?,
            Self::DryWet => {
                text.strip_suffix('%')
                    .unwrap_or(&text)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::Resolution => text
                .strip_suffix("bits")
                .unwrap_or(&text)
                .trim()
                .parse()
                .map_err(|_| ParameterError)?,
            Self::Downsampling => text
                .trim_end_matches(['×', 'x'])
                .trim()
                .parse()
                .map_err(|_| ParameterError)?,
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::BITCRUSHER_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::BITCRUSHER_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 2] = [
    (
        "Crush",
        &[
            (Parameter::Drive, "db", "Db"),
            (Parameter::Resolution, "integer", "Integer"),
            (Parameter::Downsampling, "integer", "Integer"),
        ],
    ),
    (
        "Output",
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
pub(crate) fn diagram_binding(_: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(_: usize) -> Option<Parameter> {
    None
}
