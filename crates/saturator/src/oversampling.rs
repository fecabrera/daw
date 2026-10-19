//! Fixed 4× polyphase interpolation/decimation. Coefficients are prepared off the callback.
const TAPS: usize = 129;
const HISTORY: usize = TAPS.div_ceil(4);
pub(super) const LATENCY: usize = (TAPS - 1) / 4;
pub(super) struct Oversampling {
    kernel: [f64; TAPS],
    input: [[f64; 2]; HISTORY],
    output: [[f64; 2]; TAPS],
    input_index: usize,
    output_index: usize,
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
            input: [[0.0; 2]; HISTORY],
            output: [[0.0; 2]; TAPS],
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
