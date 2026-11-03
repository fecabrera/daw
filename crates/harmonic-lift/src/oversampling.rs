//! Independent Kaiser-windowed sinc 2× FIR. No extracted reference coefficients/code.
#[derive(Clone)]
struct Fir<const N: usize> {
    taps: [f64; N],
    history: [[f64; 2]; N],
    cursor: usize,
}
fn i0(x: f64) -> f64 {
    let mut sum = 1.;
    let mut term = 1.;
    for k in 1..=40 {
        term *= x * x / (4. * (k * k) as f64);
        sum += term;
        if term < sum * 1e-16 {
            break;
        }
    }
    sum
}
impl<const N: usize> Fir<N> {
    fn new(beta: f64) -> Self {
        let center = (N - 1) as f64 / 2.;
        let denom = i0(beta);
        let mut taps = std::array::from_fn(|i| {
            let d = i as f64 - center;
            let sinc = if d == 0. {
                0.5
            } else {
                (std::f64::consts::FRAC_PI_2 * d).sin() / (std::f64::consts::PI * d)
            };
            sinc * i0(beta * (1. - (d / center).powi(2)).max(0.).sqrt()) / denom
        });
        let total: f64 = taps.iter().sum();
        for tap in &mut taps {
            *tap /= total;
        }
        Self {
            taps,
            history: [[0.; 2]; N],
            cursor: 0,
        }
    }
    fn sample(&mut self, x: [f64; 2]) -> [f64; 2] {
        self.history[self.cursor] = x;
        let mut out = [0.; 2];
        for (i, tap) in self.taps.iter().enumerate() {
            let frame = self.history[(self.cursor + N - i) % N];
            for ch in 0..2 {
                out[ch] += tap * frame[ch];
            }
        }
        self.cursor = (self.cursor + 1) % N;
        out
    }
    fn reset(&mut self) {
        self.history.fill([0.; 2]);
        self.cursor = 0;
    }
}
#[derive(Clone)]
pub(super) struct Oversampling {
    up: Fir<119>,
    down: Fir<79>,
}
impl Oversampling {
    pub fn new() -> Self {
        Self {
            up: Fir::new(8.6),
            down: Fir::new(7.5),
        }
    }
    pub fn up(&mut self, x: [f64; 2]) -> [[f64; 2]; 2] {
        [self.up.sample(x.map(|v| 2. * v)), self.up.sample([0.; 2])]
    }
    pub fn down(&mut self, x: [[f64; 2]; 2]) -> [f64; 2] {
        let result = self.down.sample(x[0]);
        self.down.sample(x[1]);
        result
    }
    pub fn reset(&mut self) {
        self.up.reset();
        self.down.reset();
    }
}
