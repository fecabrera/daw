use crate::{Parameter, ParameterError};
use daw_unit_core::{DRIVE_V2_STATE_VERSION, DriveV2ColorShape, DriveV2Settings};
use std::{
    fmt,
    io::{Read, Write},
};

pub const STATE_VERSION: u32 = 1;
pub const MAX_STATE_BYTES: usize = 16 + 8 * Parameter::COUNT;
const MAGIC: &[u8; 8] = b"DAWDRV2!";

/// A validated, copyable snapshot. State I/O runs outside the audio callback.
/// There are no project IDs, modulation routes, paths, UI shells, or DSP histories.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PluginState {
    values: [f64; Parameter::COUNT],
}

#[derive(Debug)]
pub enum StateError {
    InvalidEnvelope,
    UnsupportedVersion,
    Parameter(ParameterError),
    Io(std::io::Error),
}
impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEnvelope => f.write_str("invalid Drive plugin state envelope"),
            Self::UnsupportedVersion => f.write_str("unsupported Drive plugin state version"),
            Self::Parameter(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for StateError {}
impl From<ParameterError> for StateError {
    fn from(error: ParameterError) -> Self {
        Self::Parameter(error)
    }
}
impl From<std::io::Error> for StateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl Default for PluginState {
    fn default() -> Self {
        Self {
            values: Parameter::ALL.map(|p| p.info().default),
        }
    }
}
impl PluginState {
    pub fn get(&self, parameter: Parameter) -> f64 {
        self.values[parameter.index()]
    }
    pub fn set(&mut self, parameter: Parameter, plain: f64) -> Result<(), ParameterError> {
        self.values[parameter.index()] = parameter.validate(plain)?;
        Ok(())
    }
    pub fn set_normalized(
        &mut self,
        parameter: Parameter,
        normalized: f64,
    ) -> Result<(), ParameterError> {
        self.set(parameter, parameter.denormalize(normalized)?)
    }
    /// Color Off only affects editor enablement. It never deletes retained values.
    pub fn control_enabled(&self, parameter: Parameter) -> bool {
        !matches!(
            parameter,
            Parameter::ColorShape | Parameter::ColorStrength | Parameter::ColorFrequency
        ) || self.get(Parameter::ColorEnabled) != 0.0
    }
    pub fn to_settings(&self) -> DriveV2Settings {
        let volume_db = daw_unit_core::gain::to_db_f32(self.get(Parameter::Volume));
        DriveV2Settings {
            enabled: self.get(Parameter::Bypass) == 0.0,
            level_db: volume_db,
            input_gain_db: daw_unit_core::gain::to_db_f32(self.get(Parameter::InputGain)),
            dry_wet: self.get(Parameter::DryWet) as f32,
            color_enabled: self.get(Parameter::ColorEnabled) != 0.0,
            clip: if self.get(Parameter::Clip) == 0.0 {
                daw_unit_core::DriveV2Clip::Soft
            } else {
                daw_unit_core::DriveV2Clip::Hard
            },
            gain: self.get(Parameter::Gain) as f32,
            tone: self.get(Parameter::Tone) as f32,
            color_shape: DriveV2ColorShape::ALL[self.get(Parameter::ColorShape) as usize],
            color_strength: self.get(Parameter::ColorStrength) as f32,
            color_frequency_hz: self.get(Parameter::ColorFrequency) as f32,
            ..DriveV2Settings::default()
        }
    }
    /// Import only base DSP settings; DAW presentation and routes remain DAW-owned.
    pub fn from_settings(settings: &DriveV2Settings) -> Result<Self, ParameterError> {
        if !settings.valid() {
            return Err(ParameterError);
        }
        let values = [
            f64::from(!settings.enabled),
            f64::from(settings.level_db),
            f64::from(settings.gain),
            f64::from(settings.tone),
            settings.color_shape.index() as f64,
            f64::from(settings.color_strength),
            f64::from(settings.color_frequency_hz),
            f64::from(settings.input_gain_db),
            f64::from(settings.dry_wet),
            f64::from(settings.clip == daw_unit_core::DriveV2Clip::Hard),
            f64::from(settings.color_enabled),
        ];
        let mut state = Self::default();
        for (parameter, value) in Parameter::ALL.into_iter().zip(values) {
            state.set(parameter, value)?;
        }
        Ok(state)
    }
    pub fn encode(&self) -> [u8; MAX_STATE_BYTES] {
        let mut bytes = [0; MAX_STATE_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&STATE_VERSION.to_le_bytes());
        bytes[12..16].copy_from_slice(&DRIVE_V2_STATE_VERSION.to_le_bytes());
        for (chunk, value) in bytes[16..].chunks_exact_mut(8).zip(self.values) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, StateError> {
        if bytes.len() != MAX_STATE_BYTES || &bytes[..8] != MAGIC {
            return Err(StateError::InvalidEnvelope);
        }
        if bytes[8..12] != STATE_VERSION.to_le_bytes()
            || bytes[12..16] != DRIVE_V2_STATE_VERSION.to_le_bytes()
        {
            return Err(StateError::UnsupportedVersion);
        }
        let mut state = Self::default();
        for (parameter, chunk) in Parameter::ALL.into_iter().zip(bytes[16..].chunks_exact(8)) {
            let mut word = [0; 8];
            word.copy_from_slice(chunk);
            state.set(parameter, f64::from_le_bytes(word))?;
        }
        Ok(state)
    }
    /// Validate completely before changing the caller's current snapshot.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        *self = Self::decode(bytes)?;
        Ok(())
    }
    /// Read at most MAX_STATE_BYTES + 1 bytes with bounded calls, including interruptions. Trailing data
    /// is rejected. Each format adapter supplies a reader bounded to its payload.
    pub fn read_from(reader: &mut impl Read) -> Result<Self, StateError> {
        let mut buffer = [0; MAX_STATE_BYTES + 1];
        let mut length = 0;
        for _ in 0..2 * (MAX_STATE_BYTES + 1) {
            match reader.read(&mut buffer[length..]) {
                Ok(0) => return Self::decode(&buffer[..length]),
                Ok(count) => {
                    length += count;
                    if length > MAX_STATE_BYTES {
                        return Err(StateError::InvalidEnvelope);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(StateError::InvalidEnvelope)
    }
    /// Complete short writes with bounded retries; zero writes are failures.
    pub fn write_to(&self, writer: &mut impl Write) -> Result<(), StateError> {
        let bytes = self.encode();
        let mut offset = 0;
        for _ in 0..2 * MAX_STATE_BYTES {
            match writer.write(&bytes[offset..]) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()),
                Ok(count) => {
                    offset += count;
                    if offset == MAX_STATE_BYTES {
                        return Ok(());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(StateError::InvalidEnvelope)
    }
}
