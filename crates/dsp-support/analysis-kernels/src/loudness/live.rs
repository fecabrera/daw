//! Prepared rolling measurements and worker-side loudness statistics.
use super::{Distribution, KWeighting, TruePeak, db, lufs};

/// One completed interval of numeric observations.
#[derive(Clone, Copy)]
pub struct Observation {
    pub momentary_power: f64,
    pub short_term_power: f64,
    pub true_peak: f64,
    pub max_momentary_power: f64,
    pub max_short_term_power: f64,
    pub seconds: f64,
    pub ticks: u64,
}

/// Prepared 400 ms and 3 s windows, with bounded work per frame.
pub struct RollingLoudness {
    rate: usize,
    weighting: KWeighting,
    true_peak: TruePeak,
    powers: Box<[f64]>,
    index: usize,
    count: u64,
    sum_m: f64,
    sum_s: f64,
    peak: f64,
    max_m: f64,
    max_s: f64,
}

impl RollingLoudness {
    /// Allocate storage before processing. The sample rate must be at least 100 Hz.
    pub fn new(rate: u32) -> Self {
        assert!(
            rate >= 100,
            "rolling loudness needs a sample rate of at least 100 Hz"
        );
        Self {
            rate: rate as usize,
            weighting: KWeighting::new(f64::from(rate)),
            true_peak: TruePeak::new(),
            powers: vec![0.0; rate as usize * 3].into_boxed_slice(),
            index: 0,
            count: 0,
            sum_m: 0.0,
            sum_s: 0.0,
            peak: 0.0,
            max_m: 0.0,
            max_s: 0.0,
        }
    }

    pub fn heap_bytes(&self) -> u64 {
        self.powers.len() as u64 * 8
    }

    /// Old ring entries stay masked until a full window of new samples exists.
    pub fn reset(&mut self) {
        self.index = 0;
        self.count = 0;
        self.sum_m = 0.0;
        self.sum_s = 0.0;
        self.peak = 0.0;
        self.max_m = 0.0;
        self.max_s = 0.0;
        self.weighting.reset();
        self.true_peak.reset();
    }

    /// Accept one finite stereo frame. No allocation, wait or worker call is made.
    #[inline]
    pub fn push(&mut self, frame: [f64; 2], hq: bool) -> Option<Observation> {
        let power = self.weighting.power(frame);
        let m = self.rate * 2 / 5;
        let n = self.powers.len();
        if self.count >= m as u64 {
            self.sum_m -= self.powers[(self.index + n - m) % n];
        }
        if self.count >= n as u64 {
            self.sum_s -= self.powers[self.index];
        }
        self.powers[self.index] = power;
        self.index = (self.index + 1) % n;
        self.sum_m += power;
        self.sum_s += power;
        self.count += 1;
        self.peak = self.peak.max(self.true_peak.tick(frame, hq));
        if self.count.is_multiple_of((self.rate / 100) as u64) {
            self.max_m = self.max_m.max(self.sum_m.max(0.0) / m as f64);
            self.max_s = self.max_s.max(self.sum_s.max(0.0) / n as f64);
        }
        let hop = (self.rate / 10) as u64;
        if !self.count.is_multiple_of(hop) {
            return None;
        }
        let observation = Observation {
            momentary_power: self.sum_m.max(0.0) / m as f64,
            short_term_power: self.sum_s.max(0.0) / n as f64,
            true_peak: self.peak,
            max_momentary_power: self.max_m,
            max_short_term_power: self.max_s,
            seconds: self.count as f64 / self.rate as f64,
            ticks: self.count / hop,
        };
        self.peak = 0.0;
        Some(observation)
    }
}

/// Current and accumulated numeric measurements, without display history.
pub struct Measurements {
    pub momentary: f32,
    pub short_term: f32,
    pub integrated: f32,
    pub max_momentary: f32,
    pub max_short_term: f32,
    pub max_true_peak: f32,
    pub loudness_range: f32,
    pub range_low: f32,
    pub range_high: f32,
    pub true_peak: f32,
    pub peak_to_short_term: f32,
    pub peak_to_integrated: f32,
    pub incomplete: bool,
}

/// Bounded worker-side reduction. The caller owns observation delivery and reset epochs.
pub struct IntegratedStatistics {
    integrated: Distribution,
    range: Distribution,
    peak: f64,
    max_m: f32,
    max_s: f32,
    incomplete: bool,
}

impl Default for IntegratedStatistics {
    fn default() -> Self {
        Self {
            integrated: Distribution::default(),
            range: Distribution::default(),
            peak: 0.0,
            max_m: f32::NEG_INFINITY,
            max_s: f32::NEG_INFINITY,
            incomplete: false,
        }
    }
}

impl IntegratedStatistics {
    pub fn reset(&mut self) {
        self.integrated.clear();
        self.range.clear();
        self.peak = 0.0;
        self.max_m = f32::NEG_INFINITY;
        self.max_s = f32::NEG_INFINITY;
        self.incomplete = false;
    }

    /// Reduce one observation on a worker. Gating scans the fixed distributions.
    pub fn accept(&mut self, o: Observation, incomplete: bool) -> Measurements {
        let m = lufs(o.momentary_power);
        let s = lufs(o.short_term_power);
        self.incomplete |= incomplete || m > 40.0 || s > 40.0;
        self.max_m = self.max_m.max(lufs(o.max_momentary_power));
        self.max_s = self.max_s.max(lufs(o.max_short_term_power));
        self.peak = self.peak.max(o.true_peak);
        if o.seconds >= 0.4 {
            self.integrated.add(o.momentary_power);
        }
        if o.seconds >= 3.0 && o.ticks.is_multiple_of(10) {
            self.range.add(o.short_term_power);
        }
        let (i, _) = self.integrated.gated(-10.0);
        let (_, gate) = self.range.gated(-20.0);
        let lo = self.range.percentile(gate, 0.10);
        let hi = self.range.percentile(gate, 0.95);
        let tp = db(o.true_peak);
        let max_tp = db(self.peak);
        Measurements {
            momentary: m,
            short_term: s,
            integrated: i,
            max_momentary: self.max_m,
            max_short_term: self.max_s,
            max_true_peak: max_tp,
            loudness_range: if lo.is_finite() && hi.is_finite() {
                hi - lo
            } else {
                0.0
            },
            range_low: lo,
            range_high: hi,
            true_peak: tp,
            peak_to_short_term: if s.is_finite() && tp.is_finite() {
                (tp - s).max(0.0)
            } else {
                0.0
            },
            peak_to_integrated: if i.is_finite() && self.peak > 0.0 {
                (max_tp - i).max(0.0)
            } else {
                0.0
            },
            incomplete: self.incomplete,
        }
    }
}
