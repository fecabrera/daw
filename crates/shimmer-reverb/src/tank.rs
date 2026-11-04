use crate::delay::Delay;
/// Infinity-norm contractive eight-line network. Every scattering row has an
/// absolute sum of one; the return gain is at most 0.92. Moving linear reads,
/// convex damping and convex DC rejection cannot increase that bound.
#[derive(Clone)]
pub(crate) struct Tank {
    delay: [Delay; 8],
    low: [f64; 8],
    dc: [f64; 8],
    phase: f64,
    rate: f64,
    motion: [f64; 8],
    motion_step: [f64; 8],
    coefficients: [f64; 2],
    tick: u8,
}
impl Tank {
    pub fn new(rate: u32) -> Self {
        Self {
            delay: std::array::from_fn(|_| Delay::new((f64::from(rate) * 0.3).ceil() as usize)),
            low: [0.0; 8],
            dc: [0.0; 8],
            phase: 0.0,
            rate: f64::from(rate),
            motion: [0.0; 8],
            motion_step: [0.0; 8],
            coefficients: [
                1.0,
                1.0 - (-std::f64::consts::TAU * 400.0 / f64::from(rate)).exp(),
            ],
            tick: 0,
        }
    }
    pub fn sample(
        &mut self,
        input: [f64; 2],
        size: f64,
        rate: f64,
        depth: f64,
        low: f64,
        high: f64,
    ) -> [f64; 2] {
        const TIMES: [f64; 8] = [
            0.0437, 0.0571, 0.0713, 0.0899, 0.1093, 0.1319, 0.1571, 0.1811,
        ];
        if self.tick == 0 {
            self.coefficients[0] = 1.0
                - (-std::f64::consts::TAU * (18000.0 * (800.0f64 / 18000.0).powf(high))
                    / self.rate)
                    .exp();
            self.motion_step = std::array::from_fn(|i| {
                let target =
                    0.002 * depth * (std::f64::consts::TAU * (self.phase + i as f64 / 8.0)).sin();
                (target - self.motion[i]) / 32.0
            });
        }
        let [lp, hp] = self.coefficients;
        let taps: [f64; 8] = std::array::from_fn(|i| {
            self.motion[i] += self.motion_step[i];
            let q = self.delay[i].read(self.rate * (TIMES[i] * (0.3 + size) + self.motion[i]));
            self.low[i] += lp * (q - self.low[i]);
            self.dc[i] += hp * (self.low[i] - self.dc[i]);
            // Both low and dc are bounded by the input history supremum.
            // Half their difference has the same bound, including automation.
            self.low[i] * (1.0 - low) + 0.5 * (self.low[i] - self.dc[i]) * low
        });
        let feedback = 0.25 + 0.67 * size;
        for i in 0..8 {
            let j = (i + 3) % 8;
            let k = (i + 5) % 8;
            let mixed = 0.5 * taps[j]
                + if i % 2 == 0 {
                    0.5 * taps[k]
                } else {
                    -0.5 * taps[k]
                };
            self.delay[i].write(input[i % 2] * 0.18 + feedback * mixed);
        }
        self.phase = (self.phase + (0.1 + 1.7 * rate) / self.rate).fract();
        self.tick = (self.tick + 1) % 32;
        [
            0.25 * (taps[0] + taps[2] - taps[4] + taps[6]),
            0.25 * (taps[1] - taps[3] + taps[5] + taps[7]),
        ]
    }
    pub fn reset(&mut self) {
        for d in &mut self.delay {
            d.reset();
        }
        self.low = [0.0; 8];
        self.dc = [0.0; 8];
        self.phase = 0.0;
        self.motion = [0.0; 8];
        self.motion_step = [0.0; 8];
        self.tick = 0;
    }
    pub fn heap_bytes(&self) -> u64 {
        self.delay.iter().map(Delay::heap_bytes).sum()
    }
}
