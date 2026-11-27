//! Bounded gated energy distributions with 0.01 LU bins.
const BINS: usize = 14001;
pub fn lufs(p: f64) -> f32 {
    if p > 1e-30 {
        (-0.691 + 10.0 * p.log10()) as f32
    } else {
        f32::NEG_INFINITY
    }
}
pub fn db(a: f64) -> f32 {
    if a > 1e-30 {
        (20.0 * a.log10()) as f32
    } else {
        f32::NEG_INFINITY
    }
}
/// Fixed-size energy histogram for absolute and relative loudness gating.
pub struct Distribution {
    bins: Box<[u64]>,
    sums: Box<[f64]>,
}
impl Default for Distribution {
    fn default() -> Self {
        Self {
            bins: vec![0; BINS].into_boxed_slice(),
            sums: vec![0.0; BINS].into_boxed_slice(),
        }
    }
}

impl Distribution {
    pub fn clear(&mut self) {
        self.bins.fill(0);
        self.sums.fill(0.0);
    }
    pub fn add(&mut self, p: f64) {
        let l = lufs(p);
        if l >= -70.0 {
            let i = ((l + 100.0) * 100.0).round().clamp(0.0, (BINS - 1) as f32) as usize;
            self.bins[i] += 1;
            self.sums[i] += p;
        }
    }
    fn above(&self, t: f32) -> (u64, f64) {
        let i = ((t + 100.0) * 100.0).ceil().clamp(0.0, BINS as f32) as usize;
        (self.bins[i..].iter().sum(), self.sums[i..].iter().sum())
    }
    pub fn gated(&self, relative: f32) -> (f32, f32) {
        let (n, sum) = self.above(-70.0);
        if n == 0 {
            return (f32::NEG_INFINITY, -70.0);
        }
        let gate = (lufs(sum / n as f64) + relative).max(-70.0);
        let (n, sum) = self.above(gate);
        (
            if n > 0 {
                lufs(sum / n as f64)
            } else {
                f32::NEG_INFINITY
            },
            gate,
        )
    }
    pub fn percentile(&self, gate: f32, fraction: f64) -> f32 {
        let (n, _) = self.above(gate);
        if n == 0 {
            return f32::NEG_INFINITY;
        }
        let rank = ((n - 1) as f64 * fraction).round() as u64;
        let mut seen = 0;
        for (i, &count) in self.bins.iter().enumerate() {
            let value = i as f32 / 100.0 - 100.0;
            if value < gate {
                continue;
            }
            seen += count;
            if seen > rank {
                return value;
            }
        }
        f32::NEG_INFINITY
    }
}
