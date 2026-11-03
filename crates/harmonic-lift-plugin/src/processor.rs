use crate::Dsp as Drive;
use crate::{Parameter, ParameterError, PluginState};
use daw_unit_core::{EffectUnit, ProcessingContext};

pub const SUPPORTED_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurationError;
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unsupported effect rate or block bounds")
    }
}
impl std::error::Error for ConfigurationError {}

/// Prepared outside processing. Only the portable effect owns audio DSP.
pub struct Processor {
    drive: Drive,
    state: PluginState,
    settings: crate::mapping::Settings,
    frame: u64,
    tempo: f32,
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
            settings: state.to_settings(),
            frame: 0,
            tempo: 120.0,
            state,
        })
    }
    pub fn set_output_meter(&mut self, meter: std::sync::Arc<crate::OutputMeter>) {
        self.drive.set_output_peak_meter(meter.output.clone());
        self.drive
            .set_gain_reduction_peak_meter(meter.reduction.clone());
    }
    pub fn state(&self) -> PluginState {
        self.state
    }
    /// Copy only the changed parameter from an already validated snapshot.
    /// Timed automation must not copy the complete state on every sample.
    pub fn apply_parameter(&mut self, parameter: Parameter, state: &PluginState) {
        self.state.copy_parameter_from(parameter, state);
        crate::mapping::update_settings(&self.state, &mut self.settings);
        self.drive.retarget(&self.settings);
    }
    pub fn restore(&mut self, state: PluginState) {
        self.state = state;
        crate::mapping::update_settings(&self.state, &mut self.settings);
        self.drive.retarget(&self.settings);
    }
    pub fn set(&mut self, parameter: Parameter, value: f64) -> Result<(), ParameterError> {
        self.state.set(parameter, value)?;
        crate::mapping::update_settings(&self.state, &mut self.settings);
        self.drive.retarget(&self.settings);
        Ok(())
    }
    pub fn reset(&mut self) {
        self.frame = 0;
        self.drive.reset();
    }
    pub fn set_transport(&mut self, frame: Option<u64>, tempo: Option<f32>) {
        if let Some(frame) = frame {
            self.frame = frame;
        }
        if let Some(tempo) = tempo.filter(|t| t.is_finite() && *t > 0.0) {
            self.tempo = tempo;
        }
    }
    pub fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        let output = self.drive.process_sample(input);
        self.frame = self.frame.saturating_add(1);
        output
    }
}
