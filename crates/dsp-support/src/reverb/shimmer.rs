//! Prepared Shimmer predelay, pitch voices, feedback tank, and output stages.
use super::{Delay, TapTransition};
use crate::ramps::ModulationRamp;
use delay::Diffuser;

pub mod delay;
pub mod pitch;
pub mod tank;
pub mod vowel;

// +15 dB calibrated from pink noise and chord/percussion fixtures. Apply after
// Warm and outside both regeneration loops.
const WET_MAKEUP: f64 = 5.623413251903491;

#[derive(Clone)]
pub struct ShimmerKernel {
    sample_rate_hz: u32,
    predelay: Delay,
    early: [Diffuser; 4],
    vowel: vowel::Vowel,
    pitch: [pitch::Pitch; 2],
    tank: tank::Tank,
    warm: [f64; 2],
    intervals: [i8; 2],
    counter: u64,
    warm_alpha: f64,
    predelay_transition: TapTransition<1>,
    injection: ModulationRamp,
}

fn injection_target([first, second]: [i8; 2]) -> f32 {
    if first == second { 0.5 } else { 1.0 }
}

impl ShimmerKernel {
    pub fn new(sample_rate_hz: u32, intervals: [i8; 2], amounts: [f32; 2]) -> Self {
        Self {
            sample_rate_hz,
            predelay: Delay::new((sample_rate_hz as usize) * 8),
            early: [0.0043, 0.0059, 0.0097, 0.0127].map(|t| Diffuser::new(sample_rate_hz, t)),
            vowel: vowel::Vowel::new(sample_rate_hz),
            pitch: std::array::from_fn(|i| {
                pitch::Pitch::new(sample_rate_hz, intervals[i], amounts[i])
            }),
            tank: tank::Tank::new(sample_rate_hz),
            warm: [0.0; 2],
            intervals,
            counter: 0,
            warm_alpha: 1.0 - (-std::f64::consts::TAU * 5000.0 / f64::from(sample_rate_hz)).exp(),
            predelay_transition: TapTransition::new(sample_rate_hz),
            injection: ModulationRamp::new(injection_target(intervals)),
        }
    }

    pub fn retarget_pitch(&mut self, intervals: [i8; 2], amounts: [f32; 2], frames: u32) {
        self.intervals = intervals;
        for ((voice, interval), amount) in self.pitch.iter_mut().zip(intervals).zip(amounts) {
            voice.retarget(interval, amount, frames);
        }
        self.injection.set(injection_target(intervals), frames);
    }

    pub fn heap_bytes(&self) -> u64 {
        self.predelay.heap_bytes() as u64
            + self.early.iter().map(Diffuser::heap_bytes).sum::<u64>()
            + self.pitch.iter().map(pitch::Pitch::heap_bytes).sum::<u64>()
            + self.tank.heap_bytes()
    }

    pub fn reset(&mut self, amounts: [f32; 2]) {
        self.predelay.reset();
        self.predelay_transition.reset();
        self.injection.reset();
        for d in &mut self.early {
            d.reset();
        }
        self.vowel.reset();
        for (i, p) in self.pitch.iter_mut().enumerate() {
            p.reset(amounts[i], self.intervals[i] != 0);
        }
        self.tank.reset();
        self.warm = [0.0; 2];
        self.counter = 0;
    }

    /// Process bounded controls in unit order. Level is linear and predelay is in ms.
    pub fn sample(
        &mut self,
        input: [f32; 2],
        values: [f64; 12],
        warm: f32,
        enabled: f32,
    ) -> [f32; 2] {
        let [
            size,
            predelay_ms,
            vowel,
            vowel_amount,
            amount_1,
            amount_2,
            rate,
            depth,
            low_damp,
            high_damp,
            level,
            mix,
        ] = values;
        let warm = f64::from(warm);
        let enabled = f64::from(enabled);
        let dry = input.map(|x| if x.is_finite() { f64::from(x) } else { 0.0 });
        // Keep both read positions fixed during a fade. The latest pending target
        // follows knob, modulation and tempo changes without scrubbing history.
        self.predelay_transition
            .target([predelay_ms * f64::from(self.sample_rate_hz) * 0.001]);
        let weight = self.predelay_transition.advance();
        let delayed: [f64; 2] = self
            .predelay_transition
            .read_with(0, 0.0, weight, |frames| {
                let read = self.predelay.read(frames);
                let blend = frames.clamp(0.0, 1.0);
                std::array::from_fn(|ch| dry[ch] * (1.0 - blend) + read[ch] * blend)
            });
        self.predelay.write(dry);
        let early = std::array::from_fn(|ch| self.early[ch].sample(delayed[ch]));
        if self.counter.is_multiple_of(32) {
            self.vowel.retarget(vowel as f32);
        }
        let voiced = self.vowel.sample(early, vowel_amount);
        let early = std::array::from_fn(|ch| self.early[ch + 2].sample(voiced[ch]));
        let a = self.pitch[0].sample(
            early,
            amount_1 as f32,
            self.intervals[0] != 0,
            (self.sample_rate_hz / 200).max(1),
        );
        let b = self.pitch[1].sample(
            early,
            amount_2 as f32,
            self.intervals[1] != 0,
            (self.sample_rate_hz / 200).max(1),
        );
        self.injection.set(
            injection_target(self.intervals),
            (self.sample_rate_hz / 200).max(1),
        );
        let injection =
            f64::from(self.injection.next_value()) * (1.0 - 0.25 * amount_1.max(amount_2));
        let tank_input = std::array::from_fn(|ch| early[ch] + injection * (a[ch] + b[ch]));
        let wet = self
            .tank
            .sample(tank_input, size, rate, depth, low_damp, high_damp);
        let output = std::array::from_fn(|ch| {
            self.warm[ch] += self.warm_alpha * (wet[ch] - self.warm[ch]);
            let wet = wet[ch] * (1.0 - warm) + self.warm[ch] * warm;
            let effected = ((1.0 - mix * mix) * dry[ch]
                + 0.9 * (1.0 - (mix - 1.0).powi(2)) * wet * WET_MAKEUP)
                * level;
            (dry[ch] * (1.0 - enabled) + effected * enabled)
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        });
        self.counter = self.counter.wrapping_add(1);
        output
    }
}
