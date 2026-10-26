//! Portable contracts and saved settings shared by built-in audio units and their hosts.
pub mod gain;
pub mod memory;
mod meter;
pub use meter::{BandMeter, LevelMonitor, MeterStorage, PeakHold, PeakMeter, UnitMeter};
mod note_name;
pub use note_name::{natural_note, note_name};
mod audio_range;
pub use audio_range::{ClipLoop, ClipStretch};
mod granulizer;
pub use granulizer::{GrainWindow, GranulizerSettings};
mod sampler;
pub use sampler::{SamplerInput, SamplerMode, SamplerSettings, SamplerTrigger};
mod instrument_bus;
pub use instrument_bus::InstrumentBusSettings;
mod instrument_components;
mod instrument_copy;
pub use instrument_components::InstrumentComponent;
mod macro_control;
pub use macro_control::*;
mod lfo;
pub use lfo::*;
use serde::{Deserialize, Serialize};
mod auto_filter;
mod filter;
pub use filter::{FILTER_STATE_VERSION, FILTER_TYPE_ID, FilterSettings};
mod modulation;
pub use modulation::*;
mod saturator;
pub use saturator::*;
mod bitcrusher;
mod chorus;
pub use chorus::*;
mod compressor;
pub use bitcrusher::{BITCRUSHER_STATE_VERSION, BITCRUSHER_TYPE_ID, BitcrusherSettings};
mod stereo_spread;
pub use stereo_spread::{STEREO_SPREAD_STATE_VERSION, STEREO_SPREAD_TYPE_ID, StereoSpreadSettings};
mod direction_mixer;
pub use direction_mixer::{
    DIRECTION_MIXER_STATE_VERSION, DIRECTION_MIXER_TYPE_ID, DirectionMixerInput,
    DirectionMixerSettings,
};
mod limiter;
pub use limiter::{
    LIMITER_STATE_VERSION, LIMITER_TYPE_ID, LimiterMode, LimiterRouting, LimiterSettings,
};
mod ott;
pub use auto_filter::{
    AUTO_FILTER_STATE_VERSION, AUTO_FILTER_TYPE_ID, AutoFilterSettings, FilterShape, LfoRateMode,
    LfoWave, NoteRate,
};
pub use compressor::{
    COMPRESSOR_STATE_VERSION, COMPRESSOR_TYPE_ID, CompressorDetection, CompressorSettings,
};
pub use ott::{OTT_STATE_VERSION, OTT_TYPE_ID, OttBandSettings, OttSettings};
mod drive_v2;
pub use drive_v2::{
    DRIVE_V2_STATE_VERSION, DRIVE_V2_TYPE_ID, DriveV2Clip, DriveV2ColorShape, DriveV2Settings,
};
mod drive;
pub use drive::{
    DRIVE_MAX_FREQUENCY_HZ, DRIVE_MAX_VOLUME_DB, DRIVE_MIN_FREQUENCY_HZ, DRIVE_STATE_VERSION,
    DRIVE_TYPE_ID, DriveColorShape, DriveSettings,
};
mod effects;
pub use effects::{
    EQ_STATE_VERSION, EQ_TYPE_ID, EffectInstance, EffectSettings, EqBandSettings, EqSettings,
};
mod envelope;
pub use envelope::{
    CurveShape, EnvelopeSettings, EnvelopeStageSettings, EnvelopeTiming, MAX_ENVELOPE_TIME_MS,
};

pub const OSCILLATOR_TYPE_ID: &str = "daw.oscillator";
pub const UNIT_STATE_VERSION: u32 = 1;
/// Finite saved dB value whose f32 amplitude is exactly zero.
pub const SILENT_GAIN_DB: f32 = -1000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Waveform {
    #[default]
    Sine,
    Saw,
    Square,
    Triangle,
}
impl Waveform {
    pub const ALL: [Self; 4] = [Self::Sine, Self::Saw, Self::Square, Self::Triangle];
    pub fn label(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Saw => "Saw",
            Self::Square => "Square",
            Self::Triangle => "Triangle",
        }
    }
}

mod oscillator;
pub use oscillator::OscillatorSettings;

pub const INSTRUMENT_STATE_VERSION: u32 = 3;

/// A parallel layer with per-voice sources and a shared stereo effect bus.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InstrumentLayerSettings {
    /// Serial stereo bus processors after per-voice Amplitude, before layer mixing.
    pub effects: Vec<EffectInstance>,
    pub id: uuid::Uuid,
    pub name: String,
    /// Presentation only; missing entries follow the canonical component inventory.
    pub component_order: Vec<InstrumentComponent>,
    /// Saved compact presentation; does not change unit or audio settings.
    pub collapsed: bool,
    pub muted: bool,
    pub soloed: bool,
    pub gain_db: f32,
    pub pan: f32,
    pub oscillators: Vec<OscillatorSettings>,
    pub sampler: Option<SamplerSettings>,
    /// Unit enable states retain individual source switches and envelope settings.
    pub oscillator_enabled: bool,
    /// Gain after summing oscillator sources, before per-note Amplitude.
    pub oscillator_level_db: f32,
    /// Common transpose for all oscillator sources, in semitones.
    pub oscillator_pitch_semitones: f32,
    pub oscillator_collapsed: bool,
    pub oscillator_modulation_open: bool,
    pub oscillator_modulations: Vec<Modulation>,
    pub envelopes: Vec<EnvelopeInstance>,
    pub lfos: Vec<LfoInstance>,
    pub macro_module: MacroModuleSettings,
    pub amplitude: AmplitudeSettings,
}
impl Default for InstrumentLayerSettings {
    fn default() -> Self {
        Self {
            effects: Vec::new(),
            id: uuid::Uuid::new_v4(),
            name: "Layer 1".into(),
            component_order: Vec::new(),
            collapsed: false,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            pan: 0.0,
            oscillators: Vec::new(),
            sampler: None,
            oscillator_enabled: true,
            oscillator_level_db: 0.0,
            oscillator_pitch_semitones: 0.0,
            oscillator_collapsed: false,
            oscillator_modulation_open: false,
            oscillator_modulations: Vec::new(),
            envelopes: (1..=2)
                .map(|id| EnvelopeInstance {
                    id: uuid::Uuid::from_u128(id),
                    present: false,
                    ..Default::default()
                })
                .collect(),
            lfos: Vec::new(),
            macro_module: MacroModuleSettings::default(),
            amplitude: AmplitudeSettings::default(),
        }
    }
}
impl InstrumentLayerSettings {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.name.trim().is_empty()
            && self.gain_db.is_finite()
            && 10.0_f32.powf(self.gain_db / 20.0).is_finite()
            && (-1.0..=1.0).contains(&self.pan)
            && self
                .sampler
                .as_ref()
                .is_none_or(|s| s.valid() && self.oscillators.is_empty())
            && self.oscillator_pitch_semitones.is_finite()
            && (-48.0..=48.0).contains(&self.oscillator_pitch_semitones)
            && self.oscillator_level_db.is_finite()
            && (SILENT_GAIN_DB..=12.0).contains(&self.oscillator_level_db)
            && self.oscillators.iter().all(OscillatorSettings::valid)
            && self.effects.iter().all(|e| e.unit.valid())
            && self.amplitude.valid()
            && self.macro_module.valid()
            && self.valid_component_order()
            && ModulationDestinationSettings::valid_modulations(self)
            && {
                let mut ids = std::collections::HashSet::new();
                self.oscillators.iter().all(|s| ids.insert(s.id))
            }
            && {
                let mut ids = std::collections::HashSet::new();
                self.envelopes
                    .iter()
                    .all(|e| !e.id.is_nil() && ids.insert(e.id) && e.valid())
                    && self.lfos.iter().all(|l| ids.insert(l.id) && l.valid())
                    && self.macro_module.controls.iter().all(|c| ids.insert(c.id))
            }
    }
    /// Remove only this destination and its connections; surviving IDs never change.
    pub fn remove_oscillator(&mut self, id: uuid::Uuid) -> Option<OscillatorSettings> {
        let index = self.oscillators.iter().position(|s| s.id == id)?;
        let removed = self.oscillators.remove(index);
        self.oscillator_modulations.retain(|r| r.target != Some(id));
        Some(removed)
    }
    /// Compatibility constructor: the former amplitude envelope becomes an explicit connection.
    pub fn install_amplitude_envelope(&mut self) {
        if self.envelopes.is_empty() {
            self.envelopes.push(EnvelopeInstance::default());
        }
        let e = &mut self.envelopes[0];
        e.present = true;
        self.amplitude.present = true;
        if self.amplitude.modulations.is_empty() {
            let mut route = Modulation::new(ModulationParameter::Amplitude);
            route.id = uuid::Uuid::from_u128(1);
            route.source = Some(ModulationSourceId {
                layer: self.id,
                unit: e.id,
            });
            route.amount = 1.0;
            self.amplitude.modulations.push(route);
        }
    }
}

// Missing names in existing version-2 instruments follow their original row positions.
#[derive(Deserialize)]
struct SavedLayer {
    #[serde(default)]
    effects: Vec<EffectInstance>,
    id: uuid::Uuid,
    #[serde(default, deserialize_with = "saved_layer_name")]
    name: Option<String>,
    #[serde(default)]
    component_order: Vec<InstrumentComponent>,
    #[serde(default)]
    collapsed: bool,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    soloed: bool,
    #[serde(default)]
    gain_db: f32,
    #[serde(default)]
    pan: f32,
    #[serde(deserialize_with = "oscillator::saved_oscillators")]
    oscillators: Vec<OscillatorSettings>,
    #[serde(default)]
    sampler: Option<SamplerSettings>,
    #[serde(default = "default_unit_enabled")]
    oscillator_enabled: bool,
    #[serde(default)]
    oscillator_level_db: f32,
    #[serde(default)]
    oscillator_pitch_semitones: f32,
    #[serde(default = "default_unit_enabled")]
    envelope_enabled: bool,
    #[serde(default)]
    oscillator_collapsed: bool,
    #[serde(default)]
    oscillator_modulation_open: bool,
    #[serde(default)]
    oscillator_modulations: Vec<Modulation>,
    #[serde(default)]
    envelope_collapsed: bool,
    #[serde(default)]
    has_envelope: bool,
    #[serde(default)]
    envelope: EnvelopeSettings,
    #[serde(default)]
    has_filter_envelope: bool,
    #[serde(default)]
    filter_envelope: EnvelopeSettings,
    #[serde(default = "default_unit_enabled")]
    filter_envelope_enabled: bool,
    #[serde(default)]
    filter_envelope_collapsed: bool,
    #[serde(default, deserialize_with = "saved_envelopes")]
    envelopes: Option<Vec<EnvelopeInstance>>,
    #[serde(default)]
    lfos: Vec<LfoInstance>,
    #[serde(default)]
    macro_module: MacroModuleSettings,
    #[serde(default, deserialize_with = "saved_amplitude")]
    amplitude: Option<AmplitudeSettings>,
}
fn saved_envelopes<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<EnvelopeInstance>>, D::Error> {
    Vec::deserialize(d).map(Some)
}
fn saved_amplitude<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<AmplitudeSettings>, D::Error> {
    AmplitudeSettings::deserialize(d).map(Some)
}
fn saved_layer_name<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
fn default_unit_enabled() -> bool {
    true
}
impl SavedLayer {
    fn into_settings(self, index: usize) -> InstrumentLayerSettings {
        let legacy = self.envelopes.is_none();
        let mut layer = InstrumentLayerSettings {
            effects: self.effects,
            id: self.id,
            name: self.name.unwrap_or_else(|| format!("Layer {}", index + 1)),
            component_order: self.component_order,
            collapsed: self.collapsed,
            muted: self.muted,
            soloed: self.soloed,
            gain_db: self.gain_db,
            pan: self.pan,
            oscillators: self.oscillators,
            sampler: self.sampler,
            oscillator_enabled: self.oscillator_enabled,
            oscillator_level_db: self.oscillator_level_db,
            oscillator_pitch_semitones: self.oscillator_pitch_semitones,
            oscillator_collapsed: self.oscillator_collapsed,
            oscillator_modulation_open: self.oscillator_modulation_open,
            oscillator_modulations: self.oscillator_modulations,
            envelopes: self.envelopes.unwrap_or_else(|| {
                vec![
                    EnvelopeInstance {
                        id: uuid::Uuid::from_u128(1),
                        present: self.has_envelope,
                        enabled: self.envelope_enabled,
                        collapsed: self.envelope_collapsed,
                        settings: self.envelope,
                        ..Default::default()
                    },
                    EnvelopeInstance {
                        id: uuid::Uuid::from_u128(2),
                        present: self.has_filter_envelope,
                        enabled: self.filter_envelope_enabled,
                        collapsed: self.filter_envelope_collapsed,
                        settings: self.filter_envelope,
                        ..Default::default()
                    },
                ]
            }),
            lfos: self.lfos,
            macro_module: self.macro_module,
            amplitude: self.amplitude.unwrap_or_default(),
        };
        if legacy && self.has_envelope {
            layer.install_amplitude_envelope();
        }
        layer
    }
}
impl<'de> Deserialize<'de> for InstrumentLayerSettings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SavedLayer::deserialize(d).map(|layer| layer.into_settings(0))
    }
}

/// One track instrument, with parallel layers feeding one output bus.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InstrumentSettings {
    pub version: u32,
    pub layers: Vec<InstrumentLayerSettings>,
    pub main: InstrumentBusSettings,
}
/// Compatibility name for callers of the original oscillator-only instrument API.
pub type OscillatorInstrumentSettings = InstrumentSettings;
impl Default for InstrumentSettings {
    fn default() -> Self {
        Self {
            version: INSTRUMENT_STATE_VERSION,
            layers: vec![InstrumentLayerSettings::default()],
            main: InstrumentBusSettings::default(),
        }
    }
}
impl From<OscillatorSettings> for InstrumentSettings {
    fn from(oscillator: OscillatorSettings) -> Self {
        let mut instrument = Self::default();
        let layer = &mut instrument.layers[0];
        layer.oscillators.push(oscillator);
        layer.install_amplitude_envelope();
        instrument
    }
}
impl InstrumentSettings {
    /// All installed or retained providers, independent of their concrete DSP type.
    pub fn modulation_sources(&self) -> impl Iterator<Item = ModulationSourceInfo<'_>> {
        self.layers
            .iter()
            .flat_map(|layer| {
                layer
                    .envelopes
                    .iter()
                    .enumerate()
                    .map(|(index, provider)| {
                        provider.modulation_source(layer.id, &layer.name, index)
                    })
                    .chain(layer.lfos.iter().enumerate().map(|(index, provider)| {
                        provider.modulation_source(layer.id, &layer.name, index)
                    }))
                    .chain(layer.macro_module.sources(layer.id, &layer.name))
            })
            .chain(self.main.modulation_sources())
    }
    pub fn modulation_source(&self, id: ModulationSourceId) -> Option<ModulationSourceInfo<'_>> {
        self.modulation_sources().find(|source| source.id == id)
    }
    pub fn lfo_destinations(&self) -> impl Iterator<Item = (ModulationSourceId, &LfoInstance)> {
        self.layers
            .iter()
            .flat_map(|l| {
                l.lfos.iter().map(move |f| {
                    (
                        ModulationSourceId {
                            layer: l.id,
                            unit: f.id,
                        },
                        f,
                    )
                })
            })
            .chain(self.main.lfos.iter().map(|f| {
                (
                    ModulationSourceId {
                        layer: self.main.id,
                        unit: f.id,
                    },
                    f,
                )
            }))
    }
    pub fn macro_modules(&self) -> impl Iterator<Item = (uuid::Uuid, &MacroModuleSettings)> {
        self.layers
            .iter()
            .map(|l| (l.id, &l.macro_module))
            .chain(std::iter::once((self.main.id, &self.main.macro_module)))
    }
    pub fn envelope_destinations(
        &self,
    ) -> impl Iterator<Item = (ModulationSourceId, &EnvelopeInstance)> {
        self.layers
            .iter()
            .flat_map(|l| {
                l.envelopes.iter().map(move |e| {
                    (
                        ModulationSourceId {
                            layer: l.id,
                            unit: e.id,
                        },
                        e,
                    )
                })
            })
            .chain(self.main.envelopes.iter().map(|e| {
                (
                    ModulationSourceId {
                        layer: self.main.id,
                        unit: e.id,
                    },
                    e,
                )
            }))
    }
    pub fn provider_destinations(
        &self,
    ) -> impl Iterator<Item = (ModulationSourceId, &[Modulation])> {
        self.envelope_destinations()
            .map(|(id, e)| (id, e.modulations.as_slice()))
            .chain(
                self.lfo_destinations()
                    .map(|(id, l)| (id, l.modulations.as_slice()))
                    .chain(self.macro_modules().flat_map(|(layer, module)| {
                        module.controls.iter().map(move |c| {
                            (
                                ModulationSourceId { layer, unit: c.id },
                                module.modulations.as_slice(),
                            )
                        })
                    })),
            )
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        let dependencies: Vec<_> = self.provider_destinations().collect();
        self.version == INSTRUMENT_STATE_VERSION
            && modulation_dependency_order(&dependencies).is_some()
            && dependencies.iter().all(|(destination, routes)| {
                routes
                    .iter()
                    .filter(|r| r.target.is_none_or(|target| target == destination.unit))
                    .all(|r| {
                        r.source.is_none_or(|id| {
                            self.modulation_source(id).is_some_and(|s| {
                                self.modulation_source(*destination)
                                    .is_some_and(|d| d.scope.accepts(s.scope))
                            })
                        })
                    })
            })
            && self.main.valid()
            && ids.insert(self.main.id)
            && !self.layers.is_empty()
            && self
                .layers
                .iter()
                .all(|layer| ids.insert(layer.id) && layer.valid())
    }
}
impl<'de> Deserialize<'de> for InstrumentSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Saved {
            version: u32,
            #[serde(default, deserialize_with = "saved_main")]
            main: Option<InstrumentBusSettings>,
            #[serde(default, deserialize_with = "layers")]
            layers: Option<Vec<InstrumentLayerSettings>>,
            #[serde(default, deserialize_with = "sources")]
            oscillators: Option<Vec<OscillatorSettings>>,
            #[serde(default, deserialize_with = "oscillator::saved_id")]
            id: Option<uuid::Uuid>,
            waveform: Option<Waveform>,
            transpose: Option<i8>,
            gain_db: Option<f32>,
            #[serde(default)]
            muted: bool,
            #[serde(default = "EnvelopeSettings::legacy")]
            envelope: EnvelopeSettings,
        }
        fn saved_main<'de, D: serde::Deserializer<'de>>(
            d: D,
        ) -> Result<Option<InstrumentBusSettings>, D::Error> {
            InstrumentBusSettings::deserialize(d).map(Some)
        }
        // Explicit malformed collections must fail instead of selecting legacy fields.
        fn layers<'de, D: serde::Deserializer<'de>>(
            d: D,
        ) -> Result<Option<Vec<InstrumentLayerSettings>>, D::Error> {
            Vec::<SavedLayer>::deserialize(d).map(|layers| {
                Some(
                    layers
                        .into_iter()
                        .enumerate()
                        .map(|(index, layer)| layer.into_settings(index))
                        .collect(),
                )
            })
        }
        fn sources<'de, D: serde::Deserializer<'de>>(
            d: D,
        ) -> Result<Option<Vec<OscillatorSettings>>, D::Error> {
            oscillator::saved_oscillators(d).map(Some)
        }
        let saved = Saved::deserialize(deserializer)?;
        if let Some(layers) = saved.layers {
            let main = saved
                .main
                .unwrap_or_else(|| InstrumentBusSettings::legacy(&layers));
            return Ok(Self {
                version: if saved.version == 2 {
                    INSTRUMENT_STATE_VERSION
                } else {
                    saved.version
                },
                layers,
                main,
            });
        }
        if saved.version != UNIT_STATE_VERSION {
            return Err(serde::de::Error::custom(
                "Unsupported legacy instrument version",
            ));
        }
        let oscillators = if let Some(sources) = saved.oscillators {
            sources
        } else {
            vec![OscillatorSettings {
                id: saved.id.unwrap_or(uuid::Uuid::from_u128(1)),
                version: saved.version,
                waveform: saved
                    .waveform
                    .ok_or_else(|| serde::de::Error::missing_field("waveform"))?,
                transpose: saved
                    .transpose
                    .ok_or_else(|| serde::de::Error::missing_field("transpose"))?,
                gain_db: saved
                    .gain_db
                    .ok_or_else(|| serde::de::Error::missing_field("gain_db"))?,
                muted: saved.muted,
            }]
        };
        let mut layer = InstrumentLayerSettings {
            id: uuid::Uuid::from_u128(1),
            oscillators,
            ..Default::default()
        };
        layer.envelopes[0].settings = saved.envelope;
        layer.install_amplitude_envelope();
        let main = saved
            .main
            .unwrap_or_else(|| InstrumentBusSettings::legacy(std::slice::from_ref(&layer)));
        Ok(Self {
            version: INSTRUMENT_STATE_VERSION,
            layers: vec![layer],
            main,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortKind {
    MonoAudio,
    StereoAudio,
    Modulation,
    NoteEvents,
}
#[derive(Clone, Copy, Debug)]
pub struct UnitDescriptor {
    pub type_id: &'static str,
    pub name: &'static str,
    pub is_source: bool,
    pub inputs: &'static [PortKind],
    pub outputs: &'static [PortKind],
}
#[derive(Clone, Copy, Debug)]
pub struct ProcessingContext {
    pub sample_rate_hz: u32,
    pub max_block_frames: u32,
}

/// A prepared mono source. Processing and reset must not allocate, block, or use I/O.
/// Voices and note identity belong to the host, allowing reuse outside a DAW.
pub trait SourceUnit: Send {
    fn descriptor(&self) -> UnitDescriptor;
    fn reset(&mut self);
    fn set_frequency(&mut self, hz: f32);
    fn next_sample(&mut self) -> f32;
}

/// A prepared stereo source with the same callback constraints as mono sources.
/// Hosts own note scheduling; source-specific controls stay in the portable implementation.
pub trait StereoSourceUnit: Send {
    fn descriptor(&self) -> UnitDescriptor;
    fn reset(&mut self);
    fn next_sample(&mut self) -> [f32; 2];
}

/// A prepared stereo effect. All callback methods must be allocation-free and nonblocking.
pub trait EffectUnit: Send {
    /// Owned dynamic processor storage, excluding inline state and shared media.
    /// Hosts query this on a worker when budgeting renderer snapshots.
    fn heap_bytes(&self) -> u64 {
        0
    }

    /// Fixed processing delay. Hosts align parallel paths and prime this many frames.
    fn latency_frames(&self) -> u32 {
        0
    }
    fn descriptor(&self) -> UnitDescriptor;
    fn reset(&mut self);
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oscillator_gain_accepts_finite_silence_and_rejects_invalid_limits() {
        for gain_db in [SILENT_GAIN_DB, -90.0, -60.0, 0.0, 12.0] {
            assert!(
                OscillatorSettings {
                    gain_db,
                    ..Default::default()
                }
                .valid()
            );
        }
        for gain_db in [
            SILENT_GAIN_DB - 1.0,
            12.1,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert!(
                !OscillatorSettings {
                    gain_db,
                    ..Default::default()
                }
                .valid()
            );
        }
    }
}

#[cfg(test)]
mod oscillator_level_tests {
    use super::*;
    #[test]
    fn oscillator_level_round_trips_defaults_legacy_layers_to_unity_and_validates_bounds() {
        let mut layer = InstrumentLayerSettings::default();
        layer.oscillators.push(OscillatorSettings::default());
        for level in [SILENT_GAIN_DB, -12.0, 0.0, 12.0] {
            layer.oscillator_level_db = level;
            assert!(layer.valid());
            let saved = serde_json::to_value(&layer).unwrap();
            let reopened: InstrumentLayerSettings = serde_json::from_value(saved).unwrap();
            assert_eq!(reopened, layer);
        }
        let mut legacy = serde_json::to_value(&layer).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("oscillator_level_db");
        let reopened: InstrumentLayerSettings = serde_json::from_value(legacy).unwrap();
        assert_eq!(reopened.oscillator_level_db, 0.0);
        for level in [f32::NAN, f32::INFINITY, SILENT_GAIN_DB - 1.0, 12.1] {
            layer.oscillator_level_db = level;
            assert!(!layer.valid());
        }
    }
}

#[cfg(test)]
mod oscillator_level_modulation_tests {
    use super::*;
    use crate::ModulationDestinationSettings;
    #[test]
    fn level_routes_validate_roundtrip_copy_and_survive_source_removal() {
        let mut layer = InstrumentLayerSettings::default();
        layer.oscillators.push(OscillatorSettings::default());
        let mut level = Modulation::new(ModulationParameter::Level);
        level.amount = -24.0;
        layer.oscillator_modulations.push(level);
        assert!(layer.valid_modulations());
        let saved = serde_json::to_string(&layer).unwrap();
        let loaded: InstrumentLayerSettings = serde_json::from_str(&saved).unwrap();
        assert_eq!(layer, loaded);
        let copied = layer.duplicate();
        assert_ne!(copied.oscillator_modulations[0].id, level.id);
        assert_eq!(copied.oscillator_modulations[0].target, None);
        assert!(copied.valid_modulations());
        layer.oscillator_modulations[0].target = Some(layer.oscillators[0].id);
        assert!(!layer.valid_modulations());
        layer.oscillator_modulations[0].target = None;
        layer.oscillator_modulations[0].parameter = ModulationParameter::OscillatorGain;
        assert!(!layer.valid_modulations());
        layer.oscillator_modulations[0] = level;
        layer.oscillator_modulations.push(level);
        assert!(!layer.valid_modulations());
        layer.oscillator_modulations.pop();
        layer.oscillator_modulations[0].amount = 24.1;
        assert!(!layer.valid_modulations());
        layer.oscillator_modulations[0] = level;
        layer.remove_oscillator(layer.oscillators[0].id);
        assert_eq!(layer.oscillator_modulations, vec![level]);
    }
}

#[cfg(test)]
mod oscillator_output_pitch_tests {
    use super::*;
    #[test]
    fn output_pitch_roundtrips_defaults_and_coexists_with_source_pitch() {
        let mut layer = InstrumentLayerSettings::default();
        layer.oscillators.push(OscillatorSettings::default());
        for pitch in [-48.0, -0.25, 0.0, 12.0, 48.0] {
            layer.oscillator_pitch_semitones = pitch;
            assert!(layer.valid());
            let saved = serde_json::to_value(&layer).unwrap();
            assert_eq!(
                serde_json::from_value::<InstrumentLayerSettings>(saved).unwrap(),
                layer
            );
        }
        let mut saved = serde_json::to_value(&layer).unwrap();
        saved
            .as_object_mut()
            .unwrap()
            .remove("oscillator_pitch_semitones");
        assert_eq!(
            serde_json::from_value::<InstrumentLayerSettings>(saved)
                .unwrap()
                .oscillator_pitch_semitones,
            0.0
        );
        for pitch in [f32::NAN, f32::INFINITY, -48.1, 48.1] {
            layer.oscillator_pitch_semitones = pitch;
            assert!(!layer.valid());
        }
        layer.oscillator_pitch_semitones = 12.0;
        let unit = Modulation::new(ModulationParameter::Pitch);
        let mut source = Modulation::new(ModulationParameter::Pitch);
        source.target = Some(layer.oscillators[0].id);
        layer.oscillator_modulations = vec![unit, source];
        assert!(layer.valid());
        let copy = layer.duplicate();
        assert_eq!(copy.oscillator_pitch_semitones, 12.0);
        assert!(copy.valid());
        assert_ne!(copy.oscillator_modulations[0].id, unit.id);
        assert_eq!(copy.oscillator_modulations[0].target, None);
        assert_eq!(
            copy.oscillator_modulations[1].target,
            Some(copy.oscillators[0].id)
        );
        layer.remove_oscillator(layer.oscillators[0].id);
        assert_eq!(layer.oscillator_modulations, vec![unit]);
    }
}

mod multiband_eq;
pub use multiband_eq::*;
