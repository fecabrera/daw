//! Portable trapezoidal SVF primitives. Backend coefficient types stay private.
use fundsp::svf::SvfCoefs;
fn tick(x: f64, c: &SvfCoefs<f64>, state: &mut [f64; 2]) -> [f64; 2] {
    let v3 = x - state[1];
    let band = c.a1 * state[0] + c.a2 * v3;
    let low = state[1] + c.a2 * state[0] + c.a3 * v3;
    state[0] = 2. * band - state[0];
    state[1] = 2. * low - state[1];
    [low, band]
}
#[derive(Default, Clone)]
pub struct StateVariableFilter {
    state: [f64; 2],
    coefs: SvfCoefs<f64>,
    configured: Option<(f64, f64, f64)>,
}
impl StateVariableFilter {
    fn run(&mut self, x: f64, f: f64, q: f64, rate: f64) -> [f64; 2] {
        if self.configured != Some((f, q, rate)) {
            self.coefs = SvfCoefs::lowpass(rate, f, q);
            self.configured = Some((f, q, rate));
        }
        tick(x, &self.coefs, &mut self.state)
    }
    pub fn band(&mut self, x: f64, f: f64, q: f64, rate: f64) -> f64 {
        self.run(x, f, q, rate)[1]
    }
    pub fn high(&mut self, x: f64, f: f64, q: f64, rate: f64) -> f64 {
        let [low, band] = self.run(x, f, q, rate);
        x - band / q - low
    }
    pub fn sample(&mut self, x: f64, f: f64, q: f64, rate: f64, weights: [f64; 4]) -> f64 {
        let [low, band] = self.run(x, f, q, rate);
        let normalized_band = band / q;
        let high = x - normalized_band - low;
        let notch = x - normalized_band;
        [low, high, normalized_band, notch]
            .into_iter()
            .zip(weights)
            .map(|(v, w)| v * w)
            .sum()
    }
    pub fn reset(&mut self) {
        self.state = [0.; 2];
    }
}
#[derive(Clone)]
pub struct HighShelf {
    coefs: SvfCoefs<f64>,
    state: [f64; 2],
}
impl HighShelf {
    pub fn new(rate: f64, f: f64, q: f64, gain_db: f64) -> Self {
        Self {
            coefs: SvfCoefs::highshelf(rate, f, q, 10_f64.powf(gain_db / 20.)),
            state: [0.; 2],
        }
    }
    pub fn sample(&mut self, x: f64) -> f64 {
        let [low, band] = tick(x, &self.coefs, &mut self.state);
        self.coefs.m0 * x + self.coefs.m1 * band + self.coefs.m2 * low
    }
    pub fn reset(&mut self) {
        self.state = [0.; 2];
    }
}
