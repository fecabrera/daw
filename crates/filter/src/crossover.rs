//! Shared stereo fourth-order crossover; backend nodes remain private.
use fundsp::{
    audionode::AudioNode,
    svf::{FixedSvf, HighpassMode, LowpassMode, SvfParams},
};
/// Two cascaded Butterworth sections on each branch: fourth-order Linkwitz–Riley.
#[derive(Clone)]
pub struct LinkwitzRileySplit {
    low: [[FixedSvf<f64, LowpassMode<f64>>; 2]; 2],
    high: [[FixedSvf<f64, HighpassMode<f64>>; 2]; 2],
    frequency: f32,
}
impl LinkwitzRileySplit {
    /// Current boundary in Hz, after the caller's control smoothing.
    pub fn frequency_hz(&self) -> f32 {
        self.frequency
    }

    pub fn new(rate: u32, frequency: f32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(rate),
            cutoff: f64::from(frequency),
            q: std::f64::consts::FRAC_1_SQRT_2,
            gain: 1.0,
        };
        Self {
            low: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(LowpassMode::new(), &params))
            }),
            high: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(HighpassMode::new(), &params))
            }),
            frequency,
        }
    }
    pub fn configure(&mut self, frequency: f32) {
        if self.frequency != frequency {
            self.frequency = frequency;
            for channel in &mut self.low {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
            for channel in &mut self.high {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
        }
    }
    pub fn sample(&mut self, input: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let low = std::array::from_fn(|ch| {
            self.low[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        let high = std::array::from_fn(|ch| {
            self.high[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        (low, high)
    }
    pub fn reset(&mut self) {
        for channel in &mut self.low {
            for node in channel {
                node.reset();
            }
        }
        for channel in &mut self.high {
            for node in channel {
                node.reset();
            }
        }
    }
}

/// Prepared Linkwitz–Riley splits and phase compensation for an ordered band bank.
/// A shorter active prefix leaves dormant filter histories intact.
#[derive(Clone)]
pub struct CrossoverBank {
    // [current log Hz, target log Hz] share one clock to preserve boundary order.
    frequencies: Vec<[f32; 2]>,
    crossover_left: u32,
    splits: Vec<LinkwitzRileySplit>,
    compensation: Vec<Vec<LinkwitzRileySplit>>,
}

impl CrossoverBank {
    pub fn new(rate: u32, frequencies: &[f32], band_count: usize) -> Self {
        debug_assert_eq!(frequencies.len(), band_count.saturating_sub(1));
        Self {
            splits: frequencies
                .iter()
                .map(|f| LinkwitzRileySplit::new(rate, *f))
                .collect(),
            compensation: (0..band_count)
                .map(|i| {
                    frequencies
                        .iter()
                        .skip(i + 1)
                        .map(|f| LinkwitzRileySplit::new(rate, *f))
                        .collect()
                })
                .collect(),
            frequencies: frequencies.iter().map(|f| [f.ln(); 2]).collect(),
            crossover_left: 0,
        }
    }

    /// Current smoothed boundaries in natural-log Hz.
    pub fn current_log_cutoffs(&self) -> impl Iterator<Item = f32> + '_ {
        self.frequencies.iter().map(|boundary| boundary[0])
    }

    pub fn retarget(&mut self, frequencies: impl Iterator<Item = f32> + Clone, frames: u32) {
        if self
            .frequencies
            .iter()
            .zip(frequencies.clone())
            .any(|(f, hz)| f[1] != hz.ln())
        {
            for (f, hz) in self.frequencies.iter_mut().zip(frequencies) {
                f[1] = hz.ln();
            }
            self.restart(frames);
        }
    }

    pub fn set_target(&mut self, boundary: usize, hz: f32) {
        self.frequencies[boundary][1] = hz.ln();
    }

    pub fn restart(&mut self, frames: u32) {
        self.crossover_left = frames;
    }

    pub fn inherit(&mut self, previous: &mut Self, frames: u32) {
        if self.splits.len() == previous.splits.len() {
            std::mem::swap(&mut self.splits, &mut previous.splits);
            std::mem::swap(&mut self.compensation, &mut previous.compensation);
            let unchanged = self
                .frequencies
                .iter()
                .zip(&previous.frequencies)
                .all(|(a, b)| a[1] == b[1]);
            for (next, old) in self.frequencies.iter_mut().zip(&previous.frequencies) {
                next[0] = old[0];
            }
            self.crossover_left = if unchanged {
                previous.crossover_left
            } else {
                frames
            };
        }
    }

    pub fn advance(&mut self, active_bands: usize) {
        for (i, f) in self
            .frequencies
            .iter_mut()
            .take(active_bands.saturating_sub(1))
            .enumerate()
        {
            if self.crossover_left > 0 {
                f[0] += (f[1] - f[0]) / self.crossover_left as f32;
            }
            let frequency = f[0].exp();
            self.splits[i].configure(frequency);
            // A lower branch receives the same smoothed boundary as the later split.
            for (row_index, row) in self
                .compensation
                .iter_mut()
                .take(active_bands.saturating_sub(1))
                .enumerate()
                .take(i)
            {
                row[i - row_index - 1].configure(frequency);
            }
        }
        self.crossover_left = self.crossover_left.saturating_sub(1);
    }

    pub fn sample_band(
        &mut self,
        rest: &mut [f32; 2],
        band: usize,
        active_bands: usize,
    ) -> [f32; 2] {
        let mut sample = if band + 1 < active_bands {
            let (low, high) = self.splits[band].sample(*rest);
            *rest = high;
            low
        } else {
            *rest
        };
        for split in self.compensation[band]
            .iter_mut()
            .take(active_bands.saturating_sub(band + 2))
        {
            let (low, high) = split.sample(sample);
            sample = std::array::from_fn(|ch| low[ch] + high[ch]);
        }
        sample
    }

    pub fn reset(&mut self) {
        for f in &mut self.frequencies {
            f[0] = f[1];
        }
        self.crossover_left = 0;
        for s in &mut self.splits {
            s.reset();
        }
        for s in self.compensation.iter_mut().flatten() {
            s.reset();
        }
    }

    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.frequencies)
            + daw_unit_core::memory::vec_bytes(&self.splits)
            + daw_unit_core::memory::vec_bytes(&self.compensation)
            + self
                .compensation
                .iter()
                .map(daw_unit_core::memory::vec_bytes)
                .sum::<u64>()
    }
}
