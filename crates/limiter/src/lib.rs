//! Stereo limiter with fixed host-compensated latency and allocation-free processing.
use daw_unit_core::{
    EffectUnit, LIMITER_TYPE_ID, LimiterMode, LimiterRouting, LimiterSettings, PeakMeter, PortKind,
    ProcessingContext, UnitDescriptor, UnitMeter,
};
use std::sync::{Arc, atomic::AtomicU32};
mod modulation;
mod true_peak;

#[derive(Clone, Copy, Default)]
struct Frame {
    dry: [f64; 2],
    wet: [f64; 2],
}
#[derive(Clone, Copy)]
struct Ramp {
    value: f64,
    target: f64,
    remaining: u32,
}
impl Ramp {
    fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn tick(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.value
    }
}
fn amplitude(db: f32) -> f64 {
    10_f64.powf(f64::from(db) / 20.0)
}
fn peak(input: [f64; 2]) -> f64 {
    input[0].abs().max(input[1].abs())
}
fn ms(input: [f64; 2]) -> [f64; 2] {
    [(input[0] + input[1]) * 0.5, (input[0] - input[1]) * 0.5]
}
fn maximum(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i].max(b[i]))
}

#[derive(Clone)]
pub struct Limiter {
    rate: u32,
    frames: Vec<Frame>,
    peaks: Vec<[f64; 3]>,
    tree_size: usize,
    index: usize,
    // Input, release milliseconds, lookahead milliseconds, ceiling, M/S weight, enabled.
    parameters: [Ramp; 6],
    external: daw_unit_core::ModulationSmoother<4>,
    effective: [f64; 6],
    gains: [f64; 3],
    timing: (usize, f64),
    coefficients: [f64; 2],
    true_peak: bool,
    guard: true_peak::Guard,
    reduction_db: f32,
    meter: UnitMeter<PeakMeter>,
}
impl Limiter {
    /// Fixed maximum lookahead plus the reconstruction guard block, even when bypassed.
    pub fn latency_for_rate(rate: u32) -> u32 {
        rate.div_ceil(100) + true_peak::BLOCK as u32
    }
    pub fn latency_frames(&self) -> u32 {
        Self::latency_for_rate(self.rate)
    }
    pub fn new(context: ProcessingContext, settings: &LimiterSettings) -> Self {
        let length = context.sample_rate_hz.div_ceil(100) as usize + 1;
        let tree_size = length.next_power_of_two();
        Self {
            rate: context.sample_rate_hz,
            frames: vec![Frame::default(); length],
            peaks: vec![[0.0; 3]; tree_size * 2],
            tree_size,
            index: 0,
            parameters: Self::values(settings).map(Ramp::new),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: Self::values(settings),
            gains: [1.0; 3],
            timing: (usize::MAX, f64::NAN),
            coefficients: [0.0; 2],
            true_peak: settings.mode == LimiterMode::TruePeak,
            guard: true_peak::Guard::new(),
            reduction_db: 0.0,
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
        for (parameter, value) in self.parameters.iter_mut().zip(Self::values(settings)) {
            parameter.retarget(value, (self.rate / 200).max(1));
        }
        self.true_peak = settings.mode == LimiterMode::TruePeak;
    }
    pub fn set_gain_reduction_meter(&mut self, meter: Arc<AtomicU32>) {
        self.set_gain_reduction_peak_meter(Arc::new(PeakMeter::new(meter)));
    }
    /// Shared instantaneous/held monitoring, attached before callback publication.
    pub fn set_gain_reduction_peak_meter(&mut self, meter: Arc<PeakMeter>) {
        self.meter.attach(meter, self.reduction_db);
    }
    pub fn gain_reduction_db(&self) -> f32 {
        self.reduction_db
    }
    fn publish(&self) {
        self.meter.publish(self.reduction_db);
    }
    fn update_peak(&mut self, index: usize, value: [f64; 3]) {
        let mut node = self.tree_size + index;
        self.peaks[node] = value;
        while node > 1 {
            node /= 2;
            self.peaks[node] = maximum(self.peaks[node * 2], self.peaks[node * 2 + 1]);
        }
    }
    fn range_peak(&self, start: usize, end: usize) -> [f64; 3] {
        let mut result = [0.0; 3];
        let (mut left, mut right) = (start + self.tree_size, end + self.tree_size);
        while left < right {
            if left & 1 != 0 {
                result = maximum(result, self.peaks[left]);
                left += 1;
            }
            if right & 1 != 0 {
                right -= 1;
                result = maximum(result, self.peaks[right]);
            }
            left /= 2;
            right /= 2;
        }
        result
    }
}
impl EffectUnit for Limiter {
    fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.frames)
            + daw_unit_core::memory::vec_bytes(&self.peaks)
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
        self.frames.fill(Frame::default());
        self.peaks.fill([0.0; 3]);
        self.index = 0;
        for p in &mut self.parameters {
            p.value = p.target;
            p.remaining = 0;
        }
        self.external.reset();
        self.effective = self.parameters.each_ref().map(|p| p.value);
        self.gains = [1.0; 3];
        self.guard.reset();
        self.reduction_db = 0.0;
        self.publish();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 4])
    }
}
impl Limiter {
    fn process_effective_sample(&mut self, input: [f32; 2], p: [f64; 6]) -> [f32; 2] {
        let dry = input.map(|v| if v.is_finite() { f64::from(v) } else { 0.0 });
        let wet = dry.map(|v| v * p[0]);
        self.frames[self.index] = Frame { dry, wet };
        let mid_side = ms(wet);
        self.update_peak(
            self.index,
            [peak(wet), mid_side[0].abs(), mid_side[1].abs()],
        );
        self.index = (self.index + 1) % self.frames.len();
        let frame = self.frames[self.index];
        let lookahead = (p[2] * f64::from(self.rate) * 0.001).round() as usize;
        let end = self.index + lookahead.min(self.frames.len() - 1) + 1;
        let detected = if end <= self.frames.len() {
            self.range_peak(self.index, end)
        } else {
            maximum(
                self.range_peak(self.index, self.frames.len()),
                self.range_peak(0, end - self.frames.len()),
            )
        };
        let ceiling = p[3] * (1.0 - 1e-7);
        if self.timing.0 != lookahead {
            self.coefficients[0] = if lookahead == 0 {
                0.0
            } else {
                (-3.0 / lookahead as f64).exp()
            };
        }
        if self.timing.1 != p[1] {
            self.coefficients[1] = (-1.0 / (p[1] * 0.001 * f64::from(self.rate))).exp();
        }
        self.timing = (lookahead, p[1]);
        let [attack, release] = self.coefficients;
        for (gain, detected) in self.gains.iter_mut().zip(detected) {
            let target = (ceiling / detected.max(1e-30)).min(1.0);
            let coefficient = if target < *gain { attack } else { release };
            *gain = target + coefficient * (*gain - target);
        }
        let lr = frame.wet.map(|v| v * self.gains[0]);
        let [mid, side] = ms(frame.wet);
        let (mid, side) = (mid * self.gains[1], side * self.gains[2]);
        let ms = [mid + side, mid - side];
        let mut limited = std::array::from_fn(|ch| lr[ch] * (1.0 - p[4]) + ms[ch] * p[4]);
        // Recombining M/S can exceed either component's ceiling. Link the final L/R guard.
        let safety = (ceiling / peak(limited).max(1e-30)).min(1.0);
        for value in &mut limited {
            *value *= safety;
        }
        let output = std::array::from_fn(|ch| frame.dry[ch] * (1.0 - p[5]) + limited[ch] * p[5]);
        let output = self
            .guard
            .tick(output, ceiling, self.true_peak && p[5] >= 1.0 - 1e-9);
        let reduction =
            (self.gains[0] * (1.0 - p[4]) + self.gains[1].min(self.gains[2]) * p[4]) * safety;
        self.reduction_db =
            ((-20.0 * reduction.max(1e-30).log10()) as f32 + self.guard.reduction_db) * p[5] as f32;
        self.publish();
        output.map(|v| v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod modulation_tests;
