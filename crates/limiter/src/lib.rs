//! Stereo limiter with fixed host-compensated latency and allocation-free processing.
use daw_unit_core::ramps::LinearRamp64 as Ramp;
use daw_unit_core::{
    BandMeter, EffectUnit, LIMITER_TYPE_ID, LevelMonitor, LimiterMode, LimiterRouting,
    LimiterSettings, PeakMeter, PortKind, ProcessingContext, UnitDescriptor, UnitMeter,
};
use std::sync::{Arc, atomic::AtomicU32};
mod modulation;
use daw_dsp_support::limiter::{GUARD_FRAMES, LimiterKernel};

fn amplitude(db: f32) -> f64 {
    10_f64.powf(f64::from(db) / 20.0)
}
#[derive(Clone)]
pub struct Limiter {
    rate: u32,
    kernel: LimiterKernel,
    // Input, release milliseconds, lookahead milliseconds, ceiling, M/S weight, enabled.
    parameters: [Ramp; 6],
    external: daw_unit_core::ModulationSmoother<4>,
    effective: [f64; 6],
    output_monitor: LevelMonitor<3>,
    meter: UnitMeter<PeakMeter>,
}
impl Limiter {
    /// Fixed maximum lookahead plus the reconstruction guard block, even when bypassed.
    pub fn latency_for_rate(rate: u32) -> u32 {
        rate.div_ceil(100) + GUARD_FRAMES
    }
    pub fn latency_frames(&self) -> u32 {
        Self::latency_for_rate(self.rate)
    }
    pub fn new(context: ProcessingContext, settings: &LimiterSettings) -> Self {
        Self {
            rate: context.sample_rate_hz,
            kernel: LimiterKernel::new(
                context.sample_rate_hz,
                settings.mode == LimiterMode::TruePeak,
            ),
            parameters: Self::values(settings).map(Ramp::new),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: Self::values(settings),
            output_monitor: LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            meter: UnitMeter::new(settings.enabled),
        }
    }
    fn values(s: &LimiterSettings) -> [f64; 6] {
        [
            amplitude(s.input_db),
            f64::from(s.release_ms),
            f64::from(s.lookahead_ms),
            amplitude(s.ceiling_db),
            f64::from(s.routing == LimiterRouting::MidSide),
            f64::from(s.enabled),
        ]
    }
    pub fn retarget(&mut self, settings: &LimiterSettings) {
        self.meter.set_enabled(settings.enabled);
        self.output_monitor.set_enabled(settings.enabled);
        for (parameter, value) in self.parameters.iter_mut().zip(Self::values(settings)) {
            parameter.retarget(value, (self.rate / 200).max(1));
        }
        self.kernel
            .set_true_peak(settings.mode == LimiterMode::TruePeak);
    }
    pub fn set_gain_reduction_meter(&mut self, meter: Arc<AtomicU32>) {
        self.set_gain_reduction_peak_meter(Arc::new(PeakMeter::new(meter)));
    }
    /// Shared instantaneous/held monitoring, attached before callback publication.
    pub fn set_gain_reduction_peak_meter(&mut self, meter: Arc<PeakMeter>) {
        self.meter.attach(meter, self.kernel.reduction_db());
    }
    pub fn gain_reduction_db(&self) -> f32 {
        self.kernel.reduction_db()
    }
    /// Attach final-output current and held sample peaks on a control thread.
    pub fn set_output_peak_meter(&mut self, meter: Arc<BandMeter<3>>) {
        self.output_monitor.attach_peak_prepared(meter);
    }
    fn publish(&self) {
        self.meter.publish(self.kernel.reduction_db());
    }
}
impl EffectUnit for Limiter {
    fn heap_bytes(&self) -> u64 {
        self.kernel.heap_bytes()
    }

    fn latency_frames(&self) -> u32 {
        Limiter::latency_frames(self)
    }
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: LIMITER_TYPE_ID,
            name: "Limiter",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.kernel.reset();
        for p in &mut self.parameters {
            p.reset();
        }
        self.external.reset();
        self.effective = self.parameters.each_ref().map(|p| p.value());
        self.output_monitor.reset();
        self.publish();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 4])
    }
}
impl Limiter {
    fn process_effective_sample(&mut self, input: [f32; 2], p: [f64; 6]) -> [f32; 2] {
        let output = self.kernel.sample(input, p);
        self.publish();
        self.output_monitor.sample_output(output);
        output
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod modulation_tests;
