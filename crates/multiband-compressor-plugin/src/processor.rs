use crate::{Dsp, Parameter, ParameterError, PluginState};
use daw_unit_core::{EffectUnit, ProcessingContext};
pub const SUPPORTED_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurationError;
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unsupported rate or block bounds")
    }
}
impl std::error::Error for ConfigurationError {}
pub struct Processor {
    drive: Dsp,
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
        let slots: [_; 64] = std::array::from_fn(|i| crate::mapping::band(&state, i));
        let present: [_; 64] = std::array::from_fn(|i| crate::mapping::present(&state, i));
        let drive = Dsp::prepare_slots(
            ProcessingContext {
                sample_rate_hz: rate as u32,
                max_block_frames: maximum,
            },
            &slots,
            &present,
            state.get(Parameter::Bypass) == 0.0,
            state.get(Parameter::Level) as f32,
            state.get(Parameter::DryWet) as f32,
        );
        Ok(Self { drive, state })
    }
    pub fn set_output_meter(&mut self, meter: std::sync::Arc<crate::OutputMeter>) {
        self.drive.set_audition_gate(meter.audition_allowed.clone());
        self.drive.set_output_peak_meter(meter.output.clone());
        for slot in 0..64 {
            let id = uuid::Uuid::from_u128(slot as u128 + 1);
            self.drive.set_band_solo(id, meter.solos[slot].clone());
            self.drive
                .set_band_level_meter(id, meter.eq_bands[slot].clone());
            self.drive
                .set_band_reduction_meter(id, meter.reductions[slot].clone());
        }
    }
    pub fn state(&self) -> PluginState {
        self.state
    }
    pub fn apply_parameter(&mut self, p: Parameter, state: &PluginState) {
        self.state.copy_parameter_from(p, state);
        self.retarget(p);
    }
    pub fn set(&mut self, p: Parameter, v: f64) -> Result<(), ParameterError> {
        self.state.set(p, v)?;
        self.retarget(p);
        Ok(())
    }
    fn retarget(&mut self, p: Parameter) {
        if (1..=640).contains(&p.index()) {
            let slot = (p.index() - 1) / 10;
            self.drive.retarget_slot(
                slot,
                &crate::mapping::band(&self.state, slot),
                crate::mapping::present(&self.state, slot),
            );
        } else {
            self.drive.retarget_output(
                self.state.get(Parameter::Bypass) == 0.0,
                daw_unit_core::gain::to_db_f32(self.state.get(Parameter::Level)),
                self.state.get(Parameter::DryWet) as f32,
            );
        }
    }
    pub fn restore(&mut self, state: PluginState) {
        self.state = state;
        for slot in 0..64 {
            self.drive.retarget_slot(
                slot,
                &crate::mapping::band(&state, slot),
                crate::mapping::present(&state, slot),
            );
        }
        self.retarget(Parameter::Bypass);
    }
    pub fn reset(&mut self) {
        self.drive.reset();
    }
    pub fn set_transport_state(&mut self, _: Option<u64>, _: Option<f32>, _: Option<bool>) {}
    pub fn set_transport(&mut self, _: Option<u64>, _: Option<f32>) {}
    pub fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.drive.process_sample(input)
    }
}
