//! Unit-side parameter translation, metering and tail reporting.
use daw_dsp_support::reverb as dsp;
use daw_unit_core::{BandMeter, LevelMonitor, ProcessingContext};
use std::sync::{Arc, atomic::AtomicU32};

#[derive(Clone, Copy)]
pub struct Parameters {
    /// Decay seconds, predelay ms, HP Hz, LP Hz, Drive dB, Bloom, Tone,
    /// Motion, Width, Level dB, Mix, power.
    pub values: [f32; 12],
    pub hq: bool,
    pub gated: bool,
    pub dark: bool,
    pub drive_enabled: bool,
    pub drive_mode: daw_unit_core::PlateDriveMode,
    pub highpass_enabled: bool,
    pub lowpass_enabled: bool,
    pub highpass_slope: daw_unit_core::ReverbFilterSlope,
    pub lowpass_slope: daw_unit_core::ReverbFilterSlope,
}
impl Parameters {
    fn dsp(self) -> dsp::Parameters {
        dsp::Parameters {
            values: self.values,
            hq: self.hq,
            gated: self.gated,
            dark: self.dark,
            drive_enabled: self.drive_enabled,
            drive_mode: self.drive_mode.into(),
            highpass_enabled: self.highpass_enabled,
            lowpass_enabled: self.lowpass_enabled,
            highpass_slope: self.highpass_slope.into(),
            lowpass_slope: self.lowpass_slope.into(),
        }
    }

    fn tail_frames(self, rate: u32, plate: bool) -> u64 {
        if self.values[11] == 0.0 {
            return 0;
        }
        let seconds = match (plate, self.gated) {
            (true, true) => 2,
            (true, false) => daw_unit_core::PlateReverbSettings::MAX_TAIL_SECONDS,
            (false, _) => daw_unit_core::AmbientReverbSettings::MAX_TAIL_SECONDS,
        };
        u64::from(rate) * u64::from(seconds)
    }
}

#[derive(Clone)]
pub struct Reverb {
    kernel: dsp::Reverb,
    monitor: LevelMonitor<3>,
    rate: u32,
    plate: bool,
    tail: u64,
}

impl Reverb {
    pub fn new(context: ProcessingContext, p: Parameters, plate: bool) -> Self {
        let rate = context.sample_rate_hz;
        Self {
            kernel: dsp::Reverb::new(rate, p.dsp(), plate),
            monitor: LevelMonitor::new(rate, p.values[11] != 0.0),
            rate,
            plate,
            tail: p.tail_frames(rate, plate),
        }
    }

    pub fn retarget(&mut self, p: Parameters) {
        self.kernel.retarget(p.dsp());
        self.monitor.set_enabled(p.values[11] != 0.0);
        self.tail = p.tail_frames(self.rate, self.plate);
    }

    pub fn tail_frames(&self) -> u64 {
        self.tail
    }

    pub fn heap_bytes(&self) -> u64 {
        self.kernel.heap_bytes()
    }

    pub fn set_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }

    pub fn set_output_peak_meter(&mut self, meter: Arc<BandMeter<3>>) {
        self.monitor.attach_peak_prepared(meter);
    }

    pub fn effective(&self) -> [f32; 12] {
        self.kernel.effective()
    }

    pub fn reset(&mut self) {
        self.kernel.reset();
        self.monitor.reset();
    }

    pub fn sample(&mut self, input: [f32; 2], offsets: [f64; 12]) -> [f32; 2] {
        let output = self.kernel.sample(input, offsets);
        self.monitor.sample_output(output);
        output
    }
}
