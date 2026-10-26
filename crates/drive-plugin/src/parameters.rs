use daw_unit_core::{
    DRIVE_MAX_FREQUENCY_HZ, DRIVE_MAX_VOLUME_DB, DRIVE_MIN_FREQUENCY_HZ, DriveColorShape,
    DriveSettings, SILENT_GAIN_DB, gain,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    Bypass = 100,
    Volume = 101,
    Gain = 102,
    Tone = 103,
    ColorShape = 104,
    ColorStrength = 105,
    ColorFrequency = 106,
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
        f.write_str("unknown Drive parameter or invalid value")
    }
}
impl std::error::Error for ParameterError {}

impl Parameter {
    pub const COUNT: usize = 7;
    pub const ALL: [Self; 7] = [
        Self::Bypass,
        Self::Volume,
        Self::Gain,
        Self::Tone,
        Self::ColorShape,
        Self::ColorStrength,
        Self::ColorFrequency,
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
        let defaults = DriveSettings::default();
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
            Self::Volume => (
                "volume",
                "Volume",
                f64::from(SILENT_GAIN_DB),
                f64::from(DRIVE_MAX_VOLUME_DB),
                f64::from(defaults.volume_db),
                Scale::Gain,
                0,
            ),
            Self::Gain => (
                "gain",
                "Gain",
                0.0,
                1.0,
                f64::from(defaults.gain),
                Scale::Linear,
                0,
            ),
            Self::Tone => (
                "tone",
                "Tone",
                0.0,
                1.0,
                f64::from(defaults.tone),
                Scale::Linear,
                0,
            ),
            Self::ColorShape => (
                "color_shape",
                "Color shape",
                0.0,
                3.0,
                defaults.color_shape.index() as f64,
                Scale::Choice,
                3,
            ),
            Self::ColorStrength => (
                "color_strength",
                "Color strength",
                0.0,
                1.0,
                f64::from(defaults.color_strength),
                Scale::Linear,
                0,
            ),
            Self::ColorFrequency => (
                "color_frequency",
                "Color frequency",
                f64::from(DRIVE_MIN_FREQUENCY_HZ),
                f64::from(DRIVE_MAX_FREQUENCY_HZ),
                f64::from(defaults.color_frequency_hz),
                Scale::Logarithmic,
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
        match self.info().scale {
            Scale::Gain => "dB",
            Scale::Logarithmic => "Hz",
            Scale::Linear => "%",
            _ => "",
        }
    }
    pub fn validate(self, value: f64) -> Result<f64, ParameterError> {
        let info = self.info();
        if !value.is_finite()
            || !(info.minimum..=info.maximum).contains(&value)
            || (info.steps > 0 && value.fract() != 0.0)
        {
            return Err(ParameterError);
        }
        Ok(value)
    }
    pub fn normalize(self, plain: f64) -> Result<f64, ParameterError> {
        self.validate(plain)?;
        let info = self.info();
        Ok(match info.scale {
            Scale::Gain => gain::knob_position(plain),
            Scale::Logarithmic => (plain / info.minimum).ln() / (info.maximum / info.minimum).ln(),
            _ => (plain - info.minimum) / (info.maximum - info.minimum),
        })
    }
    pub fn denormalize(self, normalized: f64) -> Result<f64, ParameterError> {
        if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
            return Err(ParameterError);
        }
        let info = self.info();
        if normalized == 0.0 {
            return Ok(info.minimum);
        }
        if normalized == 1.0 {
            return Ok(info.maximum);
        }
        let value = match info.scale {
            Scale::Gain => gain::knob_db(normalized),
            Scale::Logarithmic => info.minimum * (info.maximum / info.minimum).powf(normalized),
            Scale::Boolean | Scale::Choice => (normalized * info.maximum).round(),
            Scale::Linear => info.minimum + normalized * (info.maximum - info.minimum),
        };
        self.validate(value)
    }
    /// Host text conversion is a readout, not a lossless state encoding.
    pub fn format(self, value: f64) -> Result<String, ParameterError> {
        self.validate(value)?;
        Ok(match self {
            Self::Bypass => if value == 0.0 { "Off" } else { "On" }.into(),
            Self::Volume => format!("{} dB", gain::format_db(value)),
            Self::ColorShape => DriveColorShape::ALL[value as usize].name().into(),
            Self::ColorFrequency => format!("{value:.0} Hz"),
            _ => format!("{:.0}%", value * 100.0),
        })
    }
    pub fn parse(self, text: &str) -> Result<f64, ParameterError> {
        let text = text.trim().to_lowercase();
        let value = match self {
            Self::Volume => gain::parse_db(&text).ok_or(ParameterError)?,
            Self::Bypass => match text.as_str() {
                "off" | "0" => 0.0,
                "on" | "1" => 1.0,
                _ => return Err(ParameterError),
            },
            Self::ColorShape => {
                if let Some(index) = DriveColorShape::ALL
                    .iter()
                    .position(|s| s.name().to_lowercase() == text)
                {
                    index as f64
                } else {
                    text.parse().map_err(|_| ParameterError)?
                }
            }
            Self::ColorFrequency => {
                let (number, factor) = if let Some(t) = text.strip_suffix("khz") {
                    (t, 1000.0)
                } else {
                    (text.strip_suffix("hz").unwrap_or(&text), 1.0)
                };
                number.trim().parse::<f64>().map_err(|_| ParameterError)? * factor
            }
            _ => {
                text.strip_suffix('%')
                    .unwrap_or(&text)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParameterError)?
                    / 100.0
            }
        };
        self.validate(value)
    }
}

#[allow(dead_code)]
pub(crate) const MANIFEST_TYPE_ID: &str = daw_unit_core::DRIVE_TYPE_ID;
#[allow(dead_code)]
pub(crate) const SETTINGS_VERSION: u32 = daw_unit_core::DRIVE_STATE_VERSION;
#[allow(dead_code)]
type StripBinding = (&'static str, [(Parameter, &'static str, &'static str); 3]);
#[allow(dead_code)]
pub(crate) const STRIP_BINDINGS: [StripBinding; 2] = [
    (
        "Drive",
        [
            (Parameter::Volume, "gain", "Gain"),
            (Parameter::Gain, "percent", "Percent"),
            (Parameter::Tone, "percent", "Percent"),
        ],
    ),
    (
        "Color",
        [
            (Parameter::ColorShape, "shape", "Shape"),
            (Parameter::ColorStrength, "percent", "Percent"),
            (Parameter::ColorFrequency, "frequency", "Frequency"),
        ],
    ),
];
#[allow(dead_code)]
pub(crate) fn enable_binding(p: Parameter) -> Option<Parameter> {
    matches!(p, Parameter::ColorStrength | Parameter::ColorFrequency)
        .then_some(Parameter::ColorShape)
}
#[allow(dead_code)]
pub(crate) fn diagram_binding(p: Parameter) -> Option<&'static str> {
    (p == Parameter::ColorShape).then_some("drive_color")
}
#[allow(dead_code)]
pub(crate) fn footer_power(index: usize) -> Option<Parameter> {
    let _ = index;
    None
}
