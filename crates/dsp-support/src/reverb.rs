//! Prepared reverb signal blocks. No host or unit settings.
mod drive;
mod filters;
mod makeup;
mod processor;
mod tap_transition;

pub use processor::{Parameters, Reverb};
pub use tap_transition::TapTransition;

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
pub struct Delay(DelayRing<2>);

impl Delay {
    /// Reserve at least one sample of delay plus interpolation guard storage.
    pub fn new(maximum_frames: usize) -> Self {
        Self(DelayRing::new(maximum_frames.max(1).saturating_add(2)))
    }
    pub fn heap_bytes(&self) -> usize {
        self.0.heap_bytes()
    }
    pub fn reset(&mut self) {
        self.0.reset();
    }
    pub fn read(&self, delay_frames: f64) -> [f64; 2] {
        self.0.read(if delay_frames.is_finite() {
            delay_frames
        } else {
            1.0
        })
    }
    pub fn write(&mut self, sample: [f64; 2]) {
        self.0
            .write(sample.map(|v| if v.is_finite() { v } else { 0.0 }));
    }
}

/// Prepared fractional delay ring. Callers own input sanitization policy.
#[derive(Clone)]
pub struct DelayRing<const CHANNELS: usize> {
    samples: Vec<[f64; CHANNELS]>,
    cursor: usize,
    valid_frames: usize,
}

impl<const CHANNELS: usize> DelayRing<CHANNELS> {
    /// Allocate the prepared capacity including the two interpolation guard frames.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity >= 3);
        Self {
            samples: vec![[0.0; CHANNELS]; capacity],
            cursor: 0,
            valid_frames: 0,
        }
    }
    pub fn heap_bytes(&self) -> usize {
        self.samples.capacity() * size_of::<[f64; CHANNELS]>()
    }
    /// Logical clearing is constant time; old samples remain inaccessible.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.valid_frames = 0;
    }
    pub fn read(&self, delay_frames: f64) -> [f64; CHANNELS] {
        let delay = delay_frames.clamp(1.0, (self.samples.len() - 2) as f64);
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
    pub fn write(&mut self, sample: [f64; CHANNELS]) {
        self.samples[self.cursor] = sample;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slope {
    Db6,
    Db12,
    Db24,
}
impl Slope {
    pub const ALL: [Self; 3] = [Self::Db6, Self::Db12, Self::Db24];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveMode {
    Soft,
    Hard,
    Tube,
}
impl DriveMode {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Hard, Self::Tube];
}

pub mod shimmer;
