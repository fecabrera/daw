//! Eight-phase windowed-sinc reconstruction guard. This is an estimate, not certification.
//! Each block uses one linked gain. Test every FIR window that the block can affect,
//! including its zero-extended tail, against the already committed output history.
const TAPS: usize = 64;
pub(super) const BLOCK: usize = 32;

pub(super) struct Guard {
    coefficients: [[f64; TAPS]; 8],
    history: [[f64; 2]; TAPS],
    pending: [[f64; 2]; BLOCK],
    ready: [[f64; 2]; BLOCK],
    index: usize,
    ceiling: f64,
    active: bool,
    pub reduction_db: f32,
}
impl Guard {
    pub fn new() -> Self {
        let coefficients = std::array::from_fn(|phase| {
            let mut kernel = std::array::from_fn(|tap| {
                let x = tap as f64 - 31.0 - phase as f64 / 8.0;
                let sinc = if x.abs() < 1e-10 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * x / 32.0).cos();
                sinc * window.max(0.0)
            });
            let sum: f64 = kernel.iter().sum();
            for value in &mut kernel {
                *value /= sum;
            }
            kernel
        });
        Self {
            coefficients,
            history: [[0.0; 2]; TAPS],
            pending: [[0.0; 2]; BLOCK],
            ready: [[0.0; 2]; BLOCK],
            index: 0,
            ceiling: 1.0,
            active: false,
            reduction_db: 0.0,
        }
    }
    pub fn reset(&mut self) {
        self.history.fill([0.0; 2]);
        self.pending.fill([0.0; 2]);
        self.ready.fill([0.0; 2]);
        self.index = 0;
        self.active = false;
        self.ceiling = 1.0;
        self.reduction_db = 0.0;
    }
    pub fn tick(&mut self, input: [f64; 2], ceiling: f64, active: bool) -> [f64; 2] {
        let output = self.ready[self.index];
        self.pending[self.index] = input;
        self.active |= active;
        self.ceiling = self.ceiling.min(ceiling);
        self.index += 1;
        if self.index == BLOCK {
            let mut gain: f64 = 1.0;
            if self.active {
                // Small headroom for finite interpolation and f32 output rounding.
                let ceiling = self.ceiling * 0.98;
                for center in -32..BLOCK as isize + 32 {
                    for kernel in &self.coefficients {
                        for channel in 0..2 {
                            let mut previous = 0.0;
                            let mut current = 0.0;
                            for (tap, coefficient) in kernel.iter().enumerate() {
                                let index = center + tap as isize - 31;
                                if (-64..0).contains(&index) {
                                    previous +=
                                        coefficient * self.history[(index + 64) as usize][channel];
                                } else if (0..BLOCK as isize).contains(&index) {
                                    current += coefficient * self.pending[index as usize][channel];
                                }
                            }
                            let bound = if current > 1e-30 {
                                (ceiling - previous) / current
                            } else if current < -1e-30 {
                                (-ceiling - previous) / current
                            } else {
                                1.0
                            };
                            gain = gain.min(bound.clamp(0.0, 1.0));
                        }
                    }
                }
            }
            self.reduction_db = (-20.0 * gain.max(1e-30).log10()) as f32;
            for (ready, input) in self.ready.iter_mut().zip(self.pending) {
                *ready = input.map(|value| value * gain);
            }
            self.history.copy_within(BLOCK.., 0);
            self.history[TAPS - BLOCK..].copy_from_slice(&self.ready);
            self.ceiling = 1.0;
            self.active = false;
            self.index = 0;
        }
        output
    }
}
