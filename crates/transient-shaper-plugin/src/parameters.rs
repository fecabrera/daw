use daw_unit_core::{TransientShaperClipMode, gain};
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Level = 101,
    Attack = 102,
    Pump = 103,
    Sustain = 104,
    Speed = 105,
    DryWet = 106,
    ClipMode = 107,
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
    pub const COUNT: usize = 8;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Bypass,
        Self::Level,
        Self::Attack,
        Self::Pump,
        Self::Sustain,
        Self::Speed,
        Self::DryWet,
        Self::ClipMode,
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
        let (key, name, minimum, maximum, default, scale, steps) = match self {
            Self::Bypass => ("bypass", "Bypass", 0.0, 1.0, 0.0, Scale::Boolean, 1),
            Self::Level => ("level", "Output level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::Attack => ("attack", "Attack", -12.0, 12.0, 0.0, Scale::Linear, 0),
            Self::Pump => ("pump", "Pump", 0.0, 12.0, 0.0, Scale::Linear, 0),
            Self::Sustain => ("sustain", "Sustain", -12.0, 12.0, 0.0, Scale::Linear, 0),
            Self::Speed => ("speed", "Speed", 0.0, 1.0, 0.5, Scale::Linear, 0),
            Self::DryWet => ("dry_wet", "Dry/Wet", 0.0, 1.0, 1.0, Scale::Linear, 0),
            Self::ClipMode => ("clip_mode", "Clip Mode", 0.0, 3.0, 0.0, Scale::Choice, 3),
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
            Self::Level | Self::Attack | Self::Pump | Self::Sustain => "dB",
            Self::Speed | Self::DryWet => "%",
            _ => "",
        }
    }

    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Level => format!("{} dB", gain::format_db(value)),
            Self::Attack | Self::Pump | Self::Sustain => format!("{value:.1} dB"),
            Self::Speed | Self::DryWet => format!("{:.0}%", value * 100.0),
            Self::ClipMode => TransientShaperClipMode::ALL[value as usize].label().into(),
        })
    }

    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim().to_ascii_lowercase();
        let value = match self {
            Self::Bypass => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::Level => gain::parse_db(&text).ok_or(ParameterError)?,
            Self::Attack | Self::Pump | Self::Sustain => text
                .strip_suffix("db")
                .unwrap_or(&text)
                .trim()
                .parse()
                .map_err(|_| ParameterError)?,
            Self::Speed | Self::DryWet => {
                text.strip_suffix('%')
                    .unwrap_or(&text)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
            Self::ClipMode => TransientShaperClipMode::ALL
                .iter()
                .position(|mode| mode.label().eq_ignore_ascii_case(&text))
                .map(|index| index as f64)
                .ok_or(ParameterError)?,
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::TRANSIENT_SHAPER_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::TRANSIENT_SHAPER_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 3] = [
    (
        "Shape",
        &[
            (Parameter::Attack, "db", "Db"),
            (Parameter::Pump, "db", "Db"),
            (Parameter::Sustain, "db", "Db"),
        ],
    ),
    ("Detector", &[(Parameter::Speed, "percent", "Percent")]),
    (
        "Out",
        &[
            (Parameter::Level, "gain", "Gain"),
            (Parameter::DryWet, "percent", "Percent"),
            (Parameter::ClipMode, "selector", "Selector"),
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
