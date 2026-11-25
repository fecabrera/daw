//! A second pulse vote from signed changes in broad-band levels.
use super::{MAX_BINS, MAX_BPM, MIN_BPM, Tempo, choose_octave};
use crate::Error;

const MIN_LAG: usize = 23;
const MAX_LAG: usize = 136;
// A two-beat period at the lowest loop-candidate rate is 150 bins.
// Include the near-peak search on either side.
const CACHED_LAG: usize = 154;
const MAX_PEAKS: usize = 8;
const MAX_LANDMARKS: usize = 8;
const DIRECT_MIN: f64 = 0.32;
const CORE_MIN: f64 = 0.44;
const PHASE_MIN: f64 = 0.56;
const RIVAL_MARGIN: f64 = 0.07;
const LOOP_PHASE_MIN: f64 = 0.10;

#[derive(Debug, PartialEq)]
pub(super) enum Vote {
    Estimate(f64),
    OctaveAmbiguous(f64),
    Weak,
    Veto,
}

impl Vote {
    pub(super) fn into_tempo(self) -> Tempo {
        match self {
            Self::Estimate(bpm) => Tempo::Estimate {
                bpm,
                sampled: false,
            },
            Self::OctaveAmbiguous(bpm) => Tempo::OctaveAmbiguous {
                bpm,
                sampled: false,
            },
            Self::Weak => Tempo::Weak,
            Self::Veto => Tempo::Ambiguous,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Landmark {
    bin: usize,
    weight: f64,
}

#[derive(Clone, Copy, Default)]
struct Peak {
    period: f64,
    repeat: f64,
}

pub(super) fn estimate(
    levels: &[[f64; 3]],
    loop_candidate: Option<f64>,
    cancelled: &impl Fn() -> bool,
) -> Result<Vote, Error> {
    if levels.len() < 100 {
        return Ok(Vote::Weak);
    }
    let mut changes = vec![[0.0_f64; 3]; levels.len() - 6];
    if !signed_changes(levels, &mut changes, cancelled)? {
        return Ok(Vote::Weak);
    }
    let halfway = changes.len() / 2;
    let left = landmarks(&changes[..halfway], 0);
    let right = landmarks(&changes[halfway..], halfway);
    if left[2].weight == 0.0 || right[2].weight == 0.0 {
        return Ok(Vote::Weak);
    }

    let scores = Correlations::new(&changes, cancelled)?;
    if let Some(bpm) = loop_candidate
        && confirms_loop_rate(&scores, bpm, cancelled)?
    {
        return Ok(Vote::Estimate(bpm));
    }
    let mut peaks = [Peak::default(); MAX_PEAKS];
    for lag in MIN_LAG..=MAX_LAG {
        let [before, repeat, after] = [lag - 1, lag, lag + 1].map(|lag| scores.at(lag));
        if repeat < DIRECT_MIN || repeat <= before || repeat < after {
            continue;
        }
        let curve = before - 2.0 * repeat + after;
        let shift = if curve < -1e-10 {
            0.5 * (before - after) / curve
        } else {
            0.0
        };
        let period = lag as f64 + shift;
        let repeat = repeat - 0.25 * (before - after) * shift;
        let repeat = (repeat + 0.18 * scores.interpolate(2.0 * period).max(0.0)) / 1.18;
        if let Some(index) = peaks.iter().position(|peak| repeat > peak.repeat) {
            peaks[index..].rotate_right(1);
            peaks[index] = Peak { period, repeat };
        }
    }

    let mut candidates = [(0.0, 0.0); MAX_PEAKS];
    for (entry, peak) in candidates.iter_mut().zip(peaks) {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        if peak.repeat < CORE_MIN {
            break;
        }
        let phase = phase_agreement(&left, &right, peak.period);
        let off_grid = phase_agreement(&left, &right, peak.period * 0.93).max(phase_agreement(
            &left,
            &right,
            peak.period * 1.075,
        ));
        if phase >= PHASE_MIN && phase >= off_grid + 0.04 {
            *entry = (peak.period, 0.72 * peak.repeat + 0.28 * phase);
        }
    }
    let &(period, score) = candidates
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("fixed nonempty array");
    if score == 0.0 {
        return Ok(Vote::Weak);
    }
    let bpm = 6000.0 / period;
    if !(MIN_BPM - 0.2..=MAX_BPM + 0.2).contains(&bpm) {
        return Ok(Vote::Veto);
    }
    let rivals = candidates
        .into_iter()
        .filter(|&(rival, fit)| {
            fit > 0.0 && rival.max(period) > 1.03 * rival.min(period) && score - fit < RIVAL_MARGIN
        })
        .map(|(rival, fit)| (fit, 6000.0 / rival));
    let mut octave_guards = [None; 2];

    // A half-period peak can be crowded out of the list by longer repeats.
    // Check the octave rivals directly before accepting the raw beat rate.
    for (index, rival) in [0.5 * period, 2.0 * period].into_iter().enumerate() {
        if !(MIN_LAG as f64..=MAX_LAG as f64).contains(&rival) {
            continue;
        }
        let repeat = scores.interpolate(rival);
        if repeat >= DIRECT_MIN {
            let phase = phase_agreement(&left, &right, rival);
            let fit = 0.72 * repeat + 0.28 * phase;
            if phase >= PHASE_MIN && score - fit < RIVAL_MARGIN {
                octave_guards[index] = Some((fit, 6000.0 / rival));
            }
        }
    }

    Ok(
        match choose_octave(bpm, rivals.chain(octave_guards.into_iter().flatten())) {
            Tempo::Estimate { bpm, .. } => Vote::Estimate(bpm),
            Tempo::OctaveAmbiguous { bpm, .. } => Vote::OctaveAmbiguous(bpm),
            _ => Vote::Veto,
        },
    )
}

// The loop clock can distinguish octave-equivalent repeat peaks. Verify the
// audio near that clock before using it, including when the repeat spans two beats.
fn confirms_loop_rate(
    scores: &Correlations<'_>,
    bpm: f64,
    cancelled: &impl Fn() -> bool,
) -> Result<bool, Error> {
    let beat = 6000.0 / bpm;
    let mut support = 0.0_f64;
    for period in [beat, 2.0 * beat] {
        let lo = (period * 0.979).ceil() as usize;
        let hi = (period * 1.021).floor() as usize;
        let Some(lag) =
            (lo..=hi.min(2 * MAX_LAG)).max_by(|&a, &b| scores.at(a).total_cmp(&scores.at(b)))
        else {
            continue;
        };
        let repeat = scores.at(lag);
        if repeat < 0.38 || lag * 3 > scores.changes.len() {
            continue;
        }
        let nearby = scores
            .interpolate(period * 0.938)
            .max(scores.interpolate(period * 1.066));
        if repeat - nearby < 0.08 || repeat_in_both_halves(scores.changes, lag) < 0.23 {
            continue;
        }
        let phase = repeat_phase(scores.changes, lag, beat, cancelled)?;
        if phase >= LOOP_PHASE_MIN {
            support = support.max(repeat);
        }
    }
    if support == 0.0 {
        return Ok(false);
    }
    // A better period from a different family has priority over the length hint.
    Ok((MIN_LAG..=MAX_LAG).all(|lag| {
        let ratio = lag as f64 / beat;
        let same_family = [0.5, 1.0, 2.0]
            .into_iter()
            .any(|factor| (ratio / factor - 1.0).abs() < 0.047);
        same_family || scores.at(lag) <= support + 0.065
    }))
}

fn repeat_in_both_halves(changes: &[[f64; 3]], lag: usize) -> f64 {
    let midpoint = (changes.len() - lag) / 2;
    let before = &changes[..changes.len() - lag];
    let after = &changes[lag..];
    normalized_repeat(&before[..midpoint], &after[..midpoint])
        .min(normalized_repeat(&before[midpoint..], &after[midpoint..]))
}

fn repeat_phase(
    changes: &[[f64; 3]],
    lag: usize,
    beat: f64,
    cancelled: &impl Fn() -> bool,
) -> Result<f64, Error> {
    let count = changes.len() - lag;
    let mut best = 0.0_f64;
    // Measure the phase of recurrent changes in each half of the matched
    // overlap. Shifts in timbre or in which subdivision carries the accent
    // must still produce a stable beat or eighth-note phase.
    for subdivision in [1.0, 2.0] {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let (sin_step, cos_step) = (std::f64::consts::TAU * subdivision / beat).sin_cos();
        let (mut cos, mut sin) = (1.0, 0.0);
        let mut halves = [[0.0_f64; 3]; 2];
        for (i, (a, b)) in changes.iter().zip(&changes[lag..]).enumerate() {
            let weight = dot(a, b).max(0.0);
            let part = &mut halves[usize::from(i >= count / 2)];
            part[0] += weight * cos;
            part[1] += weight * sin;
            part[2] += weight;
            (cos, sin) = (
                cos * cos_step - sin * sin_step,
                sin * cos_step + cos * sin_step,
            );
        }
        let [left, right] = halves;
        let left_magnitude = left[0].hypot(left[1]);
        let right_magnitude = right[0].hypot(right[1]);
        let coherence = (left[0] * right[0] + left[1] * right[1])
            / (left_magnitude * right_magnitude).max(1e-12);
        let concentration =
            (left_magnitude / left[2].max(1e-12)).min(right_magnitude / right[2].max(1e-12));
        best = best.max(coherence * concentration);
    }
    Ok(best)
}

fn signed_changes(
    levels: &[[f64; 3]],
    changes: &mut [[f64; 3]],
    cancelled: &impl Fn() -> bool,
) -> Result<bool, Error> {
    let mut active = false;
    for band in 0..3 {
        let peak = levels.iter().map(|level| level[band]).fold(0.0, f64::max);
        if peak < 1e-7 {
            continue;
        }
        let floor = (peak * 0.035).max(1e-8);
        let mut energy = 0.0;
        for (i, (change, frame)) in changes.iter_mut().zip(levels.windows(7)).enumerate() {
            if i % 128 == 0 && cancelled() {
                return Err(Error::Cancelled);
            }
            let before = frame[..3].iter().map(|level| level[band]).sum::<f64>() / 3.0;
            let after = frame[4..].iter().map(|level| level[band]).sum::<f64>() / 3.0;
            let value = ((after - before) / (floor + before.min(after))).asinh();
            change[band] = value;
            energy += value * value;
        }
        let rms = (energy / changes.len() as f64).sqrt();
        if rms < 0.035 {
            for change in &mut *changes {
                change[band] = 0.0;
            }
        } else {
            active = true;
            let scale = rms.max(0.10);
            for change in &mut *changes {
                change[band] = (change[band] / scale).clamp(-3.0, 3.0);
            }
        }
    }
    Ok(active)
}

fn power(change: &[f64; 3]) -> f64 {
    change[0] * change[0] + change[1] * change[1] + change[2] * change[2]
}

fn landmarks(changes: &[[f64; 3]], offset: usize) -> [Landmark; MAX_LANDMARKS] {
    let mut selected = [Landmark::default(); MAX_LANDMARKS];
    for i in 0..MAX_LANDMARKS {
        let mut best = Landmark::default();
        for (bin, frame) in changes.iter().enumerate().skip(3).take(changes.len() - 6) {
            let weight = power(frame);
            let bin = bin + offset;
            if weight > best.weight
                && selected[..i]
                    .iter()
                    .all(|previous| previous.bin.abs_diff(bin) >= 8)
            {
                best = Landmark { bin, weight };
            }
        }
        if best.weight < 0.8 {
            break;
        }
        selected[i] = best;
    }
    selected
}

struct Correlations<'a> {
    changes: &'a [[f64; 3]],
    cumulative_power: Vec<f64>,
    cached: [f64; CACHED_LAG + 1],
}

impl<'a> Correlations<'a> {
    fn new(changes: &'a [[f64; 3]], cancelled: &impl Fn() -> bool) -> Result<Self, Error> {
        let mut cumulative_power = Vec::with_capacity(changes.len() + 1);
        cumulative_power.push(0.0);
        let mut total = 0.0;
        for frame in changes {
            total += power(frame);
            cumulative_power.push(total);
        }
        let mut correlations = Self {
            changes,
            cumulative_power,
            cached: [0.0; CACHED_LAG + 1],
        };
        for lag in MIN_LAG - 1..=CACHED_LAG {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            correlations.cached[lag] = correlations.calculate(lag);
        }
        Ok(correlations)
    }

    fn calculate(&self, lag: usize) -> f64 {
        let length = self.changes.len();
        if lag * 5 > length * 3 {
            return 0.0;
        }
        let left = self.cumulative_power[length - lag];
        let right = self.cumulative_power[length] - self.cumulative_power[lag];
        if left.min(right) < 0.15 * (length - lag) as f64 {
            return 0.0;
        }
        let cross: f64 = self
            .changes
            .iter()
            .zip(&self.changes[lag..])
            .map(|(a, b)| dot(a, b))
            .sum();
        cross / (left * right).sqrt()
    }

    fn at(&self, lag: usize) -> f64 {
        self.cached
            .get(lag)
            .copied()
            .unwrap_or_else(|| self.calculate(lag))
    }

    fn interpolate(&self, lag: f64) -> f64 {
        let i = lag.floor() as usize;
        let start = self.at(i);
        start + (lag - i as f64) * (self.at(i + 1) - start)
    }
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized_repeat(before: &[[f64; 3]], after: &[[f64; 3]]) -> f64 {
    let mut cross = 0.0;
    let mut left = 0.0;
    let mut right = 0.0;
    for (a, b) in before.iter().zip(after) {
        cross += dot(a, b);
        left += power(a);
        right += power(b);
    }
    if left.min(right) >= 0.15 * before.len() as f64 {
        cross / (left * right).sqrt()
    } else {
        0.0
    }
}

fn phase_agreement(left: &[Landmark], right: &[Landmark], period: f64) -> f64 {
    [1.0, 2.0]
        .into_iter()
        .map(|subdivision| {
            let step = period / subdivision;
            let tolerance = (step * 0.09).clamp(1.8, 3.3);
            let transfer = |train: &[Landmark], validate: &[Landmark]| {
                let mut best = (0.0_f64, 0.0_f64);
                for anchor in train.iter().take_while(|anchor| anchor.weight > 0.0) {
                    let phase = anchor.bin as f64;
                    let fit = grid_fit(train, phase, step, tolerance, subdivision as i32);
                    if fit > best.0 {
                        best = (
                            fit,
                            grid_fit(validate, phase, step, tolerance, subdivision as i32),
                        );
                    }
                }
                best.0.min(best.1)
            };
            transfer(left, right).min(transfer(right, left)) - 0.04 * (subdivision - 1.0)
        })
        .fold(0.0, f64::max)
}

fn grid_fit(landmarks: &[Landmark], phase: f64, step: f64, tolerance: f64, span: i32) -> f64 {
    let mut slots = [i32::MIN; MAX_LANDMARKS];
    let mut count = 0;
    let mut total = 0.0;
    let mut explained = 0.0;
    for landmark in landmarks
        .iter()
        .take_while(|landmark| landmark.weight > 0.0)
    {
        let delta = (landmark.bin as f64 - phase) / step;
        let nearest = delta.round();
        let error = (delta - nearest).abs() * step;
        let weight = landmark.weight.sqrt();
        total += weight;
        if error <= tolerance && !slots[..count].contains(&(nearest as i32)) {
            slots[count] = nearest as i32;
            count += 1;
            explained += weight * (1.0 - 0.25 * error / tolerance);
        }
    }
    let covered_span = slots[..count].iter().min().zip(slots[..count].iter().max());
    if count >= 3 && covered_span.is_some_and(|(first, last)| last - first >= 2 * span) {
        explained / total
    } else {
        0.0
    }
}

const _: () = assert!(
    (MAX_BINS * std::mem::size_of::<[f64; 3]>())
        + ((MAX_BINS + CACHED_LAG + 1) * std::mem::size_of::<f64>())
        < 48 * 1024
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_changes_refuse_flat_or_out_of_range_input() {
        let flat = [[0.3; 3]; 600];
        assert_eq!(estimate(&flat, Some(120.0), &|| false), Ok(Vote::Weak));

        let levels: Vec<_> = (0..1000)
            .map(|bin| {
                let phase = std::f64::consts::TAU * f64::from(bin) * 248.0 / 6000.0;
                [0.3 + 0.2 * phase.sin(), 0.3 + 0.2 * phase.cos(), 0.0]
            })
            .collect();
        assert!(!matches!(
            estimate(&levels, None, &|| false),
            Ok(Vote::Estimate(_))
        ));
        assert_eq!(estimate(&levels, None, &|| true), Err(Error::Cancelled));
    }
}
