use crate::delay::Delay;
use daw_unit_core::ModulationRamp;
#[derive(Clone)]
pub(crate) struct Pitch {
    delay: [Delay; 2],
    phase: f64,
    window: f64,
    ratio: ModulationRamp,
    gain: ModulationRamp,
    low: [f64; 2],
    coefficient: f64,
}
impl Pitch {
    pub fn new(rate: u32, pitch: i8, amount: f32) -> Self {
        Self {
            delay: std::array::from_fn(|_| Delay::new((f64::from(rate) * 0.6).ceil() as usize + 4)),
            phase: 0.0,
            window: f64::from(rate) * 0.6,
            ratio: ModulationRamp::new(2.0f32.powf(daw_unit_core::shimmer_pitch(pitch) / 12.0)),
            gain: ModulationRamp::new(if pitch == 0 { 0.0 } else { amount }),
            low: [0.0; 2],
            coefficient: 1.0 - (-std::f64::consts::TAU * 10000.0 / f64::from(rate)).exp(),
        }
    }
    pub fn retarget(&mut self, pitch: i8, amount: f32, frames: u32) {
        self.ratio.set(
            2.0f32.powf(daw_unit_core::shimmer_pitch(pitch) / 12.0),
            frames,
        );
        self.gain.set(if pitch == 0 { 0.0 } else { amount }, frames);
    }
    pub fn sample(&mut self, input: [f64; 2], amount: f32, active: bool, frames: u32) -> [f64; 2] {
        self.gain.set(if active { amount } else { 0.0 }, frames);
        // Maximum regeneration 0.65. The read and crossfade are convex, and
        // filtering is a convex one-pole. This loop remains bounded while pitching.
        let a = f64::from(self.gain.next_value());
        let feedback = 0.65 * (1.0 - (1.0 - a).powi(2));
        let p = self.phase;
        let other = (p + 0.5).fract();
        let weight = 0.5 - 0.5 * (std::f64::consts::TAU * p).cos();
        let output = std::array::from_fn(|ch| {
            let y = weight * self.delay[ch].read(2.0 + p * self.window)
                + (1.0 - weight) * self.delay[ch].read(2.0 + other * self.window);
            self.low[ch] += self.coefficient * (y - self.low[ch]);
            self.delay[ch].write(0.5 * input[ch] + feedback * self.low[ch]);
            feedback * self.low[ch]
        });
        self.phase =
            (self.phase + (1.0 - f64::from(self.ratio.next_value())) / self.window).rem_euclid(1.0);
        output
    }
    pub fn reset(&mut self, amount: f32, active: bool) {
        for d in &mut self.delay {
            d.reset();
        }
        self.phase = 0.0;
        self.low = [0.0; 2];
        self.ratio.reset();
        // The previous sample's effective amount may contain modulation or an
        // unfinished base ramp. Reset starts from the current saved base.
        self.gain = ModulationRamp::new(if active { amount } else { 0.0 });
    }
    pub fn heap_bytes(&self) -> u64 {
        self.delay.iter().map(Delay::heap_bytes).sum()
    }
}
