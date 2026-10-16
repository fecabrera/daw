//! Portable contracts and saved settings shared by built-in audio units and their hosts.
mod instrument_bus;
pub use instrument_bus::InstrumentBusSettings;
mod lfo;
pub use lfo::*;
use serde::{Deserialize, Serialize};
mod auto_filter;
mod modulation;
pub use modulation::*;
mod compressor;
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
mod drive;
pub use drive::{DRIVE_STATE_VERSION, DRIVE_TYPE_ID, DriveColorShape, DriveSettings};
mod effects;
pub use effects::{
    EQ_STATE_VERSION, EQ_TYPE_ID, EffectInstance, EffectSettings, EqBandSettings, EqSettings,
};
mod envelope;
pub use envelope::{CurveShape, EnvelopeSettings, EnvelopeStageSettings, MAX_ENVELOPE_TIME_MS};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OscillatorSettings {
    pub version: u32,
    pub waveform: Waveform,
    /// Semitone transposition. Host MIDI pitch remains separate from source state.
    pub transpose: i8,
    pub gain_db: f32,
    #[serde(default)]
    pub muted: bool,
}
impl Default for OscillatorSettings {
    fn default() -> Self {
        Self {
            version: UNIT_STATE_VERSION,
            waveform: Waveform::Sine,
            transpose: 0,
            gain_db: -12.0,
            muted: false,
        }
    }
}
impl OscillatorSettings {
    pub fn valid(&self) -> bool {
        self.version == UNIT_STATE_VERSION
            && (-48..=48).contains(&self.transpose)
            && self.gain_db.is_finite()
            && (SILENT_GAIN_DB..=12.0).contains(&self.gain_db)
    }
}

pub const INSTRUMENT_STATE_VERSION: u32 = 3;

/// A parallel instrument layer. Settings are shared by notes; runtime state is per voice.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InstrumentLayerSettings {
    pub id: uuid::Uuid,
    pub name: String,
    /// Saved compact presentation; does not change unit or audio settings.
    pub collapsed: bool,
    pub muted: bool,
    pub soloed: bool,
    pub gain_db: f32,
    pub pan: f32,
    pub oscillators: Vec<OscillatorSettings>,
    /// Unit enable states retain individual source switches and envelope settings.
    pub oscillator_enabled: bool,
    pub oscillator_collapsed: bool,
    pub envelopes: Vec<EnvelopeInstance>,
    pub lfos: Vec<LfoInstance>,
    pub amplitude: AmplitudeSettings,
}
impl Default for InstrumentLayerSettings {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: "Layer 1".into(),
            collapsed: false,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            pan: 0.0,
            oscillators: Vec::new(),
            oscillator_enabled: true,
            oscillator_collapsed: false,
            envelopes: (1..=2)
                .map(|id| EnvelopeInstance {
                    id: uuid::Uuid::from_u128(id),
                    present: false,
                    ..Default::default()
                })
                .collect(),
            lfos: Vec::new(),
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
            && self.oscillators.iter().all(OscillatorSettings::valid)
            && self.amplitude.valid()
            && {
                let mut ids = std::collections::HashSet::new();
                self.envelopes
                    .iter()
                    .all(|e| !e.id.is_nil() && ids.insert(e.id) && e.settings.valid())
                    && self
                        .lfos
                        .iter()
                        .all(|l| !l.id.is_nil() && ids.insert(l.id) && l.settings.valid())
            }
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
    id: uuid::Uuid,
    #[serde(default, deserialize_with = "saved_layer_name")]
    name: Option<String>,
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
    oscillators: Vec<OscillatorSettings>,
    #[serde(default = "default_unit_enabled")]
    oscillator_enabled: bool,
    #[serde(default = "default_unit_enabled")]
    envelope_enabled: bool,
    #[serde(default)]
    oscillator_collapsed: bool,
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
            id: self.id,
            name: self.name.unwrap_or_else(|| format!("Layer {}", index + 1)),
            collapsed: self.collapsed,
            muted: self.muted,
            soloed: self.soloed,
            gain_db: self.gain_db,
            pan: self.pan,
            oscillators: self.oscillators,
            oscillator_enabled: self.oscillator_enabled,
            oscillator_collapsed: self.oscillator_collapsed,
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
            })
            .chain(self.main.modulation_sources())
    }
    pub fn modulation_source(&self, id: ModulationSourceId) -> Option<ModulationSourceInfo<'_>> {
        self.modulation_sources().find(|source| source.id == id)
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.version == INSTRUMENT_STATE_VERSION
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
            Vec::deserialize(d).map(Some)
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

/// A prepared stereo effect. All callback methods must be allocation-free and nonblocking.
pub trait EffectUnit: Send {
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
