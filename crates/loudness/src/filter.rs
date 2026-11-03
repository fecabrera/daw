//! BS.1770 K-weighting prototypes transformed at the processing rate.
#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}
impl Biquad {
    pub fn shelf(rate: f64) -> Self {
        let k = (std::f64::consts::PI * 1681.974450955533 / rate).tan();
        let q = 0.7071752369554196;
        let vh = 10.0_f64.powf(3.999843853973347 / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let d = 1.0 + k / q + k * k;
        Self {
            b: [
                (vh + vb * k / q + k * k) / d,
                2.0 * (k * k - vh) / d,
                (vh - vb * k / q + k * k) / d,
            ],
            a: [2.0 * (k * k - 1.0) / d, (1.0 - k / q + k * k) / d],
            z: [0.0; 2],
        }
    }
    pub fn highpass(rate: f64) -> Self {
        let k = (std::f64::consts::PI * 38.13547087602444 / rate).tan();
        let q = 0.5003270373238773;
        let d = 1.0 + k / q + k * k;
        Self {
            b: [1.0, -2.0, 1.0],
            a: [2.0 * (k * k - 1.0) / d, (1.0 - k / q + k * k) / d],
            z: [0.0; 2],
        }
    }
    pub fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z = [
            self.b[1] * x - self.a[0] * y + self.z[1],
            self.b[2] * x - self.a[1] * y,
        ];
        y
    }
    pub fn reset(&mut self) {
        self.z = [0.0; 2];
    }
}
#[derive(Clone)]
pub struct TruePeak {
    kernels: [[f64; 64]; 8],
    history: [[f64; 2]; 64],
    index: usize,
}
impl TruePeak {
    pub fn new() -> Self {
        let kernels = std::array::from_fn(|phase| {
            let mut k = std::array::from_fn(|tap| {
                let x = tap as f64 - 31.0 - phase as f64 / 8.0;
                let sinc = if x.abs() < 1e-10 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                sinc * (0.5 + 0.5 * (std::f64::consts::PI * x / 32.0).cos()).max(0.0)
            });
            let sum: f64 = k.iter().sum();
            for v in &mut k {
                *v /= sum;
            }
            k
        });
        Self {
            kernels,
            history: [[0.0; 2]; 64],
            index: 0,
        }
    }
    pub fn reset(&mut self) {
        self.history.fill([0.0; 2]);
        self.index = 0;
    }
    pub fn tick(&mut self, x: [f64; 2], hq: bool) -> f64 {
        self.history[self.index] = x;
        self.index = (self.index + 1) % 64;
        let mut peak = x[0].abs().max(x[1].abs());
        for phase in (0..8).step_by(if hq { 1 } else { 2 }) {
            let mut y = [0.0; 2];
            for tap in 0..64 {
                let v = self.history[(self.index + tap) % 64];
                for c in 0..2 {
                    y[c] += v[c] * self.kernels[phase][tap];
                }
            }
            peak = peak.max(y[0].abs()).max(y[1].abs());
        }
        peak
    }
}
