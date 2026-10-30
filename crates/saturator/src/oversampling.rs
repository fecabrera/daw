//! Fixed 4× polyphase interpolation/decimation. Coefficients are prepared off the callback.
const TAPS: usize = 129;
const HISTORY: usize = TAPS.div_ceil(4);
pub const LATENCY: usize = (TAPS - 1) / 4;
#[derive(Clone)]
pub struct Oversampling {
    kernel: [f64; TAPS],
    // Mirrored rings keep each FIR history contiguous, avoiding modulo per tap.
    input: [[f64; 2]; HISTORY * 2],
    output: [[f64; 2]; TAPS * 2],
    input_index: usize,
    output_index: usize,
}
impl Default for Oversampling {
    fn default() -> Self {
        Self::new()
    }
}
impl Oversampling {
    pub fn new() -> Self {
        let mut kernel = std::array::from_fn(|i| {
            let x = i as f64 - (TAPS / 2) as f64;
            let cutoff = 0.1125; // Cycles per high-rate sample; leave a Nyquist transition band.
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let phase = 2.0 * std::f64::consts::PI * i as f64 / (TAPS - 1) as f64;
            sinc * (0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos())
        });
        let sum: f64 = kernel.iter().sum();
        for coefficient in &mut kernel {
            *coefficient /= sum;
        }
        Self {
            kernel,
            input: [[0.0; 2]; HISTORY * 2],
            output: [[0.0; 2]; TAPS * 2],
            input_index: 0,
            output_index: 0,
        }
    }
    pub fn reset(&mut self) {
        self.input.fill([0.0; 2]);
        self.output.fill([0.0; 2]);
        self.input_index = 0;
        self.output_index = 0;
    }
    pub fn process(
        &mut self,
        input: [f32; 2],
        mut nonlinear: impl FnMut([f64; 2]) -> [f64; 2],
    ) -> [f64; 2] {
        self.input[self.input_index] = input.map(f64::from);
        self.input[self.input_index + HISTORY] = input.map(f64::from);
        let mut result = [0.0; 2];
        for phase in 0..4 {
            let mut up = [0.0; 2];
            let history = &self.input[self.input_index + 1..=self.input_index + HISTORY];
            for (sample, coefficient) in history
                .iter()
                .rev()
                .zip(self.kernel[phase..].iter().step_by(4))
            {
                for ch in 0..2 {
                    up[ch] += sample[ch] * coefficient * 4.0;
                }
            }
            let shaped = nonlinear(up);
            self.output[self.output_index] = shaped;
            self.output[self.output_index + TAPS] = shaped;
            // Read phase zero: input at 4n, combined linear-phase group delay = 128/4 frames.
            if phase == 0 {
                let history = &self.output[self.output_index + 1..=self.output_index + TAPS];
                for (sample, coefficient) in history.iter().rev().zip(&self.kernel) {
                    for ch in 0..2 {
                        result[ch] += sample[ch] * coefficient;
                    }
                }
            }
            self.output_index = (self.output_index + 1) % TAPS;
        }
        self.input_index = (self.input_index + 1) % HISTORY;
        result
    }
}

#[cfg(test)]
impl Oversampling {
    fn reference(
        &mut self,
        input: [f32; 2],
        mut nonlinear: impl FnMut([f64; 2]) -> [f64; 2],
    ) -> [f64; 2] {
        self.input[self.input_index] = input.map(f64::from);
        let mut result = [0.0; 2];
        for phase in 0..4 {
            let mut up = [0.0; 2];
            for tap in (phase..TAPS).step_by(4) {
                let sample = self.input[(self.input_index + HISTORY - tap / 4) % HISTORY];
                for ch in 0..2 {
                    up[ch] += sample[ch] * self.kernel[tap] * 4.0;
                }
            }
            self.output[self.output_index] = nonlinear(up);
            // Read phase zero: input at 4n, combined linear-phase group delay = 128/4 frames.
            if phase == 0 {
                for tap in 0..TAPS {
                    let sample = self.output[(self.output_index + TAPS - tap) % TAPS];
                    for ch in 0..2 {
                        result[ch] += sample[ch] * self.kernel[tap];
                    }
                }
            }
            self.output_index = (self.output_index + 1) % TAPS;
        }
        self.input_index = (self.input_index + 1) % HISTORY;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contiguous_history_matches_scalar_ring_across_wraps_resets_and_nonlinear_changes() {
        let mut actual = Oversampling::new();
        let mut reference = Oversampling::new();
        let mut noise = 1u32;
        for frame in 0..16_384 {
            if frame == 4097 {
                actual.reset();
                reference.reset();
            }
            noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (noise as i32 as f64 / i32::MAX as f64) as f32;
            let input = if frame % 127 == 0 {
                [0.0, -0.0]
            } else {
                [v, v * -0.123]
            };
            let nonlinear = |x: [f64; 2]| {
                x.map(|v| {
                    if frame < 8192 {
                        (v * 2.5).tanh()
                    } else {
                        v.clamp(-0.7, 0.7)
                    }
                })
            };
            assert_eq!(
                actual.process(input, nonlinear).map(f64::to_bits),
                reference.reference(input, nonlinear).map(f64::to_bits)
            );
        }
    }
}
