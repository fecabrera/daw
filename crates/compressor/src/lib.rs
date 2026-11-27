//! Linked stereo compressor and downward expander. FunDSP detector types remain private.
use daw_dsp_support::dynamics::DynamicsDetector;
use daw_unit_core::ramps::ExactRamp32 as Ramp;
use daw_unit_core::{
    BandMeter, CompressorDetection, CompressorSettings, DynamicsDirection, EffectUnit,
    LevelMonitor, PeakMeter, PortKind, ProcessingContext, UnitDescriptor, UnitMeter,
};
use std::sync::{Arc, atomic::AtomicU32};

#[cfg(test)]
use std::sync::atomic::Ordering;
mod modulation;
#[cfg(test)]
mod modulation_tests;

fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

#[cfg(test)]
use daw_unit_core::compressor_gain_reduction_db as reduction;

#[derive(Clone)]
pub struct Compressor {
    direction: DynamicsDirection,
    // Threshold, slope, knee, log attack/release time, RMS weight, level, wet, enabled, Curve.
    parameters: [Ramp; 10],
    fade_frames: u32,
    detector: DynamicsDetector,
    output_monitor: LevelMonitor<3>,
    curve_monitor: LevelMonitor<3>,
    history: daw_unit_core::LevelHistoryWriter,
    meter: UnitMeter<PeakMeter>,
    external: daw_unit_core::ModulationSmoother<8>,
    effective: [f32; 10],
}
impl Compressor {
    /// Prepare with validated settings on a control thread.
    pub fn new(context: ProcessingContext, settings: &CompressorSettings) -> Self {
        Self::with_direction(context, settings, DynamicsDirection::Compressor)
    }

    pub fn with_direction(
        context: ProcessingContext,
        settings: &CompressorSettings,
        direction: DynamicsDirection,
    ) -> Self {
        let mut unit = Self {
            direction,
            parameters: Self::values(settings).map(Ramp::new),
            fade_frames: (context.sample_rate_hz / 200).max(1),
            detector: DynamicsDetector::new(context.sample_rate_hz, direction.into()),
            output_monitor: LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            curve_monitor: LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            history: daw_unit_core::LevelHistoryWriter::new(
                context.sample_rate_hz,
                settings.enabled,
            ),
            meter: UnitMeter::new(settings.enabled),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: Self::values(settings),
        };
        unit.configure([unit.effective[3], unit.effective[4]]);
        unit
    }
    fn values(settings: &CompressorSettings) -> [f32; 10] {
        [
            settings.threshold_db,
            1.0 - settings.ratio.recip(),
            settings.knee_db,
            settings.attack_ms.ln(),
            settings.release_ms.ln(),
            f32::from(settings.detection == CompressorDetection::Rms),
            amplitude(settings.level_db),
            settings.dry_wet,
            f32::from(settings.enabled),
            settings.curve,
        ]
    }
    /// Keep detector/envelope history and unrelated in-flight ramps on compatible edits.
    pub fn retarget(&mut self, settings: &CompressorSettings) {
        self.meter.set_enabled(settings.enabled);
        self.output_monitor.set_enabled(settings.enabled);
        self.curve_monitor.set_enabled(settings.enabled);
        self.history.set_enabled(settings.enabled);
        for (parameter, target) in self.parameters.iter_mut().zip(Self::values(settings)) {
            parameter.retarget(target, self.fade_frames);
        }
    }
    /// Control monitor publication independently of the audio bypass ramp.
    pub fn set_monitoring_enabled(&mut self, enabled: bool) {
        self.meter.set_enabled(enabled);
        self.output_monitor.set_enabled(enabled);
        self.curve_monitor.set_enabled(enabled);
        self.history.set_enabled(enabled);
    }
    /// Optional lock-free monitoring handle. Attach on a control thread.
    pub fn set_gain_reduction_meter(&mut self, meter: Arc<AtomicU32>) {
        self.set_gain_reduction_peak_meter(Arc::new(PeakMeter::new(meter)));
    }
    /// Shared instantaneous/held monitoring, attached before callback publication.
    pub fn set_gain_reduction_peak_meter(&mut self, meter: Arc<PeakMeter>) {
        self.meter.attach(meter, self.gain_reduction_db());
    }
    /// Compression before the dry/wet blend, zero when bypassed.
    pub fn gain_reduction_db(&self) -> f32 {
        self.detector.reduction_db() * self.parameters[8].value()
    }
    fn configure(&mut self, times: [f32; 2]) {
        self.detector.configure(times);
    }
    /// Attach final-output current and held sample peaks on a control thread.
    pub fn set_output_peak_meter(&mut self, meter: Arc<BandMeter<3>>) {
        self.output_monitor.attach_peak_prepared(meter);
    }
    /// Prepared input-envelope monitor: linked detector amplitude in cell zero.
    /// Binding must not overwrite a handle owned by the active renderer.
    pub fn set_curve_meter(&mut self, meter: Arc<BandMeter<3>>) {
        if let Some(history) = meter.history() {
            self.history.attach_prepared(history.clone());
        }
        if let Some(reduction) = meter.curve_reduction() {
            self.meter.attach_prepared(reduction.clone());
        }
        self.curve_monitor.attach_peak_prepared(meter);
    }
    fn publish(&self) {
        self.meter.publish(self.gain_reduction_db());
    }
}
impl EffectUnit for Compressor {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: self.direction.type_id(),
            name: self.direction.name(),
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for parameter in &mut self.parameters {
            parameter.reset();
        }
        self.detector.reset();
        self.external.reset();
        self.effective = self.parameters.each_ref().map(|p| p.value());
        self.configure([self.effective[3], self.effective[4]]);
        self.output_monitor.reset();
        self.curve_monitor.reset();
        self.history.reset();
        self.publish();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 8])
    }
}
impl Compressor {
    fn process_effective_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        let (output, detected, peak) = self.detector.sample(input, &self.effective);
        self.publish();
        self.output_monitor.sample_output(output);
        self.curve_monitor.sample([detected, 0.0, 0.0]);
        self.history.sample(peak, self.gain_reduction_db());
        output
    }
}

#[cfg(test)]
mod tests;
