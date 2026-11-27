//! Oversampled harmonic residual for one stereo Exciter band.
use crate::{
    oversampling::Oversampling,
    waveshaping::{Shape, biased_tanh, shape},
};

#[derive(Clone)]
pub struct ExciterResidual {
    oversampling: Oversampling,

    tape: [f64; 2],
    dc_x: [f64; 2],
    dc_y: [f64; 2],
    tube_bias: f64,
    tube_slope: f64,
    tape_coefficient: f64,
    dc_coefficient: f64,
}

impl ExciterResidual {
    pub fn new(rate: u32) -> Self {
        let tube_bias = 0.35_f64.tanh();
        Self {
            oversampling: Oversampling::new(),

            tape: [0.0; 2],
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            tube_bias,
            tube_slope: 1.0 - tube_bias.powi(2),
            tape_coefficient: 1.0
                - (-std::f64::consts::TAU * 8000.0 / (f64::from(rate) * 4.0)).exp(),
            dc_coefficient: (-std::f64::consts::TAU * 10.0 / (f64::from(rate) * 4.0)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.oversampling.reset();

        self.tape = [0.0; 2];
        self.dc_x = [0.0; 2];
        self.dc_y = [0.0; 2];
    }

    pub fn sample(&mut self, input: [f32; 2], drive: f64, w: [f64; 4]) -> [f64; 2] {
        self.oversampling.process(input, |up| {
            std::array::from_fn(|ch| {
                let x = up[ch] * drive;
                let mut value = 0.0;
                if w[0] > 0.0 {
                    value += w[0] * (shape(Shape::AnalogClip, x) / drive - up[ch]);
                }
                if w[1] > 0.0 {
                    // Shifted tanh is centered at zero with unity small-signal gain.
                    value += w[1]
                        * (biased_tanh(x, 0.35, self.tube_bias) / self.tube_slope / drive - up[ch]);
                }
                if w[2] > 0.0 {
                    let tape = x.tanh() / drive - up[ch];
                    self.tape[ch] += self.tape_coefficient * (tape - self.tape[ch]);
                    value += w[2] * self.tape[ch];
                } else {
                    self.tape[ch] *= 1.0 - self.tape_coefficient;
                }
                if w[3] > 0.0 {
                    value += w[3] * (shape(Shape::HardCurve, x) / drive - up[ch]);
                }
                // Remove DC introduced by asymmetric shaping on the residual path.
                let blocked = value - self.dc_x[ch] + self.dc_coefficient * self.dc_y[ch];
                self.dc_x[ch] = value;
                self.dc_y[ch] = blocked;
                blocked
            })
        })
    }
}
