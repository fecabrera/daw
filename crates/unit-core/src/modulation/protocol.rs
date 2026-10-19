//! Portable modulation ports. Runtime methods must not allocate, block, or perform I/O.
use super::*;
use crate::{PortKind, ProcessingContext, UnitDescriptor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationScope {
    PerNote,
    Shared,
}
impl ModulationScope {
    /// A voice destination can consume local or shared controls. A bus cannot consume a voice.
    pub fn accepts(self, source: Self) -> bool {
        self == Self::PerNote || source == Self::Shared
    }
}
impl From<EnvelopeMode> for ModulationScope {
    fn from(mode: EnvelopeMode) -> Self {
        match mode {
            EnvelopeMode::PerNote => Self::PerNote,
            EnvelopeMode::Retrigger | EnvelopeMode::Legato => Self::Shared,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationRange {
    Unipolar,
    Bipolar,
}
impl ModulationRange {
    pub fn bounds(self) -> [f32; 2] {
        match self {
            Self::Unipolar => [0.0, 1.0],
            Self::Bipolar => [-1.0, 1.0],
        }
    }
    pub fn sanitize(self, value: f32) -> f32 {
        let [min, max] = self.bounds();
        if value.is_finite() {
            value.clamp(min, max)
        } else {
            0.0
        }
    }
}

/// Contours may own a note release; continuous controls never prolong a voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationBehavior {
    Contour,
    Continuous,
}
/// Absolute project position and prepared note origins. Negative origins retain trimmed-note phase.
#[derive(Clone, Copy, Debug)]
pub struct ModulationSourceContext {
    pub frame: u64,
    pub tempo_bpm: f32,
    pub note_origin: Option<i128>,
    pub legato_origin: Option<i128>,
}
impl Default for ModulationSourceContext {
    fn default() -> Self {
        Self {
            frame: 0,
            tempo_bpm: 120.0,
            note_origin: None,
            legato_origin: None,
        }
    }
}

/// One normalized control output, independent of the unit's audio ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModulationOutputDescriptor {
    pub name: &'static str,
    pub range: ModulationRange,
    pub behavior: ModulationBehavior,
}
pub const ENVELOPE_MODULATION_OUTPUT: ModulationOutputDescriptor = ModulationOutputDescriptor {
    name: "Envelope",
    range: ModulationRange::Unipolar,
    behavior: ModulationBehavior::Contour,
};

/// Implement this on every control generator. Hosts advance it once per sample, then fan out
/// the cached `ModulationValue`; destinations never call the source themselves.
pub trait ModulationSource: Send {
    const MODULATION_OUTPUT: ModulationOutputDescriptor;
    fn modulation_output(&self) -> ModulationOutputDescriptor {
        Self::MODULATION_OUTPUT
    }
    fn next_modulation(&mut self, context: ModulationSourceContext) -> f32;
    /// Normalized contribution depth after sampling. Keep zero neutral for every destination,
    /// including multiplicative Amplitude, rather than changing the signal's polarity range.
    fn modulation_weight(&self) -> f32 {
        1.0
    }
}

/// Control-thread discovery. Retained, absent units remain discoverable for reference validation.
pub trait ModulationSourceSettings {
    const MODULATION_OUTPUT: ModulationOutputDescriptor;
    /// Optional instance name for discovery, independent of routing identity.
    fn modulation_name(&self) -> Option<&str> {
        None
    }
    fn modulation_output(&self) -> ModulationOutputDescriptor {
        Self::MODULATION_OUTPUT
    }
    fn modulation_source_id(&self) -> Uuid;
    fn modulation_scope(&self) -> ModulationScope;
    fn modulation_present(&self) -> bool;
    fn modulation_enabled(&self) -> bool;
    fn modulation_source<'a>(
        &'a self,
        layer: Uuid,
        layer_name: &'a str,
        index: usize,
    ) -> ModulationSourceInfo<'a> {
        ModulationSourceInfo {
            id: ModulationSourceId {
                layer,
                unit: self.modulation_source_id(),
            },
            layer_name,
            name: self.modulation_name(),
            index,
            output: self.modulation_output(),
            scope: self.modulation_scope(),
            present: self.modulation_present(),
            enabled: self.modulation_enabled(),
        }
    }
}
impl ModulationSourceSettings for EnvelopeInstance {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = ENVELOPE_MODULATION_OUTPUT;
    fn modulation_source_id(&self) -> Uuid {
        self.id
    }
    fn modulation_scope(&self) -> ModulationScope {
        self.mode.into()
    }
    fn modulation_present(&self) -> bool {
        self.present
    }
    fn modulation_enabled(&self) -> bool {
        self.enabled
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ModulationSourceInfo<'a> {
    pub id: ModulationSourceId,
    pub layer_name: &'a str,
    pub name: Option<&'a str>,
    /// Zero-based ordinal in this provider's saved instance list.
    pub index: usize,
    pub output: ModulationOutputDescriptor,
    pub scope: ModulationScope,
    pub present: bool,
    pub enabled: bool,
}
impl ModulationSourceInfo<'_> {
    /// Use the saved instance name, or the provider type and list ordinal.
    pub fn display_name(&self) -> std::borrow::Cow<'_, str> {
        self.name.map_or_else(
            || std::borrow::Cow::Owned(format!("{} {}", self.output.name, self.index + 1)),
            std::borrow::Cow::Borrowed,
        )
    }
}

/// Common source enable ramp, normalization and published sample construction.
/// Identity and scope belong to the host, so the same generator can serve voices or a shared bus.
pub struct ModulationOutput {
    id: ModulationSourceId,
    weight: ModulationRamp,
    frames: u32,
}
impl ModulationOutput {
    pub fn new(id: ModulationSourceId, enabled: bool, context: ProcessingContext) -> Self {
        Self {
            id,
            weight: ModulationRamp::new(f32::from(enabled)),
            frames: (context.sample_rate_hz / 200).max(1),
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        self.weight.set(next.weight.target(), self.frames);
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.weight.set(f32::from(enabled), self.frames);
    }
    pub fn reset(&mut self) {
        self.weight.reset();
    }
    pub fn sample<S: ModulationSource>(
        &mut self,
        source: &mut S,
        context: ModulationSourceContext,
    ) -> ModulationValue {
        let descriptor = source.modulation_output();
        let value = descriptor.range.sanitize(source.next_modulation(context));
        let depth = ModulationRange::Unipolar.sanitize(source.modulation_weight());
        ModulationValue {
            range: descriptor.range,
            behavior: descriptor.behavior,
            source: self.id,
            value,
            weight: self.weight.next_value() * depth,
            release_age: None,
        }
    }
}

/// Destination timing and note lifecycle supplied by the host. No transport or device dependency.
#[derive(Clone, Copy, Debug)]
pub struct ModulationContext<'a> {
    pub local_sources: Option<&'a super::ModulationSourceIndex>,
    pub shared_sources: Option<&'a super::ModulationSourceIndex>,
    pub frame: u64,
    pub tempo_bpm: f32,
    pub note_gate: f32,
    pub release_age: Option<u64>,
}
impl Default for ModulationContext<'_> {
    fn default() -> Self {
        Self {
            frame: 0,
            tempo_bpm: 120.0,
            local_sources: None,
            shared_sources: None,
            note_gate: 1.0,
            release_age: None,
        }
    }
}

/// All destinations expose their typed targets and consume the same resolved input frame.
/// Audio remains unit-specific (mono or stereo). Bounds and unit-specific DSP stay in the unit.
pub trait ModulationDestination: Send {
    type Audio;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        None
    }
    const MODULATION_PARAMETERS: &'static [ModulationParameter];
    const MODULATION_SCOPE: ModulationScope;
    fn modulation_input_scope(&self) -> ModulationScope {
        Self::MODULATION_SCOPE
    }
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        context: ModulationContext,
    ) -> Self::Audio;
}

/// Saved destinations share route access, allowed targets and validation with portable DSP.
pub trait ModulationDestinationSettings {
    const MODULATION_PARAMETERS: &'static [ModulationParameter];
    const MODULATION_SCOPE: ModulationScope;
    fn modulation_input_scope(&self) -> ModulationScope {
        Self::MODULATION_SCOPE
    }
    fn modulations(&self) -> &[Modulation];
    fn valid_modulations(&self) -> bool {
        valid_modulations(self.modulations(), Self::MODULATION_PARAMETERS)
    }
}
macro_rules! destination_settings {
    ($settings:ty, $parameters:expr, $scope:ident) => {
        impl ModulationDestinationSettings for $settings {
            const MODULATION_PARAMETERS: &'static [ModulationParameter] = $parameters;
            const MODULATION_SCOPE: ModulationScope = ModulationScope::$scope;
            fn modulations(&self) -> &[Modulation] {
                &self.modulations
            }
        }
    };
}
destination_settings!(
    crate::AmplitudeSettings,
    &[ModulationParameter::Amplitude],
    PerNote
);
destination_settings!(
    crate::AutoFilterSettings,
    &ModulationParameter::FILTER,
    Shared
);
destination_settings!(crate::DriveSettings, &ModulationParameter::DRIVE, Shared);
destination_settings!(crate::EqSettings, &ModulationParameter::EQUALIZER, Shared);

impl UnitDescriptor {
    pub fn provides_modulation(&self) -> bool {
        self.outputs.contains(&PortKind::Modulation)
    }
    pub fn consumes_modulation(&self) -> bool {
        self.inputs.contains(&PortKind::Modulation)
    }
}

impl ModulationDestinationSettings for crate::InstrumentLayerSettings {
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = &ModulationParameter::OSCILLATOR;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn modulations(&self) -> &[Modulation] {
        &self.oscillator_modulations
    }
    fn valid_modulations(&self) -> bool {
        valid_targeted_modulations(
            self.modulations(),
            Self::MODULATION_PARAMETERS,
            &self.oscillators.iter().map(|s| s.id).collect::<Vec<_>>(),
        )
    }
}

destination_settings!(
    crate::CompressorSettings,
    &ModulationParameter::COMPRESSOR,
    Shared
);

destination_settings!(
    crate::SamplerSettings,
    &[ModulationParameter::SamplerGain],
    PerNote
);

destination_settings!(
    crate::LimiterSettings,
    &ModulationParameter::LIMITER,
    Shared
);

destination_settings!(crate::OttSettings, &ModulationParameter::OTT, Shared);

destination_settings!(
    crate::DirectionMixerSettings,
    &ModulationParameter::DIRECTION_MIXER,
    Shared
);

destination_settings!(
    crate::StereoSpreadSettings,
    &ModulationParameter::STEREO_SPREAD,
    Shared
);

destination_settings!(
    crate::BitcrusherSettings,
    &ModulationParameter::BITCRUSHER,
    Shared
);

impl ModulationDestinationSettings for EnvelopeInstance {
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = &ModulationParameter::ENVELOPE;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn modulation_input_scope(&self) -> ModulationScope {
        self.mode.into()
    }
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
}
