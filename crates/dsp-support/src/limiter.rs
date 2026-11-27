//! Fixed-latency stereo limiting and peak range search.
use crate::limiter_guard as true_peak;
pub const GUARD_FRAMES: u32 = true_peak::BLOCK as u32;

#[derive(Clone, Copy, Default)]
struct Frame {
    dry: [f64; 2],
    wet: [f64; 2],
}
fn peak(input: [f64; 2]) -> f64 {
    input[0].abs().max(input[1].abs())
}
fn ms(input: [f64; 2]) -> [f64; 2] {
    [(input[0] + input[1]) * 0.5, (input[0] - input[1]) * 0.5]
}
fn maximum(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i].max(b[i]))
}

#[derive(Clone)]
pub struct LimiterKernel {
    rate: u32,
    frames: Vec<Frame>,
    peaks: Vec<[f64; 3]>,
    tree_size: usize,
    index: usize,
    gains: [f64; 3],
    timing: (usize, f64),
    coefficients: [f64; 2],
    true_peak: bool,
    guard: true_peak::Guard,
    reduction_db: f32,
}

impl LimiterKernel {
    pub fn new(rate: u32, true_peak: bool) -> Self {
        let length = rate.div_ceil(100) as usize + 1;
        let tree_size = length.next_power_of_two();
        Self {
            rate,
            frames: vec![Frame::default(); length],
            peaks: vec![[0.0; 3]; tree_size * 2],
            tree_size,
            index: 0,
            gains: [1.0; 3],
            timing: (usize::MAX, f64::NAN),
            coefficients: [0.0; 2],
            true_peak,
            guard: true_peak::Guard::new(),
            reduction_db: 0.0,
        }
    }

    pub fn set_true_peak(&mut self, enabled: bool) {
        self.true_peak = enabled;
    }

    pub fn reduction_db(&self) -> f32 {
        self.reduction_db
    }

    pub fn heap_bytes(&self) -> u64 {
        crate::memory::vec_bytes(&self.frames) + crate::memory::vec_bytes(&self.peaks)
    }

    pub fn reset(&mut self) {
        self.frames.fill(Frame::default());
        self.peaks.fill([0.0; 3]);
        self.index = 0;
        self.gains = [1.0; 3];
        self.guard.reset();
        self.reduction_db = 0.0;
    }

    fn update_peak(&mut self, index: usize, value: [f64; 3]) {
        let mut node = self.tree_size + index;
        self.peaks[node] = value;
        while node > 1 {
            node /= 2;
            self.peaks[node] = maximum(self.peaks[node * 2], self.peaks[node * 2 + 1]);
        }
    }
    fn range_peak(&self, start: usize, end: usize) -> [f64; 3] {
        let mut result = [0.0; 3];
        let (mut left, mut right) = (start + self.tree_size, end + self.tree_size);
        while left < right {
            if left & 1 != 0 {
                result = maximum(result, self.peaks[left]);
                left += 1;
            }
            if right & 1 != 0 {
                right -= 1;
                result = maximum(result, self.peaks[right]);
            }
            left /= 2;
            right /= 2;
        }
        result
    }
    pub fn sample(&mut self, input: [f32; 2], p: [f64; 6]) -> [f32; 2] {
        let dry = input.map(|v| if v.is_finite() { f64::from(v) } else { 0.0 });
        let wet = dry.map(|v| v * p[0]);
        self.frames[self.index] = Frame { dry, wet };
        let mid_side = ms(wet);
        self.update_peak(
            self.index,
            [peak(wet), mid_side[0].abs(), mid_side[1].abs()],
        );
        self.index = (self.index + 1) % self.frames.len();
        let frame = self.frames[self.index];
        let lookahead = (p[2] * f64::from(self.rate) * 0.001).round() as usize;
        let end = self.index + lookahead.min(self.frames.len() - 1) + 1;
        let detected = if end <= self.frames.len() {
            self.range_peak(self.index, end)
        } else {
            maximum(
                self.range_peak(self.index, self.frames.len()),
                self.range_peak(0, end - self.frames.len()),
            )
        };
        let ceiling = p[3] * (1.0 - 1e-7);
        if self.timing.0 != lookahead {
            self.coefficients[0] = if lookahead == 0 {
                0.0
            } else {
                (-3.0 / lookahead as f64).exp()
            };
        }
        if self.timing.1 != p[1] {
            self.coefficients[1] = (-1.0 / (p[1] * 0.001 * f64::from(self.rate))).exp();
        }
        self.timing = (lookahead, p[1]);
        let [attack, release] = self.coefficients;
        for (gain, detected) in self.gains.iter_mut().zip(detected) {
            let target = (ceiling / detected.max(1e-30)).min(1.0);
            let coefficient = if target < *gain { attack } else { release };
            *gain = target + coefficient * (*gain - target);
        }
        let lr = frame.wet.map(|v| v * self.gains[0]);
        let [mid, side] = ms(frame.wet);
        let (mid, side) = (mid * self.gains[1], side * self.gains[2]);
        let ms = [mid + side, mid - side];
        let mut limited = std::array::from_fn(|ch| lr[ch] * (1.0 - p[4]) + ms[ch] * p[4]);
        // Recombining M/S can exceed either component's ceiling. Link the final L/R guard.
        let safety = (ceiling / peak(limited).max(1e-30)).min(1.0);
        for value in &mut limited {
            *value *= safety;
        }
        let output = std::array::from_fn(|ch| frame.dry[ch] * (1.0 - p[5]) + limited[ch] * p[5]);
        let output = self
            .guard
            .tick(output, ceiling, self.true_peak && p[5] >= 1.0 - 1e-9);
        let reduction =
            (self.gains[0] * (1.0 - p[4]) + self.gains[1].min(self.gains[2]) * p[4]) * safety;
        self.reduction_db =
            ((-20.0 * reduction.max(1e-30).log10()) as f32 + self.guard.reduction_db) * p[5] as f32;
        output.map(|v| v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
    }
}
