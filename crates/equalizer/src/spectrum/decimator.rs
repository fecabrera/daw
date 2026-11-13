//! Prepared 63-tap halfbands. Symmetry needs 17 multiplies/channel/output.
use super::Complex;
const TAPS: usize = 63;
#[derive(Clone)]
struct Stage {
    ring: [Complex; TAPS],
    position: usize,
    valid: usize,
    phase: bool,
}
impl Default for Stage {
    fn default() -> Self {
        Self {
            ring: [[0.0; 2]; TAPS],
            position: 0,
            valid: 0,
            phase: false,
        }
    }
}
impl Stage {
    fn sample(&mut self, value: Complex, coefficients: &[f32; 17]) -> Option<Complex> {
        self.ring[self.position] = value;
        self.position = (self.position + 1) % TAPS;
        self.valid = (self.valid + 1).min(TAPS);
        self.phase = !self.phase;
        if self.phase {
            return None;
        }
        let at = |offset: usize| {
            if offset < self.valid {
                self.ring[(self.position + TAPS - 1 - offset) % TAPS]
            } else {
                [0.0; 2]
            }
        };
        let mut result = at(31).map(|v| v * coefficients[16]);
        for (index, coefficient) in coefficients[..16].iter().enumerate() {
            let a = at(2 * index);
            let b = at(62 - 2 * index);
            for channel in 0..2 {
                result[channel] += (a[channel] + b[channel]) * coefficient;
            }
        }
        Some(result)
    }
    fn reset(&mut self) {
        self.position = 0;
        self.valid = 0;
        self.phase = false;
    }
}
#[derive(Clone)]
pub(super) struct Decimator {
    stages: [Stage; 7],
    coefficients: [f32; 17],
    length: usize,
}
impl Decimator {
    pub fn new(length: usize) -> Self {
        assert!((5..=7).contains(&length));
        // Kaiser-windowed sinc, cutoff 1/4 cycle/input sample. Prepared in f64.
        let i0 = |x: f64| {
            let mut sum = 1.0;
            let mut term = 1.0;
            for k in 1..=40 {
                term *= (x * 0.5 / k as f64).powi(2);
                sum += term;
            }
            sum
        };
        let beta = 9.0;
        let normal = i0(beta);
        let taps: [f64; TAPS] = std::array::from_fn(|i| {
            let m = i as f64 - 31.0;
            let sinc = if i == 31 {
                0.5
            } else if i % 2 == 1 {
                0.0
            } else {
                (std::f64::consts::FRAC_PI_2 * m).sin() / (std::f64::consts::PI * m)
            };
            sinc * i0(beta * (1.0 - (m / 31.0).powi(2)).max(0.0).sqrt()) / normal
        });
        let gain: f64 = taps.iter().sum();
        Self {
            stages: std::array::from_fn(|_| Stage::default()),
            coefficients: std::array::from_fn(|i| {
                (taps[if i == 16 { 31 } else { 2 * i }] / gain) as f32
            }),
            length,
        }
    }
    pub fn factor(&self) -> usize {
        1 << self.length
    }
    pub fn delay_frames(&self) -> u64 {
        31 * (self.factor() as u64 - 1)
    }
    pub fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }
    pub fn sample(&mut self, mut value: Complex) -> Option<Complex> {
        for stage in &mut self.stages[..self.length] {
            value = stage.sample(value, &self.coefficients)?;
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cascade_passband_and_folded_alias_response() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let length =
                5 + ((rate / 44100_u32).max(1).next_power_of_two()).trailing_zeros() as usize;
            let d = Decimator::new(length);
            let response = |hz: f64| {
                let mut gain = 1.0;
                for stage in 0..length {
                    let w = std::f64::consts::TAU * hz / (rate as f64 / (1 << stage) as f64);
                    let mut h = f64::from(d.coefficients[16]);
                    for i in 0..16 {
                        h += 2.0 * f64::from(d.coefficients[i]) * (w * (31 - 2 * i) as f64).cos();
                    }
                    gain *= h.abs();
                }
                gain
            };
            for hz in 0..=400 {
                assert!((20.0 * response(hz as f64).log10()).abs() < 0.2);
            }
            let output_rate = rate as f64 / d.factor() as f64;
            for hz in (401..rate / 2).step_by(3) {
                let folded =
                    ((hz as f64 + output_rate * 0.5) % output_rate - output_rate * 0.5).abs();
                if (20.0..=250.0).contains(&folded) {
                    assert!(
                        response(hz as f64) < 0.0001,
                        "{rate} alias {hz}: {}",
                        response(hz as f64)
                    );
                }
            }
        }
    }
}
