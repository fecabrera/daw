//! Portable contracts and saved settings shared by built-in audio units and their hosts.
use serde::{Deserialize, Serialize};

pub const OSCILLATOR_TYPE_ID: &str = "daw.oscillator";
pub const UNIT_STATE_VERSION: u32 = 1;

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
}
impl Default for OscillatorSettings {
    fn default() -> Self {
        Self {
            version: UNIT_STATE_VERSION,
            waveform: Waveform::Sine,
            transpose: 0,
            gain_db: -12.0,
        }
    }
}
impl OscillatorSettings {
    pub fn valid(&self) -> bool {
        self.version == UNIT_STATE_VERSION
            && (-48..=48).contains(&self.transpose)
            && self.gain_db.is_finite()
            && (-60.0..=12.0).contains(&self.gain_db)
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
