//! Recover pulse evidence from changes within the broad energy bands.
use super::{MAX_SECONDS, Tempo, TempoWindow, recurrence::Vote};
use crate::Error;
use rustfft::{FftPlanner, num_complex::Complex};

const FFT: usize = 128;
const MAX_REDUCED_RATE: usize = 2400;
const FIRST_BIN: usize = 3;
const LAST_BIN: usize = 56;
const BANDS: usize = LAST_BIN - FIRST_BIN + 1;

/// A box-filtered stereo trace on the source clock. Keeping both channels
/// preserves spectral changes in anti-phase audio.
pub(super) struct Input {
    rate: u32,
    divisor: usize,
    frames: Vec<[f32; 2]>,
    limit: usize,
    pending: [f32; 2],
    remaining: usize,
}

impl Input {
    pub(super) fn new(rate: u32, frames: u64) -> Self {
        let divisor = match rate {
            44_100 | 48_000 => 20,
            88_200 | 96_000 => 40,
            _ => 80,
        };
        let limit = frames.min(MAX_SECONDS * u64::from(rate)) as usize / divisor;
        Self {
            rate,
            divisor,
            frames: Vec::with_capacity(limit),
            limit,
            pending: [0.0; 2],
            remaining: divisor,
        }
    }

    pub(super) fn push(&mut self, mut samples: &[[f32; 2]]) {
        let gain = 1.0 / self.divisor as f32;
        while !samples.is_empty() && self.frames.len() < self.limit {
            let count = samples.len().min(self.remaining);
            for &[left, right] in &samples[..count] {
                self.pending[0] += left * gain;
                self.pending[1] += right * gain;
            }
            samples = &samples[count..];
            self.remaining -= count;
            if self.remaining == 0 {
                self.frames.push(self.pending);
                self.remaining = self.divisor;
                self.pending = [0.0; 2];
            }
        }
    }

    pub(super) fn estimate(
        &self,
        candidate: Option<f64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Vote, Error> {
        if self.frames.len() < self.rate as usize / self.divisor {
            return Ok(Vote::Weak);
        }
        let novelty = self.novelty(cancelled)?;
        let Some(phase) = PhaseLandmarks::new(&novelty) else {
            return Ok(Vote::Weak);
        };
        let mut detector = TempoWindow::new(self.rate)?;
        detector.novelty = novelty;
        let result = detector.score(None, cancelled)?;
        if result.range_veto {
            return Ok(Vote::Veto);
        }
        if let Some(bpm) = candidate
            && phase.supports(bpm)
            && detector.supports_loop(bpm, cancelled)?
            && !matches!(result.tempo, Tempo::Estimate { bpm: rival, .. }
                if ![0.5, 1.0, 2.0].into_iter().any(|scale| ((rival*scale)/bpm-1.0).abs() <= 0.03))
        {
            return Ok(Vote::Estimate(bpm));
        }
        let (bpm, vote) = match result.tempo {
            Tempo::Estimate { bpm, .. } => (bpm, Vote::Estimate(bpm)),
            Tempo::OctaveAmbiguous { bpm, .. } => (bpm, Vote::OctaveAmbiguous(bpm)),
            _ => return Ok(Vote::Weak),
        };
        if phase.supports(bpm) && detector.supports_loop(bpm, cancelled)? {
            Ok(vote)
        } else {
            Ok(Vote::Weak)
        }
    }

    fn novelty(&self, cancelled: &impl Fn() -> bool) -> Result<Vec<f64>, Error> {
        let mut peak = 0.0_f32;
        for sample in &self.frames {
            if !sample[0].is_finite() || !sample[1].is_finite() {
                return Ok(Vec::new());
            }
            peak = peak.max(sample[0].abs()).max(sample[1].abs());
        }
        if peak < 1e-7 {
            return Ok(Vec::new());
        }
        let gain = 1.0 / peak.max(1.0);
        let plan = FftPlanner::<f32>::new().plan_fft_forward(FFT);
        let window: [f32; FFT] = std::array::from_fn(|i| {
            0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT - 1) as f32).cos()
        });
        let mut fft = [Complex::default(); FFT];
        let mut scratch = vec![Complex::default(); plan.get_inplace_scratch_len()];
        let mut previous = [0.0_f32; BANDS];
        let mut floor = [0.0_f32; BANDS];
        let bins = self.frames.len() * self.divisor / (self.rate as usize / 100);
        let mut novelty = vec![0.0_f64; bins];
        for (bin, value) in novelty.iter_mut().enumerate() {
            if bin % 32 == 0 && cancelled() {
                return Err(Error::Cancelled);
            }
            let center = bin * (self.rate as usize / 100) / self.divisor;
            if center < FFT / 2 || center + FFT / 2 >= self.frames.len() {
                continue;
            }
            let samples = &self.frames[center - FFT / 2..center + FFT / 2];
            for ((dst, &[left, right]), weight) in fft.iter_mut().zip(samples).zip(window) {
                *dst = Complex::new(left * (gain * weight), right * (gain * weight));
            }
            plan.process_with_scratch(&mut fft, &mut scratch);
            let mut flux = 0.0;
            for (band, (previous, floor)) in previous.iter_mut().zip(&mut floor).enumerate() {
                let k = band + FIRST_BIN;
                // The packed stereo transform gives total power from a conjugate pair.
                let level = (fft[k].norm_sqr() + fft[FFT - k].norm_sqr()).sqrt();
                let motion = level - *previous;
                let rise = motion.max(-0.32 * motion);
                let ratio = (rise / floor.max(1e-7)).min(12.0);
                flux += ratio / (1.0 + 0.30 * ratio) * level.sqrt();
                *previous = level;
                *floor += 0.075 * (level - *floor);
            }
            *value = f64::from(flux);
        }
        Ok(novelty)
    }
}

// Three stereo captures, three coarse histories, and the adapter's PCM and
// file-byte staging blocks. The remaining admission covers one window scorer.
const _: () = assert!(
    3 * MAX_SECONDS as usize * MAX_REDUCED_RATE * size_of::<[f32; 2]>()
        + 3 * super::MAX_BINS * size_of::<[f64; 3]>()
        + 2 * 8192 * size_of::<[f32; 2]>()
        < 900 * 1024
);

const PHASE_LANDMARKS: usize = 12;

struct PhaseLandmarks([[(usize, f64); PHASE_LANDMARKS]; 2]);

impl PhaseLandmarks {
    fn new(novelty: &[f64]) -> Option<Self> {
        if novelty.len() < 80 {
            return None;
        }
        // Limit any single attack's influence on the shared phase.
        let split = novelty.len() / 2;
        let mut peaks = [[(0_usize, 0.0_f64); PHASE_LANDMARKS]; 2];
        for (range, list) in [8..split, split..novelty.len() - 8]
            .into_iter()
            .zip(&mut peaks)
        {
            let mut strongest = 0.0_f64;
            for i in 0..list.len() {
                let mut best = (0, 0.0);
                for index in range.clone() {
                    let value = novelty[index];
                    if value > best.1
                        && value > novelty[index - 1]
                        && value >= novelty[index + 1]
                        && list[..i].iter().all(|&(prev, _)| index.abs_diff(prev) >= 6)
                    {
                        best = (index, value);
                    }
                }
                if i == 0 {
                    strongest = best.1;
                }
                if best.1 < 1e-6 || (i < 3 && best.1 < strongest * 0.10) {
                    if i < 3 {
                        return None;
                    }
                    break;
                }
                list[i] = best;
            }
        }
        Some(Self(peaks))
    }

    fn supports(&self, bpm: f64) -> bool {
        let period = 6000.0 / bpm;
        [2.0, 4.0].into_iter().any(|subdivision| {
            let spacing = period / subdivision;
            let fit = self.agreement(spacing);
            fit >= 0.22
                && fit
                    >= self
                        .agreement(spacing * 0.936)
                        .max(self.agreement(spacing * 1.068))
                        + 0.04
        })
    }

    fn agreement(&self, spacing: f64) -> f64 {
        let radians_per_bin = std::f64::consts::TAU / spacing;
        let [left, right] = self.0.map(|landmarks| {
            let mut sum = [0.0; 3];
            for (position, value) in landmarks {
                let weight = value.sqrt();
                let (sin, cos) = (position as f64 * radians_per_bin).sin_cos();
                sum[0] += weight * cos;
                sum[1] += weight * sin;
                sum[2] += weight;
            }
            sum
        });
        let amplitude_a = left[0].hypot(left[1]);
        let amplitude_b = right[0].hypot(right[1]);
        let coherence =
            (left[0] * right[0] + left[1] * right[1]) / (amplitude_a * amplitude_b).max(1e-12);
        coherence.max(0.0) * (amplitude_a / left[2]).min(amplitude_b / right[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationary_tone_and_silence_do_not_confirm_source_length() {
        let rate = 48_000;
        let frames = 4 * u64::from(rate);
        for gain in [0.0, 0.2] {
            let mut input = Input::new(rate, frames);
            for (block, start) in (0..frames).step_by(1200).enumerate() {
                let samples = std::array::from_fn::<_, 1200, _>(|offset| {
                    let time = (start + offset as u64) as f32 / rate as f32;
                    let sample = gain * (std::f32::consts::TAU * 397.0 * time).sin();
                    [sample, -sample]
                });
                input.push(&samples);
                assert_eq!(input.frames.len(), (block + 1) * 60);
            }
            assert_eq!(input.estimate(Some(120.0), &|| false), Ok(Vote::Weak));
            if gain > 0.0 {
                assert_eq!(input.estimate(Some(120.0), &|| true), Err(Error::Cancelled));
            }
        }
    }
}
