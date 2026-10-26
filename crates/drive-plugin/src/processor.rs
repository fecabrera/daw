use crate::{Parameter, ParameterError, PluginState};
use daw_drive::Drive;
use daw_unit_core::{EffectUnit, ProcessingContext};

pub const SUPPORTED_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurationError;
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unsupported Drive rate or block bounds")
    }
}
impl std::error::Error for ConfigurationError {}

/// Prepared outside processing. Only the original portable Drive owns audio DSP.
pub struct Processor {
    drive: Drive,
    state: PluginState,
}
impl Processor {
    pub fn new(
        rate: f64,
        minimum: u32,
        maximum: u32,
        state: PluginState,
    ) -> Result<Self, ConfigurationError> {
        if !SUPPORTED_RATES.iter().any(|r| f64::from(*r) == rate)
            || minimum == 0
            || minimum > maximum
            || maximum > i32::MAX as u32
        {
            return Err(ConfigurationError);
        }
        Ok(Self {
            drive: Drive::new(
                ProcessingContext {
                    sample_rate_hz: rate as u32,
                    max_block_frames: maximum,
                },
                &state.to_settings(),
            ),
            state,
        })
    }
    pub fn state(&self) -> PluginState {
        self.state
    }
    pub fn restore(&mut self, state: PluginState) {
        self.state = state;
        self.drive.retarget(&state.to_settings());
    }
    pub fn set(&mut self, parameter: Parameter, value: f64) -> Result<(), ParameterError> {
        self.state.set(parameter, value)?;
        self.drive.retarget(&self.state.to_settings());
        Ok(())
    }
    pub fn reset(&mut self) {
        self.drive.reset();
    }
    pub fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.drive.process_sample(input)
    }
}
