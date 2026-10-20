//! Three-band upward/downward compression with private FunDSP crossovers and detectors.
use daw_unit_core::{
    BandMeter, CompressorDetection, EffectUnit, OTT_TYPE_ID, OttBandSettings, OttSettings,
    PortKind, ProcessingContext, UnitDescriptor, UnitMeter,
};
use fundsp::{
    audionode::AudioNode,
    dynamics::{Meter, MeterState},
    svf::{FixedSvf, HighpassMode, LowpassMode, SvfParams},
};
use std::sync::{Arc, atomic::AtomicU32};

#[cfg(test)]
use std::sync::atomic::Ordering;

const RMS: Meter = Meter::Rms(0.01);

#[derive(Clone)]
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
fn amplitude(db: f32) -> f64 {
    10_f64.powf(f64::from(db) / 20.0)
}
fn finite(value: f64) -> f32 {
    value.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
}

/// Two cascaded Butterworth sections on each branch: fourth-order Linkwitz–Riley.
#[derive(Clone)]
struct Split {
    low: [[FixedSvf<f64, LowpassMode<f64>>; 2]; 2],
    high: [[FixedSvf<f64, HighpassMode<f64>>; 2]; 2],
    frequency: f32,
}
impl Split {
    fn new(rate: u32, frequency: f32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(rate),
            cutoff: f64::from(frequency),
            q: std::f64::consts::FRAC_1_SQRT_2,
            gain: 1.0,
        };
        Self {
            low: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(LowpassMode::new(), &params))
            }),
            high: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(HighpassMode::new(), &params))
            }),
            frequency,
        }
    }
    fn configure(&mut self, frequency: f32) {
        if self.frequency != frequency {
            self.frequency = frequency;
            for channel in &mut self.low {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
            for channel in &mut self.high {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
        }
    }
    fn sample(&mut self, input: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let low = std::array::from_fn(|ch| {
            self.low[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        let high = std::array::from_fn(|ch| {
            self.high[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        (low, high)
    }
    fn reset(&mut self) {
        for channel in &mut self.low {
            for node in channel {
                node.reset();
            }
        }
        for channel in &mut self.high {
            for node in channel {
                node.reset();
            }
        }
    }
}
// Fixed 6 dB soft knees, upward 4:1 below −36 dBFS and downward 10:1 above −18 dBFS.
fn soft_over(over: f32) -> f32 {
    if over <= -3.0 {
        0.0
    } else if over < 3.0 {
        (over + 3.0).powi(2) / 12.0
    } else {
        over
    }
}
fn gain_curve(level_db: f32) -> f32 {
    (0.75 * soft_over(-36.0 - level_db)).min(24.0) - (0.9 * soft_over(level_db + 18.0)).min(60.0)
}
#[derive(Clone)]
struct Band {
    // Depth, input/output dB, log attack/release milliseconds, enabled.
    parameters: [Ramp; 6],
    effective: [f32; 6],
    rms: [MeterState; 2],
    gain_db: f64,
    configured_times: [f32; 2],
    coefficients: [f64; 2],
    rate: f64,
}
impl Band {
    fn values(s: OttBandSettings) -> [f32; 6] {
        [
            s.depth,
            s.input_db,
            s.output_db,
            s.attack_ms.ln(),
            s.release_ms.ln(),
            f32::from(s.enabled),
        ]
    }
    fn new(rate: u32, settings: OttBandSettings) -> Self {
        Self {
            parameters: Self::values(settings).map(Ramp::new),
            effective: Self::values(settings),
            rms: std::array::from_fn(|_| {
                let mut state = MeterState::new(RMS);
                state.set_sample_rate(RMS, f64::from(rate));
                state
            }),
            gain_db: 0.0,
            configured_times: [f32::NAN; 2],
            coefficients: [0.0; 2],
            rate: f64::from(rate),
        }
    }
    fn retarget(&mut self, settings: OttBandSettings, frames: u32) {
        for (p, target) in self.parameters.iter_mut().zip(Self::values(settings)) {
            p.retarget(target, frames);
        }
    }
    fn change_db(&self) -> f32 {
        self.gain_db as f32 * self.effective[0] * self.effective[5]
    }
    fn sample(&mut self, input: [f32; 2], rms_weight: f32, offsets: [f32; 5]) -> [f32; 2] {
        for p in &mut self.parameters {
            p.advance();
        }
        let mut p = self.parameters.each_ref().map(|p| p.value);
        p[0] = (p[0] + offsets[0]).clamp(0.0, 1.0);
        for i in 1..=2 {
            p[i] = (p[i] + offsets[i]).clamp(-24.0, 24.0);
        }
        p[3] = (p[3] + offsets[3] * std::f32::consts::LN_2).clamp(0.1_f32.ln(), 200_f32.ln());
        p[4] = (p[4] + offsets[4] * std::f32::consts::LN_2).clamp(10_f32.ln(), 2000_f32.ln());
        self.effective = p;
        for index in 0..2 {
            let time = p[index + 3];
            if self.configured_times[index] != time {
                self.configured_times[index] = time;
                self.coefficients[index] =
                    (-1.0 / (f64::from(time).exp() * 0.001 * self.rate)).exp();
            }
        }
        let input = input.map(|x| finite(f64::from(x) * amplitude(p[1])));
        let peak = input[0].abs().max(input[1].abs());
        let mut rms: f32 = 0.0;
        for (state, sample) in self.rms.iter_mut().zip(input) {
            state.tick(RMS, sample.clamp(-1e15, 1e15));
            let level = state.level(RMS);
            if level < 1e-15 {
                state.reset(RMS);
            } else {
                rms = rms.max(level);
            }
        }
        let detected = peak * (1.0 - rms_weight) + rms * rms_weight;
        // Silence generates no audio. Remove hidden boost when the detector is silent.
        let target = if detected < 1e-15 {
            0.0
        } else {
            f64::from(gain_curve(20.0 * detected.log10()))
        };
        let relaxing = target.abs() < self.gain_db.abs() && target * self.gain_db >= 0.0;
        let coefficient = self.coefficients[usize::from(relaxing)];
        self.gain_db = target + coefficient * (self.gain_db - target);
        let gain = amplitude(self.change_db() + p[2]);
        input.map(|x| finite(f64::from(x) * gain))
    }
    fn reset(&mut self) {
        for p in &mut self.parameters {
            p.reset();
        }
        self.effective = self.parameters.each_ref().map(|p| p.value);
        for state in &mut self.rms {
            state.reset(RMS);
        }
        self.gain_db = 0.0;
    }
}

#[derive(Clone)]
pub struct Ott {
    // Low log frequency, High log frequency, output amplitude, wet, RMS weight, enabled.
    parameters: [Ramp; 6],
    external: daw_unit_core::ModulationSmoother<19>,
    effective: [f32; 6],
    splits: [Split; 3],
    bands: [Band; 3],
    fade_frames: u32,
    meter: UnitMeter<BandMeter<3>>,
}
impl Ott {
    pub fn new(context: ProcessingContext, settings: &OttSettings) -> Self {
        let rate = context.sample_rate_hz;
        Self {
            parameters: Self::values(settings).map(Ramp::new),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: Self::values(settings),
            splits: [
                Split::new(rate, settings.low_frequency_hz),
                Split::new(rate, settings.high_frequency_hz),
                Split::new(rate, settings.high_frequency_hz),
            ],
            bands: settings.bands.map(|s| Band::new(rate, s)),
            fade_frames: (rate / 200).max(1),
            meter: UnitMeter::new(settings.enabled),
        }
    }
    fn values(s: &OttSettings) -> [f32; 6] {
        [
            s.low_frequency_hz.ln(),
            s.high_frequency_hz.ln(),
            amplitude(s.level_db) as f32,
            s.dry_wet,
            f32::from(s.detection == CompressorDetection::Rms),
            f32::from(s.enabled),
        ]
    }
    pub fn retarget(&mut self, settings: &OttSettings) {
        self.meter.set_enabled(settings.enabled);
        let values = Self::values(settings);
        let crossover_changed = self.parameters[..2]
            .iter()
            .zip(values)
            .any(|(p, target)| p.target != target);
        for (p, target) in self.parameters.iter_mut().zip(values) {
            p.retarget(target, self.fade_frames);
        }
        // Treat the ordered crossover pair as one parameter. Independent restart times
        // could make their intermediate values cross during rapid consecutive edits.
        if crossover_changed {
            for p in &mut self.parameters[..2] {
                p.remaining = self.fade_frames;
            }
        }
        for (band, settings) in self.bands.iter_mut().zip(settings.bands) {
            band.retarget(settings, self.fade_frames);
        }
    }
    pub fn gain_change_db(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.bands[i].change_db() * self.parameters[5].value)
    }
    /// Attach on the control thread. Positive values mean boost, negative values reduction.
    pub fn set_gain_change_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.set_gain_change_peak_meter(Arc::new(BandMeter::new(meter)));
    }
    /// Attach signed readings and independent reduction holds on a control thread.
    pub fn set_gain_change_peak_meter(&mut self, meter: Arc<BandMeter<3>>) {
        self.meter.attach(meter, self.gain_change_db());
    }
    fn publish(&self) {
        self.meter.publish(self.gain_change_db());
    }
}
impl EffectUnit for Ott {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: OTT_TYPE_ID,
            name: "OTT",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for p in &mut self.parameters {
            p.reset();
        }
        self.external.reset();
        self.effective = self.parameters.each_ref().map(|p| p.value);
        for split in &mut self.splits {
            split.reset();
        }
        for band in &mut self.bands {
            band.reset();
        }
        self.publish();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 19])
    }
}
impl Ott {
    fn process_effective_sample(&mut self, input: [f32; 2], offsets: [f32; 19]) -> [f32; 2] {
        let low = self.effective[0].exp();
        let high = self.effective[1].exp();
        self.splits[0].configure(low);
        self.splits[1].configure(high);
        self.splits[2].configure(high);
        let input = input.map(|x| if x.is_finite() { x } else { 0.0 });
        // Bound the filter input only for pathological floating-point headroom.
        let (low, rest) = self.splits[0].sample(input.map(|x| x.clamp(-1e15, 1e15)));
        let (mid, high) = self.splits[1].sample(rest);
        // Match the upper split's all-pass phase on the low branch.
        let (a, b) = self.splits[2].sample(low);
        let low = std::array::from_fn(|ch| a[ch] + b[ch]);
        let raw = [low, mid, high];
        let processed: [[f32; 2]; 3] = std::array::from_fn(|i| {
            self.bands[i].sample(
                raw[i],
                self.effective[4],
                std::array::from_fn(|j| offsets[2 + 5 * i + j]),
            )
        });
        let p = &self.effective;
        let output = std::array::from_fn(|ch| {
            let dry: f64 = raw.iter().map(|band| f64::from(band[ch])).sum();
            let wet: f64 = processed.iter().map(|band| f64::from(band[ch])).sum();
            let mix = (dry * (1.0 - f64::from(p[3])) + wet * f64::from(p[3])) * f64::from(p[2]);
            finite(f64::from(input[ch]) * (1.0 - f64::from(p[5])) + mix * f64::from(p[5]))
        });
        self.publish();
        output
    }
}
mod modulation;
#[cfg(test)]
mod modulation_tests;
#[cfg(test)]
mod tests;
