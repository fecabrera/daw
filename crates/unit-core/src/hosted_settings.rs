//! Saved external-unit contracts. Native code, installed paths and mutable DSP stay in the host.
use crate::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const HOSTED_TYPE_ID: &str = "daw.external";
pub const HOSTED_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedStateBlob {
    pub sha256: String,
    pub length_bytes: u64,
}
impl HostedStateBlob {
    pub fn valid(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.length_bytes <= 16 * 1024 * 1024
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedStateReference {
    pub component: HostedStateBlob,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<HostedStateBlob>,
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostedParameterBinding {
    pub target: Uuid,
    pub definition: HostedParameter,
    /// Diagnostic base metadata when no restorable state exists. It must not be
    /// replayed as a no-state CLAP preset or project-state fallback.
    pub base_value: f64,
    #[serde(default)]
    pub edit_revision: u64,
}
impl HostedParameterBinding {
    pub fn new(definition: HostedParameter, base_value: f64) -> Self {
        Self {
            target: Uuid::new_v4(),
            definition,
            base_value,
            edit_revision: 0,
        }
    }
    pub fn valid(&self) -> bool {
        let (lo, hi) = self.definition.domain.bounds();
        !self.target.is_nil()
            && self.definition.valid()
            && self.base_value.is_finite()
            && (lo..=hi).contains(&self.base_value)
    }
    pub fn amount_limit(&self) -> f32 {
        let (lo, hi) = self.definition.domain.bounds();
        (hi - lo).min(f64::from(f32::MAX)) as f32
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum HostedInputId {
    Clap {
        id: u32,
    },
    /// VST3 bus indices are valid only within the negotiated layout signature.
    Vst3 {
        index: u32,
        layout: String,
    },
}
impl HostedInputId {
    pub fn valid(&self, format: PluginFormat) -> bool {
        match self {
            Self::Clap { id } => format == PluginFormat::Clap && *id != u32::MAX,
            Self::Vst3 { index, layout } => {
                format == PluginFormat::Vst3
                    && *index < 32
                    && layout.len() == 64
                    && layout.bytes().all(|b| b.is_ascii_hexdigit())
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedInputChannels {
    #[default]
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}
impl HostedInputChannels {
    pub fn apply(self, source: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Stereo => source,
            Self::Left => [source[0]; 2],
            Self::Right => [source[1]; 2],
            Self::Mid => [(source[0] + source[1]) * 0.5; 2],
            Self::Side => [(source[0] - source[1]) * 0.5; 2],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostedAudioInput {
    pub id: Uuid,
    pub source_track: Uuid,
    pub port: HostedInputId,
    pub tap: SidechainTap,
    #[serde(default)]
    pub channels: HostedInputChannels,
    pub gain_db: f32,
    pub enabled: bool,
}
impl HostedAudioInput {
    pub fn valid(&self, format: PluginFormat) -> bool {
        !self.id.is_nil()
            && !self.source_track.is_nil()
            && self.port.valid(format)
            && self.gain_db.is_finite()
            && (-120.0..=24.0).contains(&self.gain_db)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostedSettings {
    pub version: u32,
    pub id: Uuid,
    pub plugin: HostedPluginDescriptor,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
    pub parameters: Vec<HostedParameterBinding>,
    #[serde(default)]
    pub audio_inputs: Vec<HostedAudioInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<HostedStateReference>,
    #[serde(default)]
    pub edit_revision: u64,
    #[serde(default)]
    pub latency_frames: u32,
    #[serde(default)]
    pub tail_frames: u64,
    #[serde(default = "metadata_rate")]
    pub metadata_rate: u32,
    #[serde(default = "stereo_channels")]
    pub output_channels: u32,
}
fn stereo_channels() -> u32 {
    2
}
fn metadata_rate() -> u32 {
    48000
}
impl HostedSettings {
    pub fn new(plugin: HostedPluginDescriptor, parameters: Vec<HostedParameterBinding>) -> Self {
        Self {
            version: HOSTED_STATE_VERSION,
            id: Uuid::new_v4(),
            plugin,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            parameters,
            audio_inputs: Vec::new(),
            state: None,
            edit_revision: 0,
            latency_frames: 0,
            tail_frames: 0,
            metadata_rate: 48000,
            output_channels: 2,
        }
    }
    pub fn valid(&self) -> bool {
        let format = self.plugin.identity.format();
        let mut ids = std::collections::HashSet::new();
        let mut native = std::collections::HashSet::new();
        self.version == HOSTED_STATE_VERSION
            && !self.id.is_nil()
            && self.plugin.valid()
            && self.plugin.role != HostedRole::Unknown
            && (1..=2).contains(&self.output_channels)
            && self.latency_frames <= self.metadata_rate.saturating_mul(10)
            && self.modulations.len() <= 4096
            && self.parameters.len() <= 4096
            && self.parameters.iter().all(|p| {
                p.valid()
                    && p.edit_revision <= self.edit_revision
                    && ids.insert(p.target)
                    && native.insert(p.definition.id)
            })
            && self.state.as_ref().is_none_or(|s| {
                s.component.valid()
                    && s.controller
                        .as_ref()
                        .is_none_or(|b| b.valid() && format == PluginFormat::Vst3)
                    && s.revision <= self.edit_revision
            })
            && {
                let mut routes = std::collections::HashSet::new();
                let mut ports = Vec::new();
                self.audio_inputs.len() <= 32
                    && self.audio_inputs.iter().all(|r| {
                        r.valid(format)
                            && routes.insert(r.id)
                            && if ports.contains(&&r.port) {
                                false
                            } else {
                                ports.push(&r.port);
                                true
                            }
                    })
            }
            && ModulationDestinationSettings::valid_modulations(self)
            && [44100, 48000, 88200, 96000, 176400, 192000].contains(&self.metadata_rate)
    }
    pub fn recallable(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.revision == self.edit_revision)
    }
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = Uuid::new_v4();
        for binding in &mut copy.parameters {
            let old = binding.target;
            binding.target = Uuid::new_v4();
            for route in &mut copy.modulations {
                if route.target == Some(old) {
                    route.target = Some(binding.target);
                }
            }
        }
        for route in &mut copy.modulations {
            route.id = Uuid::new_v4();
        }
        for input in &mut copy.audio_inputs {
            input.id = Uuid::new_v4();
        }
        copy
    }
}

/// Future source payloads remain exact data and render silence.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // Inline with the containing layer; future payloads are uncommon.
pub enum HostedSource {
    Known(HostedSettings),
    Missing(serde_json::Value),
}
impl HostedSource {
    pub fn settings(&self) -> Option<&HostedSettings> {
        match self {
            Self::Known(s) => Some(s),
            Self::Missing(_) => None,
        }
    }
    pub fn settings_mut(&mut self) -> Option<&mut HostedSettings> {
        match self {
            Self::Known(s) => Some(s),
            Self::Missing(_) => None,
        }
    }
    pub fn valid(&self) -> bool {
        match self {
            Self::Known(s) => s.valid() && s.plugin.role == HostedRole::Instrument,
            Self::Missing(v) => {
                v.is_object()
                    && v.get("version")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|n| n > u64::from(HOSTED_STATE_VERSION))
            }
        }
    }
    pub fn duplicate(&self) -> Self {
        match self {
            Self::Known(s) => Self::Known(s.duplicate()),
            Self::Missing(v) => Self::Missing(v.clone()),
        }
    }
}
impl Serialize for HostedSource {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Known(v) => v.serialize(s),
            Self::Missing(v) => v.serialize(s),
        }
    }
}
impl<'de> Deserialize<'de> for HostedSource {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        if v.get("version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|n| n > u64::from(HOSTED_STATE_VERSION))
        {
            Ok(Self::Missing(v))
        } else {
            serde_json::from_value(v)
                .map(Self::Known)
                .map_err(serde::de::Error::custom)
        }
    }
}
impl ModulationDestinationSettings for HostedSettings {
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &[ModulationParameter::HostedValue];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.modulations.iter().all(|r| {
            r.valid()
                && r.parameter == ModulationParameter::HostedValue
                && ids.insert(r.id)
                && r.target
                    .and_then(|target| self.parameters.iter().find(|p| p.target == target))
                    .is_some_and(|p| {
                        p.definition.accepts_modulation() && r.amount.abs() <= p.amount_limit()
                    })
        })
    }
}

/// One typed dynamic parameter target; the common resolver still evaluates all
/// providers, routes, additive sums and the existing 1 ms offset smoother.
pub struct HostedScalar {
    pub target: Uuid,
    pub definition: HostedParameter,
    smoother: ModulationSmoother<1>,
    pub effective: f64,
}
impl HostedScalar {
    pub fn inherit(&mut self, previous: &mut Self) {
        if self.target == previous.target && self.definition == previous.definition {
            std::mem::swap(&mut self.smoother, &mut previous.smoother);
            self.effective = previous.effective;
        }
    }
    pub fn new(context: ProcessingContext, binding: &HostedParameterBinding) -> Self {
        Self {
            target: binding.target,
            definition: binding.definition.clone(),
            smoother: ModulationSmoother::new(context),
            effective: binding.base_value,
        }
    }
}
impl ModulationDestination for HostedScalar {
    type Audio = f64;
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &[ModulationParameter::HostedValue];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        Some(self.target)
    }
    fn process_with_modulation(
        &mut self,
        base: f64,
        input: &ModulationInputs,
        _: ModulationContext,
    ) -> f64 {
        let offset = self
            .smoother
            .process([input.offset(ModulationParameter::HostedValue)])[0];
        self.effective = self
            .definition
            .domain
            .constrain(base + offset)
            .unwrap_or(base);
        self.effective
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> HostedSettings {
        HostedSettings::new(
            crate::HostedPluginDescriptor {
                identity: crate::PluginIdentity::Clap("org.daw.owned".into()),
                name: "Fixture".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: crate::HostedRole::Effect,
            },
            vec![HostedParameterBinding::new(
                crate::HostedParameter {
                    id: 42,
                    name: "Offset".into(),
                    module: String::new(),
                    domain: crate::HostedParameterDomain::Plain {
                        minimum: -20.0,
                        maximum: 30.0,
                        stepped: false,
                    },
                    default_value: -3.0,
                    automatable: true,
                    modulatable: true,
                    read_only: false,
                    hidden: false,
                },
                -3.0,
            )],
        )
    }
    #[test]
    fn exact_copy_refreshes_owned_ids_and_shares_opaque_reference() {
        let mut original = settings();
        original.state = Some(HostedStateReference {
            component: HostedStateBlob {
                sha256: "a".repeat(64),
                length_bytes: 3,
            },
            controller: None,
            revision: 0,
        });
        assert!(original.valid() && original.recallable());
        let copy = original.duplicate();
        assert_ne!(copy.id, original.id);
        assert_ne!(copy.parameters[0].target, original.parameters[0].target);
        assert_eq!(copy.parameters[0].definition.id, 42);
        assert_eq!(copy.state, original.state);
        original.edit_revision = 1;
        assert!(!original.recallable());
    }
    #[test]
    fn unknown_source_version_round_trips_without_interpreting_fields() {
        let payload =
            serde_json::json!({"version":999,"vendor":"opaque","settings":{"unknown":[1,2,3]}});
        let source: HostedSource = serde_json::from_value(payload.clone()).unwrap();
        assert!(source.valid());
        assert!(source.settings().is_none());
        assert_eq!(serde_json::to_value(source).unwrap(), payload);
    }
    #[test]
    fn invalid_parameter_routes_and_nonfinite_bases_are_rejected() {
        let mut value = settings();
        value.parameters[0].base_value = f64::NAN;
        assert!(!value.valid());
        value.parameters[0].base_value = -3.0;
        let mut route = crate::Modulation::new(crate::ModulationParameter::HostedValue);
        route.target = Some(Uuid::new_v4());
        value.modulations.push(route);
        assert!(!value.valid());
    }
}
