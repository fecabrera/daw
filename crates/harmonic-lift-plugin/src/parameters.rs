use daw_unit_core::gain;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Input = 101,
    Quality = 102,
    Band1Power = 103,
    Band1Frequency = 104,
    Band1Gain = 105,
    Band2Power = 106,
    Band2Frequency = 107,
    Band2Gain = 108,
    Band3Power = 109,
    Band3Frequency = 110,
    Band3Gain = 111,
    Band4Power = 112,
    Band4Frequency = 113,
    Band4Gain = 114,
    Band5Power = 115,
    Band5Frequency = 116,
    Band5Gain = 117,
    Band6Power = 118,
    Band6Frequency = 119,
    Band6Gain = 120,
    Drive = 121,
    Push = 122,
    Limit = 123,
    Level = 124,
    DryWet = 125,
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
    pub const COUNT: usize = 26;
    pub const ALL: [Self; 26] = [
        Self::Bypass,
        Self::Input,
        Self::Quality,
        Self::Band1Power,
        Self::Band1Frequency,
        Self::Band1Gain,
        Self::Band2Power,
        Self::Band2Frequency,
        Self::Band2Gain,
        Self::Band3Power,
        Self::Band3Frequency,
        Self::Band3Gain,
        Self::Band4Power,
        Self::Band4Frequency,
        Self::Band4Gain,
        Self::Band5Power,
        Self::Band5Frequency,
        Self::Band5Gain,
        Self::Band6Power,
        Self::Band6Frequency,
        Self::Band6Gain,
        Self::Drive,
        Self::Push,
        Self::Limit,
        Self::Level,
        Self::DryWet,
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
            Self::Input => ("input", "Wet Input", -60.0, 12.0, 0.0, Scale::Linear, 0),
            Self::Quality => ("quality", "Quality", 0.0, 1.0, 0.0, Scale::Choice, 1),
            Self::Band1Power => (
                "band_1_power",
                "Low Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band1Frequency => (
                "band_1_frequency",
                "Low Frequency",
                20.0,
                250.0,
                135.0,
                Scale::Linear,
                0,
            ),
            Self::Band1Gain => ("band_1_gain", "Low Gain", 0.0, 24.0, 0.0, Scale::Linear, 0),
            Self::Band2Power => (
                "band_2_power",
                "Low Mid Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band2Frequency => (
                "band_2_frequency",
                "Low Mid Frequency",
                250.0,
                500.0,
                375.0,
                Scale::Linear,
                0,
            ),
            Self::Band2Gain => (
                "band_2_gain",
                "Low Mid Gain",
                0.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Band3Power => (
                "band_3_power",
                "Mid Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band3Frequency => (
                "band_3_frequency",
                "Mid Frequency",
                500.0,
                2000.0,
                1250.0,
                Scale::Linear,
                0,
            ),
            Self::Band3Gain => ("band_3_gain", "Mid Gain", 0.0, 24.0, 0.0, Scale::Linear, 0),
            Self::Band4Power => (
                "band_4_power",
                "High Mid Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band4Frequency => (
                "band_4_frequency",
                "High Mid Frequency",
                2000.0,
                4000.0,
                3000.0,
                Scale::Linear,
                0,
            ),
            Self::Band4Gain => (
                "band_4_gain",
                "High Mid Gain",
                0.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Band5Power => (
                "band_5_power",
                "Presence Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band5Frequency => (
                "band_5_frequency",
                "Presence Frequency",
                4000.0,
                6000.0,
                5000.0,
                Scale::Linear,
                0,
            ),
            Self::Band5Gain => (
                "band_5_gain",
                "Presence Gain",
                0.0,
                24.0,
                0.0,
                Scale::Linear,
                0,
            ),
            Self::Band6Power => (
                "band_6_power",
                "High Power",
                0.0,
                1.0,
                0.0,
                Scale::Boolean,
                1,
            ),
            Self::Band6Frequency => (
                "band_6_frequency",
                "High Frequency",
                6000.0,
                12000.0,
                9000.0,
                Scale::Linear,
                0,
            ),
            Self::Band6Gain => ("band_6_gain", "High Gain", 0.0, 24.0, 0.0, Scale::Linear, 0),
            Self::Drive => ("drive", "Drive", 0.0, 1.0, 0.0, Scale::Linear, 0),
            Self::Push => ("push", "Push", 0.0, 1.0, 0.0, Scale::Linear, 0),
            Self::Limit => ("limit", "Limit Level", -24.0, 0.0, 0.0, Scale::Linear, 0),
            Self::Level => ("level", "Level", -1000.0, 12.0, 0.0, Scale::Gain, 0),
            Self::DryWet => ("dry_wet", "Dry Wet", 0.0, 1.0, 1.0, Scale::Linear, 0),
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
            Self::Quality => "",
            Self::Band1Power => "",
            Self::Band1Frequency => "Hz",
            Self::Band1Gain => "dB",
            Self::Band2Power => "",
            Self::Band2Frequency => "Hz",
            Self::Band2Gain => "dB",
            Self::Band3Power => "",
            Self::Band3Frequency => "Hz",
            Self::Band3Gain => "dB",
            Self::Band4Power => "",
            Self::Band4Frequency => "Hz",
            Self::Band4Gain => "dB",
            Self::Band5Power => "",
            Self::Band5Frequency => "Hz",
            Self::Band5Gain => "dB",
            Self::Band6Power => "",
            Self::Band6Frequency => "Hz",
            Self::Band6Gain => "dB",
            Self::Drive => "%",
            Self::Push => "%",
            Self::Limit => "dB",
            Self::Level => "dB",
            Self::DryWet => "%",
        }
    }
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(if self == Self::Quality {
            ["Normal", "HQ 2×"][value as usize].into()
        } else if self.info().scale == Scale::Boolean {
            if value == 0.0 { "Off" } else { "On" }.into()
        } else if self == Self::Level {
            format!("{} dB", gain::format_db(value))
        } else if self.unit() == "%" {
            format!("{:.3}%", value * 100.0)
        } else {
            format!("{value:.3} {}", self.unit())
        })
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim().to_lowercase();
        let value = if self == Self::Quality {
            match text.as_str() {
                "normal" => 0.0,
                "hq 2×" | "hq" => 1.0,
                _ => text.parse().map_err(|_| ParameterError)?,
            }
        } else if self.info().scale == Scale::Boolean {
            match text.as_str() {
                "off" => 0.0,
                "on" => 1.0,
                _ => text.parse().map_err(|_| ParameterError)?,
            }
        } else if self == Self::Level {
            gain::parse_db(&text).ok_or(ParameterError)?
        } else {
            let value = text
                .trim_end_matches(self.unit().to_lowercase().as_str())
                .trim()
                .parse::<f64>()
                .map_err(|_| ParameterError)?;
            if self.unit() == "%" {
                value / 100.0
            } else {
                value
            }
        };
        self.validate(value)
    }
}
include!("../../saturator-plugin/src/conversion.rs");
#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::HARMONIC_LIFT_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::HARMONIC_LIFT_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (
    &'static str,
    &'static [(Parameter, &'static str, &'static str)],
);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 9] = [
    (
        "Input",
        &[
            (Parameter::Input, "db", "Db"),
            (Parameter::Quality, "selector", "Selector"),
        ],
    ),
    (
        "Low",
        &[
            (Parameter::Band1Frequency, "frequency", "Frequency"),
            (Parameter::Band1Gain, "db", "Db"),
        ],
    ),
    (
        "Low Mid",
        &[
            (Parameter::Band2Frequency, "frequency", "Frequency"),
            (Parameter::Band2Gain, "db", "Db"),
        ],
    ),
    (
        "Mid",
        &[
            (Parameter::Band3Frequency, "frequency", "Frequency"),
            (Parameter::Band3Gain, "db", "Db"),
        ],
    ),
    (
        "High Mid",
        &[
            (Parameter::Band4Frequency, "frequency", "Frequency"),
            (Parameter::Band4Gain, "db", "Db"),
        ],
    ),
    (
        "Presence",
        &[
            (Parameter::Band5Frequency, "frequency", "Frequency"),
            (Parameter::Band5Gain, "db", "Db"),
        ],
    ),
    (
        "High",
        &[
            (Parameter::Band6Frequency, "frequency", "Frequency"),
            (Parameter::Band6Gain, "db", "Db"),
        ],
    ),
    (
        "Maximizer",
        &[
            (Parameter::Drive, "percent", "Percent"),
            (Parameter::Push, "percent", "Percent"),
            (Parameter::Limit, "db", "Db"),
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
pub(crate) fn diagram_binding(_: Parameter) -> Option<&'static str> {
    None
}
#[allow(dead_code)]
pub(crate) fn footer_power(index: usize) -> Option<Parameter> {
    match index {
        1 => Some(Parameter::Band1Power),
        2 => Some(Parameter::Band2Power),
        3 => Some(Parameter::Band3Power),
        4 => Some(Parameter::Band4Power),
        5 => Some(Parameter::Band5Power),
        6 => Some(Parameter::Band6Power),
        _ => None,
    }
}
