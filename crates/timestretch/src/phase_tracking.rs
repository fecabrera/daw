//! Peak continuity across successive synthesis frames.
use crate::{Error, buffer, spectral::wrap};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Default)]
struct Peak {
    power: f64,
    frequency: f64,
    weight: f64,
    age: u8,
}

pub(super) struct PhaseFrame<'a> {
    pub peaks: &'a [usize],
    pub powers: &'a [f64],
    pub frequencies: &'a [f64],
    pub previous_rotations: &'a [f64],
    pub rotations: &'a mut [f64],
    // Skip phase carry at discontinuities and inside or just after a guard.
    pub analysis_advance: Option<f64>,
}

pub(super) struct PhaseTracker {
    previous_peaks: Vec<usize>,
    previous: Vec<Peak>,
    next: Vec<Peak>,
    hop: f64,
    bin_width: f64,
    smoothing: f64,
    acquire_step: f64,
    release_step: f64,
}

impl PhaseTracker {
    pub(super) fn new(
        bins: usize,
        width: usize,
        hop: usize,
        sample_rate: u32,
    ) -> Result<Self, Error> {
        let mut previous_peaks = Vec::new();
        previous_peaks
            .try_reserve_exact(bins)
            .map_err(|_| Error::ResourceLimit)?;
        let hop = hop as f64;
        let rate = f64::from(sample_rate);
        Ok(Self {
            previous_peaks,
            previous: buffer(bins, Peak::default())?,
            next: buffer(bins, Peak::default())?,
            hop,
            bin_width: TAU / width as f64,
            smoothing: 1.0 - (-(hop / (0.020 * rate))).exp(),
            acquire_step: (hop / (0.030 * rate)).min(1.0 / 3.0),
            release_step: (hop / (0.040 * rate)).min(1.0 / 3.0),
        })
    }

    pub(super) fn update(&mut self, frame: PhaseFrame<'_>) {
        for &bin in frame.peaks {
            self.next[bin] = Peak {
                power: frame.powers[bin],
                frequency: frame.frequencies[bin],
                age: 1,
                weight: 0.0,
            };
        }
        if let Some(advance) = frame
            .analysis_advance
            .filter(|_| !self.previous_peaks.is_empty())
        {
            let frame_peak = frame
                .peaks
                .iter()
                .map(|&bin| frame.powers[bin])
                .fold(0.0, f64::max);
            let stretch_advance = self.hop - advance;
            for (index, &bin) in frame.peaks.iter().enumerate() {
                let Some(old) = self.match_peak(frame.peaks, index, bin) else {
                    continue;
                };
                let prior = self.previous[old];
                let peak = &mut self.next[bin];
                let delta = (peak.frequency - prior.frequency).abs();
                let confident = old.abs_diff(bin) <= 1
                    && peak.power >= frame_peak * 1e-4
                    && peak.power >= prior.power * 0.25
                    && peak.power <= prior.power * 4.0
                    && delta <= 0.3 * self.bin_width;
                let weight = if confident {
                    (prior.weight + self.acquire_step).min(1.0)
                } else if delta <= self.bin_width {
                    (prior.weight - self.release_step).max(0.0)
                } else {
                    0.0
                };
                if weight == 0.0 {
                    continue;
                }
                peak.weight = weight;
                if confident {
                    peak.age = prior.age.saturating_add(1);
                }
                let stable_limit = (0.12 * self.bin_width).min(prior.frequency.abs() * 0.006);
                if peak.age >= 3 && delta <= stable_limit {
                    peak.frequency =
                        prior.frequency + self.smoothing * (peak.frequency - prior.frequency);
                }
                let continued =
                    wrap(frame.previous_rotations[old] + peak.frequency * stretch_advance);
                let rotation = &mut frame.rotations[bin];
                *rotation = if weight == 1.0 {
                    continued
                } else {
                    wrap(*rotation + weight * wrap(continued - *rotation))
                };
            }
        }
        std::mem::swap(&mut self.previous, &mut self.next);
        self.previous_peaks.clear();
        self.previous_peaks.extend_from_slice(frame.peaks);
    }

    fn match_peak(&self, peaks: &[usize], index: usize, bin: usize) -> Option<usize> {
        let insertion = self.previous_peaks.partition_point(|&prior| prior < bin);
        let mut neighbors = self.previous_peaks
            [insertion.saturating_sub(1)..(insertion + 1).min(self.previous_peaks.len())]
            .iter();
        let &first = neighbors.next()?;
        let old = neighbors
            .find(|&&candidate| candidate.abs_diff(bin) < first.abs_diff(bin))
            .copied()
            .unwrap_or(first);
        let distance = old.abs_diff(bin);
        // Require an unambiguous nearest match in both directions. One more
        // bin is allowed for release after an established track loses confidence.
        if distance > 2
            || (index > 0 && peaks[index - 1].abs_diff(old) <= distance)
            || (index + 1 < peaks.len() && peaks[index + 1].abs_diff(old) <= distance)
        {
            return None;
        }
        Some(old)
    }
}
