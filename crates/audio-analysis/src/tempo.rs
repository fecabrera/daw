//! Bounded source-clock pulse estimation. The caller supplies prepared stereo PCM.
use crate::{Error, SAMPLE_RATES};

const MAX_SECONDS: u64 = 12;
const BINS_PER_SECOND: u32 = 100;
const MIN_BPM: f64 = 45.0;
const MAX_BPM: f64 = 240.0;
const MAX_BINS: usize = MAX_SECONDS as usize * BINS_PER_SECOND as usize + 1;

mod recurrence;
mod spectral;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tempo {
    Estimate { bpm: f64, sampled: bool },
    OctaveAmbiguous { bpm: f64, sampled: bool },
    Weak,
    Ambiguous,
}

impl Tempo {
    /// Reference BPM when choosing between supported half and double rates.
    pub const PREFERRED_OCTAVE_BPM: f64 = 100.0;
}

/// A completed window, with level history retained only for eligible weak results.
pub struct TempoEvidence {
    score: WindowScore,
    levels: Vec<[f64; 3]>,
    spectral: Option<spectral::Input>,
}

struct WindowScore {
    tempo: Tempo,
    loop_support: bool,
    range_veto: bool,
}

impl WindowScore {
    fn new(tempo: Tempo) -> Self {
        Self {
            tempo,
            loop_support: false,
            range_veto: false,
        }
    }
}

pub struct TempoWindows {
    pub ranges: Vec<std::ops::Range<u64>>,
    pub sampled: bool,
    loop_candidate: Option<f64>,
}

impl TempoWindows {
    pub fn new(rate: u32, frames: u64) -> Result<Self, Error> {
        if !SAMPLE_RATES.contains(&rate) {
            return Err(Error::InvalidRate);
        }
        let width = u64::from(rate) * MAX_SECONDS;
        let sampled = frames > 3 * width;
        let ranges = if sampled {
            [0, (frames - width) / 2, frames - width]
                .map(|start| start..start + width)
                .into()
        } else {
            let count = frames.div_ceil(width).max(1);
            (0..count)
                .map(|i| i * frames / count..(i + 1) * frames / count)
                .collect()
        };
        // Complete sources can span a power-of-two number of beats. Pick one
        // representative of the half/double family, then require audio support.
        let loop_candidate = if sampled || frames == 0 {
            None
        } else {
            [4, 8, 16, 32, 64]
                .into_iter()
                .map(|beats| f64::from(beats * 60) * f64::from(rate) / frames as f64)
                .find(|bpm| (80.0..160.0).contains(bpm))
        };
        Ok(Self {
            ranges,
            sampled,
            loop_candidate,
        })
    }

    pub fn loop_candidate(&self) -> Option<f64> {
        self.loop_candidate
    }

    pub fn combine_with_loop(&self, windows: &[Tempo], supported: &[bool]) -> Tempo {
        let result = self.combine(windows);
        let Some(candidate) = self.loop_candidate else {
            return result;
        };
        if result != Tempo::Weak
            || windows.len() != self.ranges.len()
            || windows.len() != supported.len()
        {
            return result;
        }
        let mut agreeing = 0;
        for (&window, &supports_loop) in windows.iter().zip(supported) {
            match window {
                Tempo::Estimate { bpm, .. } => {
                    if ![0.5, 1.0, 2.0]
                        .into_iter()
                        .any(|factor| ((bpm * factor) / candidate - 1.0).abs() <= 0.03)
                    {
                        return result;
                    }
                    agreeing += 1;
                }
                Tempo::Weak if supports_loop => agreeing += 1,
                _ => {}
            }
        }
        if agreeing >= self.ranges.len().min(2) {
            Tempo::Estimate {
                bpm: candidate,
                sampled: false,
            }
        } else {
            result
        }
    }

    /// Preserve the onset and duration results before scoring retained weak windows.
    pub fn combine_evidence(
        &self,
        windows: &[TempoEvidence],
        cancelled: &impl Fn() -> bool,
    ) -> Result<Tempo, Error> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let mut votes = [Tempo::Weak; 3];
        let mut loop_support = [false; 3];
        if windows.len() != self.ranges.len() || windows.len() > votes.len() {
            return Ok(Tempo::Weak);
        }
        let votes = &mut votes[..windows.len()];
        for (i, window) in windows.iter().enumerate() {
            votes[i] = window.score.tempo;
            loop_support[i] = window.score.loop_support;
        }
        let result = self.combine_with_loop(votes, &loop_support[..windows.len()]);
        let range_veto = windows.iter().any(|window| window.score.range_veto);
        if range_veto && matches!(result, Tempo::OctaveAmbiguous { .. }) {
            return Ok(Tempo::Ambiguous);
        }
        if result != Tempo::Weak || range_veto {
            return Ok(result);
        }
        let mut recovered = false;
        for (vote, window) in votes.iter_mut().zip(windows) {
            if *vote != Tempo::Weak || window.levels.is_empty() {
                continue;
            }
            match recurrence::estimate(&window.levels, self.loop_candidate, cancelled)? {
                recurrence::Vote::Weak => {}
                recurrence::Vote::Veto => return Ok(Tempo::Weak),
                estimate => {
                    *vote = estimate.into_tempo();
                    recovered = true;
                }
            }
        }
        let result = if recovered {
            self.combine(votes)
        } else {
            result
        };
        if result != Tempo::Weak {
            return Ok(result);
        }
        let mut spectral_vote = false;
        for (vote, window) in votes.iter_mut().zip(windows) {
            let Some(input) = &window.spectral else {
                continue;
            };
            if *vote != Tempo::Weak {
                continue;
            }
            match input.estimate(self.loop_candidate, cancelled)? {
                recurrence::Vote::Weak => {}
                recurrence::Vote::Veto => return Ok(result),
                estimate => {
                    *vote = estimate.into_tempo();
                    spectral_vote = true;
                }
            }
        }
        Ok(if spectral_vote {
            self.combine(votes)
        } else {
            result
        })
    }

    pub fn combine(&self, windows: &[Tempo]) -> Tempo {
        if windows.len() != self.ranges.len() {
            return Tempo::Weak;
        }
        if windows.contains(&Tempo::Ambiguous) {
            return Tempo::Ambiguous;
        }
        let has_octave_choice = windows
            .iter()
            .any(|window| matches!(window, Tempo::OctaveAmbiguous { .. }));
        let mut periods: Vec<_> = windows
            .iter()
            .filter_map(|result| match result {
                Tempo::Estimate { bpm, .. } | Tempo::OctaveAmbiguous { bpm, .. } => {
                    Some(60.0 / bpm)
                }
                _ => None,
            })
            .collect();
        periods.sort_by(f64::total_cmp);
        let disagree = periods
            .first()
            .zip(periods.last())
            .is_some_and(|(lo, hi)| hi > &(lo * 1.03));
        if has_octave_choice || disagree {
            if periods.len() < self.ranges.len().min(2) {
                return Tempo::Ambiguous;
            }
            let rates = windows.iter().filter_map(|window| match window {
                Tempo::Estimate { bpm, .. } | Tempo::OctaveAmbiguous { bpm, .. } => Some(*bpm),
                _ => None,
            });
            let preferred = rates
                .clone()
                .reduce(closest_to_octave_reference)
                .expect("at least one vote");
            let mut low = preferred;
            let mut high = preferred;
            for rate in rates {
                let aligned = if is_half_double(rate, preferred) {
                    if rate < preferred {
                        rate * 2.0
                    } else {
                        rate * 0.5
                    }
                } else {
                    rate
                };
                low = low.min(aligned);
                high = high.max(aligned);
            }
            if tempo_ratio(low, high) <= 1.03 {
                return Tempo::OctaveAmbiguous {
                    bpm: preferred,
                    sampled: self.sampled,
                };
            }
            return Tempo::Ambiguous;
        }
        if periods.len() < self.ranges.len().min(2) {
            return Tempo::Weak;
        }
        let middle = periods.len() / 2;
        let period = (periods[(periods.len() - 1) / 2] + periods[middle]) / 2.0;
        Tempo::Estimate {
            bpm: 60.0 / period,
            sampled: self.sampled,
        }
    }
}

/// Streaming energy novelty for one scored window. Preroll updates the same
/// filter and floor state but contributes no evidence.
pub struct TempoWindow {
    bin_frames: u32,
    frames: u32,
    total: u32,
    filters: [[f64; 2]; 2],
    coefficients: [f64; 2],
    power: [f64; 3],
    previous: [f64; 3],
    floor: [f64; 3],
    novelty: Vec<f64>,
    levels: Option<Vec<[f64; 3]>>,
    spectral: Option<spectral::Input>,
}

impl TempoWindow {
    pub fn new(rate: u32) -> Result<Self, Error> {
        if !SAMPLE_RATES.contains(&rate) {
            return Err(Error::InvalidRate);
        }
        Ok(Self {
            bin_frames: rate / BINS_PER_SECOND,
            frames: 0,
            total: 0,
            filters: [[0.0; 2]; 2],
            coefficients: [150.0, 1800.0]
                .map(|hz| 1.0 - (-std::f64::consts::TAU * hz / f64::from(rate)).exp()),
            power: [0.0; 3],
            previous: [0.0; 3],
            floor: [0.0; 3],
            novelty: Vec::with_capacity(MAX_BINS),
            levels: None,
            spectral: None,
        })
    }

    pub fn with_band_history(rate: u32) -> Result<Self, Error> {
        let mut window = Self::new(rate)?;
        window.levels = Some(Vec::with_capacity(MAX_BINS));
        Ok(window)
    }

    /// Collect reduced PCM in the same scored reads used by the primary detector.
    pub fn with_spectral_history(rate: u32, frames: u64) -> Result<Self, Error> {
        let mut window = Self::with_band_history(rate)?;
        window.spectral = Some(spectral::Input::new(rate, frames));
        Ok(window)
    }

    pub fn push(&mut self, samples: &[[f32; 2]], score: bool) -> Result<(), Error> {
        let frames = u32::try_from(samples.len()).map_err(|_| Error::ResourceLimit)?;
        self.total = self
            .total
            .checked_add(frames)
            .filter(|n| *n <= self.bin_frames * 1205)
            .ok_or(Error::ResourceLimit)?;
        for sample in samples {
            for (state, &input) in self.filters.iter_mut().zip(sample) {
                let input = f64::from(input);
                if !input.is_finite() {
                    return Err(Error::NonfiniteInput);
                }
                for (filtered, coefficient) in state.iter_mut().zip(self.coefficients) {
                    *filtered += (input - *filtered) * coefficient;
                }
                let bands = [state[0], state[1] - state[0], input - state[1]];
                for (power, band) in self.power.iter_mut().zip(bands) {
                    *power += band * band;
                }
            }
            self.frames += 1;
            if self.frames == self.bin_frames {
                let mut novelty = 0.0;
                for i in 0..3 {
                    let power = self.power[i] / f64::from(2 * self.frames);
                    let level = power.sqrt();
                    let base = self.floor[i].max(1e-20);
                    let rise = (level - self.previous[i]).max(0.0);
                    novelty += (rise / base).min(20.0).ln_1p() * level.sqrt();
                    self.previous[i] = level;
                    self.floor[i] += 0.04 * (level - self.floor[i]);
                }
                self.power = [0.0; 3];
                self.frames = 0;
                if score {
                    self.novelty.push(novelty);
                    if let Some(levels) = &mut self.levels
                        && levels.len() < MAX_BINS
                    {
                        levels.push(self.previous);
                    }
                }
            }
        }
        if score && let Some(input) = &mut self.spectral {
            input.push(samples);
        }
        Ok(())
    }

    /// Require rhythm inside the window before accepting a loop-length candidate.
    fn supports_loop(&self, bpm: f64, cancelled: &impl Fn() -> bool) -> Result<bool, Error> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let period = 6000.0 / bpm;
        let mut peaks: Vec<_> = self
            .novelty
            .iter()
            .copied()
            .enumerate()
            .skip(8)
            .take(self.novelty.len().saturating_sub(16))
            .filter(|&(bin, value)| {
                value > 1e-12 && value > self.novelty[bin - 1] && value >= self.novelty[bin + 1]
            })
            .collect();
        peaks.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        let mut salient = Vec::new();
        for (bin, strength) in peaks {
            if salient.len() == 24 {
                break;
            }
            if salient
                .iter()
                .all(|&(other, _): &(usize, f64)| other.abs_diff(bin) >= 7)
            {
                salient.push((bin, strength));
            }
        }
        if salient.len() < 4 {
            return Ok(false);
        }
        let strong = salient[0].1;
        if salient[3].1 < strong * 0.10 {
            return Ok(false);
        }
        let mut pulse = vec![0.0_f64; self.novelty.len()];
        for &(bin, strength) in &salient {
            pulse[bin] = (strength / strong).sqrt();
        }
        let match_strength = |lag: usize| {
            let mut cross = 0.0;
            let mut left = 0.0;
            let mut right = 0.0;
            let mut pairs = 0;
            for (a, &b) in pulse.iter().zip(pulse.iter().skip(lag)) {
                cross += a * b;
                left += a * a;
                right += b * b;
                pairs += usize::from(*a > 0.0 && b > 0.0);
            }
            if pairs >= 2 {
                cross / (left * right).sqrt().max(1e-12)
            } else {
                0.0
            }
        };
        let grid_strength =
            |period: f64, subdivision: f64, min_hits: usize, min_span: i32, tolerance: f64| {
                let mut best = 0.0_f64;
                for &(phase, _) in salient.iter().take(8) {
                    let mut hits = 0;
                    let mut first = i32::MAX;
                    let mut last = i32::MIN;
                    let mut sum = 0.0;
                    for &(bin, strength) in &salient {
                        let delta = (bin as f64 - phase as f64) * subdivision / period;
                        let slot = delta.round();
                        if (delta - slot).abs() * period / subdivision <= tolerance {
                            hits += 1;
                            first = first.min(slot as i32);
                            last = last.max(slot as i32);
                            sum += strength;
                        }
                    }
                    if hits >= min_hits && last.saturating_sub(first) >= min_span {
                        best = best.max(sum);
                    }
                }
                best / salient.iter().map(|(_, strength)| strength).sum::<f64>()
            };
        // A syncopated loop may put accents on eighth or sixteenth notes.
        // Denser grids need more hits, a longer span and greater separation
        // from nearby tempos to avoid accepting chance overlaps.
        let well_aligned = |subdivision, min_hits, min_span, tolerance, minimum, margin| {
            let score = |period| grid_strength(period, subdivision, min_hits, min_span, tolerance);
            let fit = score(period);
            fit >= minimum && fit >= score(period * 0.926).max(score(period * 1.075)) + margin
        };
        if !well_aligned(2.0, 4, 4, 3.0, 0.58, 0.055) && !well_aligned(4.0, 6, 12, 1.8, 0.62, 0.10)
        {
            return Ok(false);
        }
        // In addition to a shared beat phase, require pairs of attacks
        // separated by a whole number of beats away from the window edges.
        for multiple in [1.0, 2.0, 4.0] {
            let lag = (period * multiple).round() as usize;
            if lag + 16 >= pulse.len() * 3 / 4 {
                continue;
            }
            if (lag - 3..=lag + 3).any(|offset| match_strength(offset) >= 0.14) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn weak_with_loop(
        &self,
        candidate: Option<f64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<WindowScore, Error> {
        let supported = match candidate {
            Some(bpm) if (80.0..160.0).contains(&bpm) => self.supports_loop(bpm, cancelled)?,
            _ => false,
        };
        Ok(WindowScore {
            loop_support: supported,
            ..WindowScore::new(Tempo::Weak)
        })
    }

    pub fn estimate(
        &self,
        loop_candidate: Option<f64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<(Tempo, bool), Error> {
        let result = self.score(loop_candidate, cancelled)?;
        Ok((result.tempo, result.loop_support))
    }

    pub fn into_evidence(
        mut self,
        loop_candidate: Option<f64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<TempoEvidence, Error> {
        let score = self.score(loop_candidate, cancelled)?;
        let levels = if score.tempo == Tempo::Weak && !score.range_veto {
            self.levels.take().unwrap_or_default()
        } else {
            Vec::new()
        };
        let spectral = if score.tempo == Tempo::Weak && !score.range_veto {
            self.spectral.take()
        } else {
            None
        };
        Ok(TempoEvidence {
            score,
            levels,
            spectral,
        })
    }

    fn score(
        &self,
        loop_candidate: Option<f64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<WindowScore, Error> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let maximum = self.novelty.iter().copied().fold(0.0, f64::max);
        if maximum <= 1e-12 {
            return Ok(WindowScore::new(Tempo::Weak));
        }
        let mut onsets: Vec<(usize, f64)> = Vec::new();
        let mut peaks: Vec<_> = self
            .novelty
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, value)| *value >= maximum * 0.09)
            .collect();
        peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (position, value) in peaks {
            if onsets
                .iter()
                .all(|(other, _)| other.abs_diff(position) >= 5)
            {
                onsets.push((position, value / maximum));
            }
        }
        if onsets.len() < 4 {
            return self.weak_with_loop(loop_candidate, cancelled);
        }
        let anchors: Vec<_> = onsets
            .iter()
            .take(8)
            .map(|&(position, _)| position)
            .collect();
        onsets.sort_unstable_by_key(|&(position, _)| position);
        // Accents can mark a slower pulse underneath regular subdivisions.
        // Test both the complete onset set and a weighted accent set; a
        // competing interpretation must pass the same grid checks.
        let accents: Vec<_> = onsets
            .iter()
            .filter(|(_, weight)| *weight >= 0.28)
            .map(|&(position, weight)| (position, weight * weight))
            .collect();
        let mut pulse = vec![0.0_f64; self.novelty.len()];
        for &(position, weight) in &onsets {
            for (index, value) in pulse
                .iter_mut()
                .enumerate()
                .skip(position.saturating_sub(2))
                .take((position + 3).min(self.novelty.len()) - position.saturating_sub(2))
            {
                *value =
                    (*value).max(weight.sqrt() * (1.0 - index.abs_diff(position) as f64 / 3.0));
            }
        }
        let correlation = |lag: usize| {
            let (cross, left, right) = pulse
                .iter()
                .zip(pulse.iter().skip(lag))
                .fold((0.0, 0.0, 0.0), |(cross, left, right), (&a, &b)| {
                    (cross + a * b, left + a * a, right + b * b)
                });
            cross / (left * right).sqrt().max(1e-12)
        };
        let mut candidates = Vec::new();
        for lag in 23..=136 {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            let middle = correlation(lag);
            let left = correlation(lag - 1);
            let right = correlation(lag + 1);
            if middle < 0.35 || middle < left || middle < right {
                continue;
            }
            let curvature = left - 2.0 * middle + right;
            let shift = if curvature < -1e-10 {
                0.5 * (left - right) / curvature
            } else {
                0.0
            };
            let center = 6000.0 / (lag as f64 + shift);
            for offset in -8..=8 {
                let bpm = center + f64::from(offset) * 0.25;
                let period = 6000.0 / bpm;
                let all = grid(&onsets, &anchors, period);
                let accents = grid(&accents, &anchors, period).map(|score| score - 0.04);
                if let Some(score) = all.into_iter().chain(accents).reduce(f64::max) {
                    candidates.push((score, bpm));
                }
            }
        }
        let Some(&(best, bpm)) = candidates.iter().max_by(|a, b| a.0.total_cmp(&b.0)) else {
            return self.weak_with_loop(loop_candidate, cancelled);
        };
        // Guard lags allow an unsupported pulse to beat an in-range
        // alias instead of presenting the alias as a measurement.
        if !(MIN_BPM - 0.2..=MAX_BPM + 0.2).contains(&bpm) {
            return Ok(WindowScore {
                range_veto: true,
                ..WindowScore::new(Tempo::Weak)
            });
        }
        let rivals = candidates.into_iter().filter(|&(score, rival)| {
            let margin = if is_half_double(bpm, rival) {
                0.10
            } else {
                0.08
            };
            tempo_ratio(rival, bpm) > 1.03 && best - score <= margin
        });
        Ok(WindowScore::new(choose_octave(bpm, rivals)))
    }
}

fn tempo_ratio(a: f64, b: f64) -> f64 {
    a.max(b) / a.min(b)
}

fn is_half_double(a: f64, b: f64) -> bool {
    (tempo_ratio(a, b) - 2.0).abs() <= 0.06
}

fn closest_to_octave_reference(a: f64, b: f64) -> f64 {
    match (a - Tempo::PREFERRED_OCTAVE_BPM)
        .abs()
        .total_cmp(&(b - Tempo::PREFERRED_OCTAVE_BPM).abs())
    {
        std::cmp::Ordering::Less => a,
        std::cmp::Ordering::Equal => a.min(b),
        std::cmp::Ordering::Greater => b,
    }
}

// Keep the strongest representative of each supported octave. Small
// offsets around a local peak are not separate pulse interpretations.
fn choose_octave(bpm: f64, rivals: impl IntoIterator<Item = (f64, f64)>) -> Tempo {
    let mut octave_peaks: [Option<(f64, f64)>; 2] = [None, None];
    for (score, rival) in rivals {
        if !is_half_double(bpm, rival) || !(MIN_BPM - 0.2..=MAX_BPM + 0.2).contains(&rival) {
            return Tempo::Ambiguous;
        }
        let peak = &mut octave_peaks[usize::from(rival > bpm)];
        if peak.is_none_or(|(best, _)| score > best) {
            *peak = Some((score, rival));
        }
    }
    let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
    if octave_peaks.iter().all(Option::is_none) {
        return Tempo::Estimate {
            bpm,
            sampled: false,
        };
    }
    let bpm = octave_peaks
        .into_iter()
        .flatten()
        .map(|(_, rate)| rate.clamp(MIN_BPM, MAX_BPM))
        .fold(bpm, closest_to_octave_reference);
    Tempo::OctaveAmbiguous {
        bpm,
        sampled: false,
    }
}

fn grid(onsets: &[(usize, f64)], anchors: &[usize], period: f64) -> Option<f64> {
    if onsets.len() < 4 {
        return None;
    }
    let total: f64 = onsets.iter().map(|&(_, weight)| weight).sum();
    let tolerance = (period * 0.04).max(2.0);
    let mut best = None;
    for &anchor in anchors {
        let mut last = None;
        let mut first = 0;
        let mut occupied = 0;
        let mut adjacent = 0;
        let mut explained = 0.0;
        let mut error = 0.0;
        let mut slot_weight = 0.0;
        let mut slot_error = 0.0;
        for &(position, weight) in onsets {
            let delta = position as f64 - anchor as f64;
            let slot = (delta / period).round() as i32;
            let distance = (delta - f64::from(slot) * period).abs();
            if distance > tolerance {
                continue;
            }
            let weighted_error = weight * distance / tolerance;
            if last == Some(slot) {
                if weight > slot_weight {
                    explained += weight - slot_weight;
                    error += weighted_error - slot_error;
                    slot_weight = weight;
                    slot_error = weighted_error;
                }
                continue;
            }
            if let Some(previous) = last {
                adjacent += usize::from(slot == previous + 1);
            } else {
                first = slot;
            }
            last = Some(slot);
            occupied += 1;
            slot_weight = weight;
            slot_error = weighted_error;
            explained += weight;
            error += weighted_error;
        }
        let coverage = f64::from(occupied) / f64::from(last.unwrap_or(first) - first + 1);
        let explanation = explained / total;
        let fit = 2.0 * coverage * explanation / (coverage + explanation).max(1e-12);
        if adjacent >= 3 && coverage >= 0.65 && fit >= 0.65 {
            let score = fit - 0.12 * error / explained;
            best = Some(best.map_or(score, |b: f64| b.max(score)));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_window(rate: u32, bpm: f64, beats: usize, phase: f64, gain: f32) -> Tempo {
        let period = 60.0 / bpm * f64::from(rate);
        let start = (phase * period).round() as usize;
        let frames =
            ((start as f64 + (beats as f64 - 0.75) * period) as usize).min(rate as usize * 12);
        let mut samples = vec![[0.0; 2]; frames];
        for i in 0..beats {
            let onset = start + (i as f64 * period).round() as usize;
            for (j, frame) in samples.iter_mut().skip(onset).take(100).enumerate() {
                let level = gain * (1.0 - j as f32 / 100.0) * if j % 2 == 0 { 1.0 } else { -1.0 };
                *frame = [level, -level];
            }
        }
        let mut window = TempoWindow::new(rate).unwrap();
        for block in samples.chunks(8192) {
            window.push(block, true).unwrap();
        }
        window.estimate(None, &|| false).unwrap().0
    }

    #[test]
    fn octave_only_rivals_use_the_supported_rate_closest_to_100() {
        let octave = |bpm| Tempo::OctaveAmbiguous {
            bpm,
            sampled: false,
        };
        assert_eq!(
            choose_octave(55.0, [(0.8, 110.0), (0.7, 108.0)]),
            octave(110.0)
        );
        assert_eq!(choose_octave(140.0, [(0.8, 70.0)]), octave(70.0));
        assert_eq!(choose_octave(134.0, [(0.8, 66.0)]), octave(66.0));
        assert_eq!(
            choose_octave(61.0, [(0.8, 122.0), (0.79, 145.0)]),
            Tempo::Ambiguous
        );
        assert_eq!(choose_octave(123.0, [(0.8, 246.0)]), Tempo::Ambiguous);
        assert_eq!(
            choose_octave(178.0, []),
            Tempo::Estimate {
                bpm: 178.0,
                sampled: false
            }
        );
    }

    #[test]
    fn sampled_octave_votes_require_agreement_after_alignment() {
        let windows = TempoWindows::new(48_000, 40 * 48_000).unwrap();
        let estimate = |bpm| Tempo::Estimate {
            bpm,
            sampled: false,
        };
        let octave_vote = choose_octave(70.0, [(0.8, 140.0)]);
        assert_eq!(
            windows.combine(&[estimate(70.0), octave_vote, Tempo::Weak]),
            Tempo::OctaveAmbiguous {
                bpm: 70.0,
                sampled: true
            }
        );
        assert_eq!(
            windows.combine(&[estimate(70.0), estimate(140.0), Tempo::Weak]),
            Tempo::OctaveAmbiguous {
                bpm: 70.0,
                sampled: true
            }
        );
        for votes in [
            [octave_vote, Tempo::Weak, Tempo::Weak],
            [octave_vote, estimate(125.0), Tempo::Weak],
            [estimate(70.0), estimate(140.0), Tempo::Ambiguous],
            [estimate(117.0), estimate(120.0), estimate(123.0)],
        ] {
            assert_eq!(windows.combine(&votes), Tempo::Ambiguous);
        }
    }

    #[test]
    fn estimates_clean_pulses_in_supported_range_and_four_beat_loops() {
        for rate in SAMPLE_RATES {
            for bpm in [45.0, 47.0, 70.0, 90.0, 120.0, 128.0, 174.0, 235.0, 240.0] {
                let result = click_window(rate, bpm, 8, 0.18, 0.002);
                assert!(
                    matches!(result, Tempo::Estimate { bpm: actual, .. } if (actual - bpm).abs() <= 1.0),
                    "{rate} {bpm}: {result:?}"
                );
            }
            for phase in [0.0, 0.2, 0.6] {
                let result = click_window(rate, 128.0, 4, phase, 0.8);
                assert!(
                    matches!(result, Tempo::Estimate { bpm, .. } if (bpm - 128.0).abs() <= 1.5),
                    "{rate} {phase}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn syncopated_tonal_loop_uses_audio_to_confirm_the_candidate() {
        let rate = 48_000;
        let frames = rate * 8;
        let mut samples = vec![[0.0; 2]; frames as usize];
        // Repeated phrase with gaps between beat positions, soft attacks,
        // and changing pitch. There are no regularly spaced loud transients.
        for (i, offset) in [
            0.5, 1.5, 3.0, 4.5, 6.5, 7.0, 8.5, 9.5, 11.0, 12.5, 14.5, 15.0,
        ]
        .into_iter()
        .enumerate()
        {
            let start = (offset * f64::from(rate) * 0.5) as usize;
            for (j, sample) in samples.iter_mut().skip(start).take(8000).enumerate() {
                let time = j as f32 / rate as f32;
                let gain = (time / 0.026).min(1.0) * (1.0 - j as f32 / 8000.0);
                let phase = std::f32::consts::TAU * time * (220.0 + (i % 6) as f32 * 40.0);
                let sound = gain * (0.25 * phase.sin() + 0.08 * (phase * 2.0).sin());
                *sample = [sound, -sound];
            }
        }
        let windows = TempoWindows::new(rate, u64::from(frames)).unwrap();
        assert_eq!(windows.loop_candidate(), Some(120.0));
        let mut detector = TempoWindow::new(rate).unwrap();
        detector.push(&samples, true).unwrap();
        let (estimate, supported) = detector
            .estimate(windows.loop_candidate(), &|| false)
            .unwrap();
        assert_eq!(estimate, Tempo::Weak);
        assert!(supported);
        assert_eq!(
            windows.combine_with_loop(&[estimate], &[supported]),
            Tempo::Estimate {
                bpm: 120.0,
                sampled: false
            }
        );
        assert!(!detector.supports_loop(107.0, &|| false).unwrap());
    }

    #[test]
    fn soft_tonal_pulse_recovers_without_changing_the_primary_score() {
        for rate in SAMPLE_RATES {
            let beat_frames = 60.0 * f64::from(rate) / 140.0;
            let frames = (8.0 * beat_frames).round() as usize;
            let windows = TempoWindows::new(rate, frames as u64).unwrap();
            let candidate = windows.loop_candidate();
            let mut detector = TempoWindow::with_band_history(rate).unwrap();
            let mut phase = 0.0;
            let samples: Vec<_> = (0..frames)
                .map(|frame| {
                    let position = frame as f64 / beat_frames;
                    let beat = position as usize;
                    let syllable = (position.fract() - 0.49).rem_euclid(1.0);
                    let smooth = (0.5 - 0.5 * (std::f64::consts::TAU * syllable).cos()).powi(2);
                    let strength = [1.0, 0.83, 0.97, 0.90][(beat + 2) % 4];
                    let pitch = [146.83, 220.0, 261.63, 293.66][(beat * 3 + 2) % 4];
                    let bend = (position * 4.0).sin() * 9.0;
                    phase += std::f64::consts::TAU * (pitch + bend) / f64::from(rate);
                    let signal = 0.20
                        * (0.34 + smooth * strength)
                        * (phase.sin() + 0.16 * (2.0 * phase).sin());
                    [signal as f32, -signal as f32]
                })
                .collect();
            for block in samples.chunks(8192) {
                detector.push(block, true).unwrap();
            }
            assert_eq!(
                detector.estimate(candidate, &|| false).unwrap(),
                (Tempo::Weak, false)
            );
            let evidence = detector.into_evidence(candidate, &|| false).unwrap();
            assert!(
                matches!(windows.combine_evidence(&[evidence], &|| false).unwrap(),
                    Tempo::Estimate { bpm, sampled: false } if (bpm - 140.0).abs() < 1.0),
                "{rate}"
            );
        }
    }

    #[test]
    fn spectral_changes_recover_a_soft_tonal_phrase() {
        let rate = 48_000;
        let bpm = 84.0;
        let beat_frames = 60.0 * f64::from(rate) / bpm;
        let frames = (12.0 * beat_frames).round() as u64;
        let mut phase = 0.0;
        let samples: Vec<_> = (0..frames)
            .map(|frame| {
                let position = frame as f64 / beat_frames;
                let beat = position as usize;
                let offset = (position.fract() - 0.69).rem_euclid(1.0);
                let gain = (0.5 - 0.5 * (std::f64::consts::TAU * offset).cos()).powi(2);
                let strength = [1.0, 0.83, 0.97, 0.90][(beat + 3) % 4];
                let pitch = [130.81, 196.0, 233.08, 349.22][(beat + 3) % 4];
                phase += std::f64::consts::TAU * (pitch + (position * 4.0).sin() * 9.0)
                    / f64::from(rate);
                let signal =
                    0.20 * (0.34 + gain * strength) * (phase.sin() + 0.16 * (2.0 * phase).sin());
                [signal as f32, -signal as f32]
            })
            .collect();

        let schedule = TempoWindows::new(rate, frames).unwrap();
        let mut baseline = TempoWindow::with_band_history(rate).unwrap();
        let mut spectral = TempoWindow::with_spectral_history(rate, frames).unwrap();
        for block in samples.chunks(8192) {
            baseline.push(block, true).unwrap();
        }
        for block in samples.chunks(1171) {
            spectral.push(block, true).unwrap();
        }
        let score = |window: TempoWindow| {
            let evidence = window
                .into_evidence(schedule.loop_candidate(), &|| false)
                .unwrap();
            schedule.combine_evidence(&[evidence], &|| false).unwrap()
        };
        assert_eq!(score(baseline), Tempo::Weak);
        assert!(
            matches!(score(spectral), Tempo::Estimate { bpm: found, sampled: false }
                if (found - bpm).abs() < 1.0)
        );
    }

    #[test]
    fn duration_cannot_replace_missing_or_conflicting_evidence() {
        let rate = 48_000;
        let windows = TempoWindows::new(rate, 16 * u64::from(rate)).unwrap();
        assert_eq!(windows.loop_candidate(), Some(120.0));
        let tempo = Tempo::Estimate {
            bpm: 140.0,
            sampled: false,
        };
        assert_eq!(
            windows.combine_with_loop(&[Tempo::Weak, Tempo::Weak], &[true, false]),
            Tempo::Weak
        );
        assert_eq!(
            windows.combine_with_loop(&[Tempo::Ambiguous, Tempo::Weak], &[false, true]),
            Tempo::Ambiguous
        );
        assert_eq!(
            windows.combine_with_loop(&[tempo, Tempo::Weak], &[false, true]),
            Tempo::Weak
        );
        assert_eq!(
            windows.combine_with_loop(&[tempo, tempo], &[true, true]),
            tempo
        );
        let mut silence = TempoWindow::new(rate).unwrap();
        silence.push(&[[0.0; 2]; 4800], true).unwrap();
        assert_eq!(
            silence
                .estimate(windows.loop_candidate(), &|| false)
                .unwrap(),
            (Tempo::Weak, false)
        );
        assert!(
            TempoWindows::new(rate, 37 * u64::from(rate))
                .unwrap()
                .loop_candidate()
                .is_none()
        );

        let period = 60.0 * f64::from(rate) / 248.0;
        let mut audio = vec![[0.0; 2]; (16.0 * period).round() as usize];
        for beat in 0..16 {
            let onset = ((f64::from(beat) + 0.1) * period).round() as usize;
            audio[onset..onset + 64].fill([0.6, -0.6]);
        }
        let source = TempoWindows::new(rate, audio.len() as u64).unwrap();
        assert!((source.loop_candidate().unwrap() - 124.0).abs() < 0.01);
        let mut detector = TempoWindow::new(rate).unwrap();
        detector.push(&audio, true).unwrap();
        assert_eq!(
            detector
                .estimate(source.loop_candidate(), &|| false)
                .unwrap(),
            (Tempo::Weak, false)
        );
    }

    #[test]
    fn refuses_inadequate_or_conflicting_evidence() {
        for (bpm, beats) in [(120.0, 1), (120.0, 3), (44.0, 8), (248.0, 12)] {
            let result = click_window(48_000, bpm, beats, 0.1, 0.5);
            assert!(
                !matches!(result, Tempo::Estimate { .. }),
                "{bpm}: {result:?}"
            );
        }
        let mut silence = TempoWindow::new(48_000).unwrap();
        silence.push(&[[0.0; 2]; 4800], true).unwrap();
        assert_eq!(silence.estimate(None, &|| false).unwrap().0, Tempo::Weak);
        let (mut noise, mut tone) = (
            TempoWindow::new(48_000).unwrap(),
            TempoWindow::new(48_000).unwrap(),
        );
        let mut random = 0x678a_42c1_u32;
        let mut frame = 0;
        for _ in 0..50 {
            let mut noise_block = [[0.0; 2]; 4800];
            let mut tone_block = [[0.0; 2]; 4800];
            for (a, b) in noise_block.iter_mut().zip(&mut tone_block) {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                *a = [random as i32 as f32 / i32::MAX as f32; 2];
                *b = [(std::f32::consts::TAU * frame as f32 * 440.0 / 48_000.0).sin(); 2];
                frame += 1;
            }
            noise.push(&noise_block, true).unwrap();
            tone.push(&tone_block, true).unwrap();
        }
        for input in [noise, tone] {
            let (tempo, supports_loop) = input.estimate(Some(120.0), &|| false).unwrap();
            assert!(!matches!(tempo, Tempo::Estimate { .. }));
            assert!(!supports_loop);
        }

        let long = TempoWindows::new(48_000, u64::MAX).unwrap();
        let pulse = Tempo::Estimate {
            bpm: 120.0,
            sampled: false,
        };
        assert_eq!(
            long.ranges
                .iter()
                .map(|range| range.end - range.start)
                .sum::<u64>(),
            36 * 48_000
        );
        assert_eq!(
            long.combine(&[pulse, pulse, Tempo::Ambiguous]),
            Tempo::Ambiguous
        );
        assert_eq!(
            long.combine(&[
                pulse,
                pulse,
                Tempo::Estimate {
                    bpm: 140.0,
                    sampled: false
                }
            ]),
            Tempo::Ambiguous
        );
        assert_eq!(
            long.combine(&[pulse, Tempo::Weak, Tempo::Weak]),
            Tempo::Weak
        );
        assert_eq!(
            long.combine(&[pulse, Tempo::Weak, pulse]),
            Tempo::Estimate {
                bpm: 120.0,
                sampled: true
            }
        );
    }
}
