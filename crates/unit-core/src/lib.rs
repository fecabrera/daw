//! Portable contracts and saved settings shared by built-in audio units and their hosts.
use serde::{Deserialize, Serialize};

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

/// One track instrument containing independently configured oscillator sources.
/// Empty instruments render silence; hosts may add sources again without replacing the instrument.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OscillatorInstrumentSettings {
    pub version: u32,
    pub oscillators: Vec<OscillatorSettings>,
}
impl Default for OscillatorInstrumentSettings {
    fn default() -> Self {
        OscillatorSettings::default().into()
    }
}
impl From<OscillatorSettings> for OscillatorInstrumentSettings {
    fn from(oscillator: OscillatorSettings) -> Self {
        Self {
            version: UNIT_STATE_VERSION,
            oscillators: vec![oscillator],
        }
    }
}
impl OscillatorInstrumentSettings {
    pub fn valid(&self) -> bool {
        self.version == UNIT_STATE_VERSION && self.oscillators.iter().all(OscillatorSettings::valid)
    }
}
impl<'de> Deserialize<'de> for OscillatorInstrumentSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Select by presence of the collection. A malformed new instrument must not
        // fall back to a legacy oscillator and silently discard its source settings.
        #[derive(Deserialize)]
        struct Saved {
            version: u32,
            #[serde(default, deserialize_with = "collection")]
            oscillators: Option<Vec<OscillatorSettings>>,
            waveform: Option<Waveform>,
            transpose: Option<i8>,
            gain_db: Option<f32>,
            #[serde(default)]
            muted: bool,
        }
        fn collection<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Vec<OscillatorSettings>>, D::Error> {
            Vec::deserialize(deserializer).map(Some)
        }
        let saved = Saved::deserialize(deserializer)?;
        if let Some(oscillators) = saved.oscillators {
            Ok(Self {
                version: saved.version,
                oscillators,
            })
        } else {
            Ok(OscillatorSettings {
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
            }
            .into())
        }
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
