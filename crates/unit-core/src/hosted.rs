//! Portable external-plugin identities and parameter metadata. No native handles or paths.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginFormat {
    Clap,
    Vst3,
}
impl PluginFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
        }
    }
}

/// Canonical VST3 processor UUID bytes, independent of the platform COM byte order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "format", content = "id", rename_all = "snake_case")]
pub enum PluginIdentity {
    Clap(String),
    Vst3([u8; 16]),
}
impl PluginIdentity {
    pub fn format(&self) -> PluginFormat {
        match self {
            Self::Clap(_) => PluginFormat::Clap,
            Self::Vst3(_) => PluginFormat::Vst3,
        }
    }
    pub fn valid(&self) -> bool {
        match self {
            Self::Clap(id) => !id.trim().is_empty() && id.len() <= 4096 && !id.contains('\0'),
            Self::Vst3(id) => *id != [0; 16],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedRole {
    Effect,
    Instrument,
    /// Metadata does not establish an audio effect or instrument capability.
    Unknown,
}

/// Values and modulation deltas use this same domain. Never infer a logarithmic curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostedParameterDomain {
    /// CLAP plain units. Stepped values truncate toward zero after bounds.
    Plain {
        minimum: f64,
        maximum: f64,
        stepped: bool,
    },
    /// VST3 normalized values. Zero step count denotes a continuous parameter.
    Normalized { step_count: u32 },
}
impl HostedParameterDomain {
    pub fn valid(self) -> bool {
        match self {
            Self::Plain {
                minimum,
                maximum,
                stepped,
            } => {
                minimum.is_finite()
                    && maximum.is_finite()
                    && minimum <= maximum
                    && (maximum - minimum).is_finite()
                    && (!stepped || minimum.ceil() <= maximum.floor())
            }
            Self::Normalized { .. } => true,
        }
    }
    pub fn bounds(self) -> (f64, f64) {
        match self {
            Self::Plain {
                minimum, maximum, ..
            } => (minimum, maximum),
            Self::Normalized { .. } => (0.0, 1.0),
        }
    }
    /// Invalid descriptors/values are rejected before ABI event construction.
    pub fn constrain(self, value: f64) -> Option<f64> {
        if !self.valid() || !value.is_finite() {
            return None;
        }
        let (low, high) = self.bounds();
        let value = value.clamp(low, high);
        Some(match self {
            Self::Plain { stepped: true, .. } => value.trunc().clamp(low.ceil(), high.floor()),
            Self::Normalized { step_count } if step_count != 0 => {
                (value * f64::from(step_count)).round() / f64::from(step_count)
            }
            _ => value,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostedParameter {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub domain: HostedParameterDomain,
    pub default_value: f64,
    pub automatable: bool,
    pub modulatable: bool,
    pub read_only: bool,
    pub hidden: bool,
}
impl HostedParameter {
    pub fn valid(&self) -> bool {
        let (minimum, maximum) = self.domain.bounds();
        self.id != u32::MAX
            && !self.name.is_empty()
            && self.name.len() <= 4096
            && self.module.len() <= 4096
            && self.domain.valid()
            && self.default_value.is_finite()
            && (minimum..=maximum).contains(&self.default_value)
    }
    pub fn accepts_modulation(&self) -> bool {
        let (minimum, maximum) = self.domain.bounds();
        self.valid()
            && minimum < maximum
            && !self.read_only
            && (self.automatable || self.modulatable)
    }
}

/// Discovery metadata is data only; it never owns or activates an instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostedPluginDescriptor {
    pub identity: PluginIdentity,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub role: HostedRole,
}
impl HostedPluginDescriptor {
    pub fn valid(&self) -> bool {
        self.identity.valid()
            && !self.name.trim().is_empty()
            && [&self.name, &self.vendor, &self.version]
                .iter()
                .all(|s| s.len() <= 4096 && !s.contains('\0'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn format_domains_preserve_plain_steps_and_normalized_steps() {
        let plain = HostedParameterDomain::Plain {
            minimum: -12.0,
            maximum: 12.0,
            stepped: true,
        };
        assert_eq!(plain.constrain(-2.9), Some(-2.0));
        assert_eq!(plain.constrain(40.0), Some(12.0));
        assert_eq!(plain.constrain(f64::NAN), None);
        let normalized = HostedParameterDomain::Normalized { step_count: 3 };
        assert_eq!(normalized.constrain(0.6), Some(2.0 / 3.0));
        assert_eq!(normalized.constrain(-1.0), Some(0.0));
        assert!(
            !HostedParameterDomain::Plain {
                minimum: -f64::MAX,
                maximum: f64::MAX,
                stepped: false
            }
            .valid()
        );
    }
    #[test]
    fn canonical_identity_round_trips_without_platform_paths() {
        let identity = PluginIdentity::Vst3([42; 16]);
        assert_eq!(
            serde_json::from_str::<PluginIdentity>(&serde_json::to_string(&identity).unwrap())
                .unwrap(),
            identity
        );
        assert!(!PluginIdentity::Clap("bad\0id".into()).valid());
    }
    #[test]
    fn fractional_step_bounds_never_produce_a_fractional_event() {
        let stepped = HostedParameterDomain::Plain {
            minimum: 0.5,
            maximum: 4.5,
            stepped: true,
        };
        assert_eq!(stepped.constrain(0.5), Some(1.0));
        assert_eq!(stepped.constrain(4.5), Some(4.0));
        assert!(
            !HostedParameterDomain::Plain {
                minimum: 0.1,
                maximum: 0.5,
                stepped: true
            }
            .valid()
        );
    }
}
