//! Two bounded rotating resonators per channel. Each state transition has norm r<1.
#[derive(Clone)]
pub(crate) struct Vowel {
    state: [[[f64; 2]; 2]; 2],
    rotation: [[f64; 2]; 2],
    radius: f64,
    rate: f64,
}
impl Vowel {
    pub fn new(rate: u32) -> Self {
        let mut v = Self {
            state: [[[0.0; 2]; 2]; 2],
            rotation: [[1.0, 0.0]; 2],
            radius: (-std::f64::consts::TAU * 45.0 / f64::from(rate)).exp(),
            rate: f64::from(rate),
        };
        v.retarget(0.0);
        v
    }
    pub fn retarget(&mut self, position: f32) {
        const FORMANTS: [[f64; 2]; 5] = [
            [800.0, 1150.0],
            [450.0, 1800.0],
            [300.0, 2300.0],
            [500.0, 900.0],
            [350.0, 700.0],
        ];
        let p = f64::from(position.clamp(0.0, 1.0)) * 4.0;
        let i = (p.floor() as usize).min(3);
        let f = p - i as f64;
        self.rotation = std::array::from_fn(|j| {
            let angle = std::f64::consts::TAU
                * (FORMANTS[i][j] * (1.0 - f) + FORMANTS[i + 1][j] * f)
                / self.rate;
            [angle.cos(), angle.sin()]
        });
    }
    pub fn sample(&mut self, input: [f64; 2], amount: f64) -> [f64; 2] {
        std::array::from_fn(|ch| {
            let mut bands = 0.0;
            for j in 0..2 {
                let [x, y] = self.state[ch][j];
                let [c, s] = self.rotation[j];
                let next = [
                    self.radius * (c * x - s * y) + (1.0 - self.radius) * input[ch],
                    self.radius * (s * x + c * y),
                ];
                self.state[ch][j] = next;
                bands += next[0] * 0.5;
            }
            input[ch] * (1.0 - amount) + bands * amount
        })
    }
    pub fn reset(&mut self) {
        self.state = [[[0.0; 2]; 2]; 2];
    }
}
