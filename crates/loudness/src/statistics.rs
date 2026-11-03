//! Worker-owned bounded distributions. Distribution bins are spaced by 0.01 LU.
use crate::Observation;
use daw_unit_core::{LOUDNESS_HISTORY, LOUDNESS_SCALARS, LoudnessReading};
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
struct Distribution {
    bins: Box<[u64]>,
    sums: Box<[f64]>,
}
impl Distribution {
    fn new() -> Self {
        Self {
            bins: vec![0; BINS].into_boxed_slice(),
            sums: vec![0.0; BINS].into_boxed_slice(),
        }
    }
    fn clear(&mut self) {
        self.bins.fill(0);
        self.sums.fill(0.0);
    }
    fn add(&mut self, p: f64) {
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
    fn gated(&self, relative: f32) -> (f32, f32) {
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
    fn percentile(&self, gate: f32, fraction: f64) -> f32 {
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
pub struct Statistics {
    pub epoch: u64,
    integrated: Distribution,
    range: Distribution,
    history: [[f32; LOUDNESS_HISTORY]; 2],
    peak: f64,
    max_m: f32,
    max_s: f32,
    incomplete: bool,
}
impl Statistics {
    pub fn new() -> Self {
        Self {
            epoch: 0,
            integrated: Distribution::new(),
            range: Distribution::new(),
            history: [[f32::NEG_INFINITY; LOUDNESS_HISTORY]; 2],
            peak: 0.0,
            max_m: f32::NEG_INFINITY,
            max_s: f32::NEG_INFINITY,
            incomplete: false,
        }
    }
    pub fn accept(&mut self, o: Observation) -> LoudnessReading {
        if self.epoch != o.epoch {
            self.epoch = o.epoch;
            self.integrated.clear();
            self.range.clear();
            self.history.fill([f32::NEG_INFINITY; LOUDNESS_HISTORY]);
            self.peak = 0.0;
            self.max_m = f32::NEG_INFINITY;
            self.max_s = f32::NEG_INFINITY;
            self.incomplete = false;
        }
        let m = lufs(o.m);
        let s = lufs(o.s);
        self.incomplete |= o.incomplete || m > 40.0 || s > 40.0;
        self.max_m = self.max_m.max(lufs(o.max_m));
        self.max_s = self.max_s.max(lufs(o.max_s));
        self.peak = self.peak.max(o.peak);
        if o.seconds >= 0.4 {
            self.integrated.add(o.m);
        }
        if o.seconds >= 3.0 && o.ticks.is_multiple_of(10) {
            self.range.add(o.s);
        }
        let (i, _) = self.integrated.gated(-10.0);
        let (_, gate) = self.range.gated(-20.0);
        let lo = self.range.percentile(gate, 0.10);
        let hi = self.range.percentile(gate, 0.95);
        if o.ticks.is_multiple_of(10) {
            for (h, v) in self.history.iter_mut().zip([m, s]) {
                h.rotate_left(1);
                h[LOUDNESS_HISTORY - 1] = v;
            }
        }
        let tp = db(o.peak);
        let mut r = LoudnessReading::default();
        r.values[..LOUDNESS_SCALARS].copy_from_slice(&[
            m,
            s,
            i,
            self.max_m,
            self.max_s,
            db(self.peak),
            if lo.is_finite() && hi.is_finite() {
                hi - lo
            } else {
                0.0
            },
            lo,
            hi,
            tp,
            if s.is_finite() && tp.is_finite() {
                (tp - s).max(0.0)
            } else {
                0.0
            },
            if i.is_finite() && self.peak > 0.0 {
                (db(self.peak) - i).max(0.0)
            } else {
                0.0
            },
            o.seconds as f32,
            f32::from(self.incomplete),
        ]);
        for (c, h) in self.history.iter().enumerate() {
            r.values[LOUDNESS_SCALARS + c * LOUDNESS_HISTORY
                ..LOUDNESS_SCALARS + (c + 1) * LOUDNESS_HISTORY]
                .copy_from_slice(h);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn power(l: f64) -> f64 {
        10.0_f64.powf((l + 0.691) / 10.0)
    }
    fn observation(tick: u64, l: f64) -> Observation {
        Observation {
            epoch: 1,
            m: power(l),
            s: power(l),
            max_m: power(l),
            max_s: power(l),
            peak: 0.2,
            seconds: tick as f64 / 10.0,
            ticks: tick,
            incomplete: false,
        }
    }
    #[test]
    fn gates_exclude_absolute_and_relative_silence_without_unbounded_storage() {
        let mut s = Statistics::new();
        for t in 1..=100 {
            s.accept(observation(t, -23.0));
        }
        for t in 101..=200 {
            s.accept(observation(t, -80.0));
        }
        let r = s.accept(observation(201, -50.0));
        assert!((r.values[2] + 23.0).abs() < 0.01, "{}", r.values[2]);
    }
    #[test]
    fn range_uses_ten_and_ninety_five_percentiles_and_history_rolls_at_one_hertz() {
        let mut s = Statistics::new();
        let mut r = Default::default();
        for t in 1..=2000 {
            r = s.accept(observation(t, if t <= 1000 { -30.0 } else { -20.0 }));
        }
        assert!((r.values[6] - 10.0).abs() < 0.02);
        assert!((r.values[7] + 30.0).abs() < 0.02);
        assert!((r.values[8] + 20.0).abs() < 0.02);
        assert!((r.values[LOUDNESS_SCALARS] + 30.0).abs() < 0.02);
        assert!((r.values[LOUDNESS_SCALARS + LOUDNESS_HISTORY - 1] + 20.0).abs() < 0.02);
        let mut o = observation(1, -40.0);
        o.epoch = 2;
        let r = s.accept(o);
        assert!(r.values[2].is_infinite());
        assert!(r.values[LOUDNESS_SCALARS..].iter().all(|v| v.is_infinite()));
    }
}
