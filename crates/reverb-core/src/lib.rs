//! Portable reverb building blocks. No host, project, device, or UI state.
//!
//! Equations follow the supplied behavioral reports. This is original Rust code;
//! no captured machine code, decompiled functions, or runtime tables are embedded.
mod drive;
mod filters;
mod processor;
pub use processor::{Parameters, Reverb};

/// A monotonic three-point control map: endpoints `a`, `b`, midpoint `c`.
pub fn rational_map(u: f64, a: f64, c: f64, b: f64) -> f64 {
    let u = if u.is_finite() {
        u.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if a == b || u == 0.0 {
        return a;
    }
    if u == 1.0 {
        return b;
    }
    let c = c.clamp(a.min(b), a.max(b));
    let denominator = (b - c) + (2.0 * c - a - b) * u;
    if denominator == 0.0 {
        c
    } else {
        a + (b - a) * (c - a) * u / denominator
    }
}

/// Endpoint-relative dry/wet gains. Plate uses a distinct intermediate map.
pub fn mix_gains(mix: f64, plate: bool) -> [f64; 2] {
    let m = rational_map(mix, 0.0, if plate { 0.33 } else { 0.5 }, 1.0);
    [3.0 * (1.0 - m) / (3.0 - 2.0 * m), 3.0 * m / (1.0 + 2.0 * m)]
}

/// Width affects only the wet Side component.
pub fn wet_width(wet: [f64; 2], width: f64) -> [f64; 2] {
    let mid = wet[0] * 0.5 + wet[1] * 0.5;
    let side = (wet[0] * 0.5 - wet[1] * 0.5) * width.clamp(0.0, 1.0);
    [mid + side, mid - side]
}

/// Coupled diffusion junction. This is deliberately not an isolated all-pass.
// The documented junction uses the rounded coefficient, not 1/sqrt(2).
#[allow(clippy::approx_constant)]
pub fn junction(input: [f64; 2]) -> [f64; 2] {
    [
        0.7071 * (input[0] + input[1]),
        0.7071 * (input[0] - input[1]),
    ]
}

/// Four independent, deterministic lanes; seeds are supplied by the product.
#[derive(Clone)]
pub struct Noise {
    initial: [u32; 4],
    state: [u32; 4],
}
impl Noise {
    pub fn new(seeds: [u32; 4]) -> Self {
        Self {
            initial: seeds,
            state: seeds,
        }
    }
    pub fn reset(&mut self) {
        self.state = self.initial;
    }
    pub fn sample(&mut self) -> [f32; 4] {
        self.state.each_mut().map(|state| {
            *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f32::from_bits((*state & 0x007f_ffff) | 0x4000_0000) - 3.0
        })
    }
}

/// Bounded stereo fractional delay. Construction and clearing belong off audio.
#[derive(Clone)]
pub struct Delay {
    samples: Vec<[f64; 2]>,
    cursor: usize,
    valid_frames: usize,
}
impl Delay {
    /// Reserve at least one sample of delay plus interpolation guard storage.
    pub fn new(maximum_frames: usize) -> Self {
        Self {
            samples: vec![[0.0; 2]; maximum_frames.max(1).saturating_add(2)],
            cursor: 0,
            valid_frames: 0,
        }
    }
    pub fn heap_bytes(&self) -> usize {
        self.samples.capacity() * size_of::<[f64; 2]>()
    }
    /// Logical clearing is constant time; old samples remain inaccessible.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.valid_frames = 0;
    }
    pub fn read(&self, delay_frames: f64) -> [f64; 2] {
        let delay = if delay_frames.is_finite() {
            delay_frames
        } else {
            1.0
        }
        .clamp(1.0, (self.samples.len() - 2) as f64);
        let whole = delay.floor() as usize;
        let fraction = delay - whole as f64;
        let a = (self.cursor + self.samples.len() - whole) % self.samples.len();
        let b = (a + self.samples.len() - 1) % self.samples.len();
        std::array::from_fn(|ch| {
            let recent = if whole <= self.valid_frames {
                self.samples[a][ch]
            } else {
                0.0
            };
            let older = if whole < self.valid_frames {
                self.samples[b][ch]
            } else {
                0.0
            };
            recent * (1.0 - fraction) + older * fraction
        })
    }
    pub fn write(&mut self, sample: [f64; 2]) {
        self.samples[self.cursor] = sample.map(|v| if v.is_finite() { v } else { 0.0 });
        self.cursor = (self.cursor + 1) % self.samples.len();
        self.valid_frames = (self.valid_frames + 1).min(self.samples.len());
    }
}

/// Prepared prime table and Bloom delay mapping. Table creation is worker-only.
#[derive(Clone)]
pub struct BloomDelays {
    primes: Vec<u32>,
}
impl Default for BloomDelays {
    fn default() -> Self {
        let mut primes = Vec::with_capacity(10_000);
        let mut candidate = 2_u32;
        while primes.len() < 10_000 {
            if primes
                .iter()
                .take_while(|&&p| p <= candidate / p)
                .all(|p| !candidate.is_multiple_of(*p))
            {
                primes.push(candidate);
            }
            candidate += 1;
        }
        Self { primes }
    }
}
impl BloomDelays {
    /// Millisecond controls map through a zero-based prime-table lookup.
    pub fn frames(&self, bloom: f64, decay_raw: f64, sample_rate: u32) -> [u32; 16] {
        let scale = rational_map(bloom, 8.5, 150.0, 300.0);
        let distribution = rational_map(bloom, 1.0, 1.42, 2.0);
        let divisor = rational_map(decay_raw, 24.0, 20.0, 16.0);
        std::array::from_fn(|i| {
            let samples =
                scale * ((i + 1) as f64 / divisor).powf(distribution) * f64::from(sample_rate)
                    / 1000.0;
            let index = if samples > 1.0 {
                (samples / (2.03289 * samples.log10())) as usize
            } else {
                0
            };
            self.primes[index.min(self.primes.len() - 1)]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_maps_preserve_reference_midpoints_and_mix() {
        for (a, c, b) in [(0.3, 3.0, 15.0), (1.0, 7.5, 60.0), (24.0, 20.0, 16.0)] {
            assert_eq!(rational_map(0.0, a, c, b), a);
            assert!((rational_map(0.5, a, c, b) - c).abs() < 1e-12);
            assert_eq!(rational_map(1.0, a, c, b), b);
        }
        assert_eq!(mix_gains(0.0, false), [1.0, 0.0]);
        assert_eq!(mix_gains(1.0, true), [0.0, 1.0]);
        assert_eq!(mix_gains(0.5, false), [0.75, 0.75]);
        let plate = mix_gains(0.5, true);
        assert!((plate[0] - 0.858974358974359).abs() < 1e-12);
        assert!((plate[1] - 0.5963855421686747).abs() < 1e-12);
    }
    #[test]
    fn prime_mapping_matches_cached_delays_and_all_project_rate_bounds() {
        let table = BloomDelays::default();
        assert_eq!(
            table.frames(0.5, 0.5, 44100),
            [
                89, 239, 433, 647, 887, 1163, 1453, 1753, 2087, 2417, 2777, 3181, 3547, 3929, 4349,
                4789
            ]
        );
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for bloom in [0.0, 0.5, 1.0] {
                for decay in [0.0, 0.5, 1.0] {
                    let frames = table.frames(bloom, decay, rate);
                    assert!(frames.windows(2).all(|w| w[0] <= w[1]));
                    assert!(frames.iter().all(|f| *f > 0 && *f <= 104729));
                }
            }
        }
    }
    #[test]
    fn delay_wrap_interpolation_and_reset_preserve_timing() {
        let mut d = Delay::new(4);
        for i in 0..30 {
            d.write([f64::from(i), -f64::from(i)]);
        }
        assert_eq!(d.read(1.0), [29.0, -29.0]);
        assert_eq!(d.read(3.5), [26.5, -26.5]);
        d.reset();
        assert_eq!(d.read(4.0), [0.0; 2]);
    }
    #[test]
    fn noise_reset_is_repeatable_and_lanes_stay_in_range() {
        let mut n = Noise::new([1, 2, 3, 4]);
        let first = n.sample();
        for _ in 0..10000 {
            assert!(n.sample().iter().all(|x| (-1.0..1.0).contains(x)));
        }
        n.reset();
        assert_eq!(n.sample(), first);
        assert_ne!(first[0], first[1]);
    }
    #[test]
    fn width_changes_only_side() {
        assert_eq!(wet_width([1.0, -1.0], 0.0), [0.0; 2]);
        assert_eq!(wet_width([0.5, 0.5], 0.0), [0.5; 2]);
        assert_eq!(wet_width([0.5, -0.25], 1.0), [0.5, -0.25]);
    }
}
