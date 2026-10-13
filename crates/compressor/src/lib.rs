//! Linked stereo feed-forward compressor. FunDSP detector types remain private.
use daw_unit_core::{
    COMPRESSOR_TYPE_ID, CompressorDetection, CompressorSettings, EffectUnit, PortKind,
    ProcessingContext, UnitDescriptor,
};
use fundsp::dynamics::{Meter, MeterState};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

const RMS: Meter = Meter::Rms(0.01);

struct Ramp {
    value: f32,
    target: f32,
    remaining: u32,
}
impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f32, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn advance(&mut self) {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / self.remaining as f32;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.value = self.target;
            }
        }
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}
fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

/// Soft-knee gain reduction in dB. The knee joins the two slopes continuously.
fn reduction(level_db: f32, threshold_db: f32, slope: f32, knee: f32) -> f32 {
    let over = level_db - threshold_db;
    if knee > 0.0 && over > -knee * 0.5 && over < knee * 0.5 {
        slope * (over + knee * 0.5).powi(2) / (2.0 * knee)
    } else {
        slope * over.max(0.0)
    }
}

pub struct Compressor {
    rms: [MeterState; 2],
    // Threshold, slope, knee, log attack/release time, RMS weight, level, wet, enabled.
    parameters: [Ramp; 9],
    rate: f64,
    fade_frames: u32,
    configured_times: [f32; 2],
    coefficients: [f64; 2],
    reduction_db: f64,
    meter: Option<Arc<AtomicU32>>,
}
impl Compressor {
    /// Prepare with validated settings on a control thread.
    pub fn new(context: ProcessingContext, settings: &CompressorSettings) -> Self {
        let rate = f64::from(context.sample_rate_hz);
        let rms = std::array::from_fn(|_| {
            let mut state = MeterState::new(RMS);
            state.set_sample_rate(RMS, rate);
            state
        });
        let mut unit = Self {
            rms,
            parameters: Self::values(settings).map(Ramp::new),
            rate,
            fade_frames: (context.sample_rate_hz / 200).max(1),
            configured_times: [f32::NAN; 2],
            coefficients: [0.0; 2],
            reduction_db: 0.0,
            meter: None,
        };
        unit.configure();
        unit
    }
    fn values(settings: &CompressorSettings) -> [f32; 9] {
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
        ]
    }
    /// Keep detector/envelope history and unrelated in-flight ramps on compatible edits.
    pub fn retarget(&mut self, settings: &CompressorSettings) {
        for (parameter, target) in self.parameters.iter_mut().zip(Self::values(settings)) {
            parameter.retarget(target, self.fade_frames);
        }
    }
    /// Optional lock-free monitoring handle. Attach on a control thread.
    pub fn set_gain_reduction_meter(&mut self, meter: Arc<AtomicU32>) {
        meter.store(self.gain_reduction_db().to_bits(), Ordering::Relaxed);
        self.meter = Some(meter);
    }
    /// Compression before the dry/wet blend, zero when bypassed.
    pub fn gain_reduction_db(&self) -> f32 {
        self.reduction_db as f32 * self.parameters[8].value
    }
    fn configure(&mut self) {
        for index in 0..2 {
            let time = self.parameters[index + 3].value;
            if time != self.configured_times[index] {
                self.coefficients[index] =
                    (-1.0 / (f64::from(time).exp() * 0.001 * self.rate)).exp();
                self.configured_times[index] = time;
            }
        }
    }
    fn publish(&self) {
        if let Some(meter) = &self.meter {
            meter.store(self.gain_reduction_db().to_bits(), Ordering::Relaxed);
        }
    }
}
impl EffectUnit for Compressor {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: COMPRESSOR_TYPE_ID,
            name: "Compressor",
            is_source: false,
            inputs: &[PortKind::StereoAudio],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for state in &mut self.rms {
            state.reset(RMS);
        }
        for parameter in &mut self.parameters {
            parameter.reset();
        }
        self.reduction_db = 0.0;
        self.configure();
        self.publish();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        for parameter in &mut self.parameters {
            parameter.advance();
        }
        self.configure();
        let input = input.map(|value| if value.is_finite() { value } else { 0.0 });
        let peak = input[0].abs().max(input[1].abs());
        let mut rms: f32 = 0.0;
        for (state, sample) in self.rms.iter_mut().zip(input) {
            // Bound only the detector input to prevent square overflow with extreme headroom.
            state.tick(RMS, sample.clamp(-1e15, 1e15));
            let level = state.level(RMS);
            if level < 1e-15 {
                state.reset(RMS);
            } else {
                rms = rms.max(level);
            }
        }
        let p = &self.parameters;
        let detected = peak * (1.0 - p[5].value) + rms * p[5].value;
        let level_db = 20.0 * detected.max(1e-20).log10();
        let target = f64::from(reduction(level_db, p[0].value, p[1].value, p[2].value));
        let coefficient = self.coefficients[usize::from(target < self.reduction_db)];
        self.reduction_db = target + coefficient * (self.reduction_db - target);
        if self.reduction_db < 1e-8 {
            self.reduction_db = 0.0;
        }
        let gain = f64::from(amplitude(-(self.reduction_db as f32)));
        let output = input.map(|dry| {
            let dry = f64::from(dry);
            let mix = f64::from(p[7].value);
            let enabled = f64::from(p[8].value);
            let wet = dry * ((1.0 - mix) + gain * mix) * f64::from(p[6].value);
            (dry * (1.0 - enabled) + wet * enabled).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
                as f32
        });
        self.publish();
        output
    }
}

#[cfg(test)]
mod tests;
