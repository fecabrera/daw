use super::{ContextTrace, Mode, Stage, Work, check_cancelled, onset::Envelope};
use crate::Error;
use std::ops::Range;

mod verify;

const MIN_PERIOD: f64 = 25.0;
const MAX_PERIOD: f64 = 6000.0 / 45.0;
const MIN_LAG: usize = 23;
const MAX_LAG: usize = 150;
const CONTEXT: usize = 600;
const STRIDE: usize = 50;
const MAX_CANDIDATES: usize = 30;

#[derive(Clone, Copy, Default)]
struct Candidate {
    period: f64,
    score: f64,
    balanced: bool,
}

pub(super) struct Run {
    pub range: Range<usize>,
    pub bpm: f64,
    pub octave: Option<f64>,
    pub beats: Vec<(f64, bool)>,
}

enum Decoded {
    Run(Run),
    Weak,
    Ambiguous,
}

#[derive(Default)]
pub(super) struct Track {
    pub runs: Vec<Run>,
    pub ambiguous: bool,
    pub outside: bool,
    pub variable: bool,
}

pub(super) fn estimate(
    envelope: &Envelope,
    loop_hint: Option<f64>,
    mode: Mode,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Track, Error> {
    let mut result = Track::default();
    let strongest = envelope.rms.iter().copied().fold(0.0, f64::max);
    let floor = (strongest * 0.012).max(2e-5);
    let active = envelope
        .rms
        .iter()
        .map(|&level| level >= floor)
        .collect::<Vec<_>>();
    let last_active = mode
        .targeted_selection()
        .then(|| active.iter().rposition(|&on| on))
        .flatten();
    let mut cursor = 0;
    while let Some(first) = active[cursor..].iter().position(|&on| on) {
        let start = cursor + first;
        let mut last = start;
        for (i, &on) in active.iter().enumerate().skip(start + 1) {
            if i - last >= 150 {
                break;
            }
            if on {
                last = i;
            }
        }
        // A duration-derived challenger applies only to a single active span.
        let span_mode = if mode.targeted_selection() && (cursor > 0 || last_active != Some(last)) {
            Mode::Legacy
        } else {
            mode
        };
        cursor = last + 1;
        // Leave room for the centered envelope and onset refinement.
        let span = start.saturating_sub(12)..(cursor + 8).min(active.len());
        track_span(
            envelope,
            span,
            loop_hint,
            span_mode,
            &mut result,
            work,
            cancelled,
        )?;
    }
    Ok(result)
}

fn track_span(
    envelope: &Envelope,
    span: Range<usize>,
    loop_hint: Option<f64>,
    mode: Mode,
    result: &mut Track,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<(), Error> {
    if span.len() < 48 {
        return Ok(());
    }
    let mut times: Vec<_> = if span.len() <= CONTEXT {
        vec![(span.start + span.end) / 2]
    } else {
        (span.start + CONTEXT / 2..=span.end - CONTEXT / 2)
            .step_by(STRIDE)
            .chain(std::iter::once(span.end - CONTEXT / 2))
            .collect()
    };
    times.dedup();
    let mut contexts = Vec::with_capacity(times.len());
    let mut any_guard = false;
    let mut disputes = Vec::new();
    let trace_start = work
        .profile
        .as_ref()
        .map_or(0, |profile| profile.contexts.len());
    for &time in &times {
        check_cancelled(cancelled)?;
        let start = time.saturating_sub(CONTEXT / 2).max(span.start);
        let end = (start + CONTEXT).min(span.end);
        let clock = work.start();
        if let Some(profile) = &mut work.profile {
            profile.contexts.push(ContextTrace {
                landmarks: 0,
                proposed_periods: Vec::new(),
                qualified_periods: Vec::new(),
                kept_periods: Vec::new(),
                guard: false,
                selected_period: None,
            });
        }
        let mut candidates =
            candidates_for_context(envelope, start..end, loop_hint, mode, work, cancelled)?;
        work.finish(Stage::Candidate, clock);
        if mode.rival_paths() {
            disputes.push(candidates.clone());
        }
        let guard = candidates.first().is_some_and(|guard| {
            outside(guard.period)
                && !candidates.iter().any(|candidate| {
                    !outside(candidate.period) && candidate.score >= guard.score - 0.07
                })
        });
        if let Some(trace) = work.profile.as_mut().and_then(|p| p.contexts.last_mut()) {
            trace.guard = guard;
            trace
                .kept_periods
                .extend(candidates.iter().map(|c| c.period));
        }
        if guard {
            any_guard = true;
            if !mode.rival_paths() {
                candidates.clear();
            }
        }
        candidates.retain(|c| !outside(c.period));
        contexts.push(candidates);
    }
    let clock = work.start();
    let path = tempo_path(&contexts, work, cancelled)?;
    work.finish(Stage::Contour, clock);
    if let Some(profile) = &mut work.profile {
        for (trace, &period) in profile.contexts[trace_start..].iter_mut().zip(&path) {
            trace.selected_period = period;
        }
    }
    let mut left = 0;
    let mut recovery = None;
    while left < path.len() {
        let Some(period) = path[left] else {
            left += 1;
            continue;
        };
        let mut right = left + 1;
        while right < path.len()
            && path[right].is_some_and(|next| (next / path[right - 1].unwrap()).ln().abs() <= 0.09)
        {
            right += 1;
        }
        let start = if left == 0 {
            span.start
        } else {
            (times[left - 1] + times[left]) / 2
        };
        let end = if right == times.len() {
            span.end
        } else {
            (times[right - 1] + times[right]) / 2
        };
        let range = start..end;
        if range.len() >= 48 {
            let periods = interpolate_path(range.clone(), &times[left..right], &path[left..right]);
            let balanced = verify::path_uses_balanced(&contexts[left..right], &path[left..right]);
            if mode.rival_paths() {
                let decision = verify::decide(
                    envelope,
                    range.clone(),
                    &times[left..right],
                    &path[left..right],
                    &periods,
                    &disputes[left..right],
                    balanced,
                    loop_hint,
                    false,
                    work,
                    cancelled,
                )?;
                match decision {
                    verify::Decision::Run(run) => append_run(result, run, &periods),
                    verify::Decision::Ambiguous => result.ambiguous = true,
                    verify::Decision::Outside => any_guard = true,
                    verify::Decision::Weak => {}
                }
            } else {
                let alternative = supported_rival(period, &contexts[left..right], work, cancelled)?;
                if alternative.is_some_and(|rival| !is_octave(period, rival)) {
                    result.ambiguous = true;
                } else {
                    match decode(
                        envelope,
                        range.clone(),
                        &periods,
                        balanced,
                        false,
                        work,
                        cancelled,
                    )? {
                        Decoded::Run(mut run) => {
                            if let Some(rival) = alternative {
                                let multiplier = if rival > period { 2.0 } else { 0.5 };
                                let rival_periods: Vec<_> =
                                    periods.iter().map(|value| value * multiplier).collect();
                                if let Decoded::Run(other) = decode(
                                    envelope,
                                    range.clone(),
                                    &rival_periods,
                                    balanced,
                                    false,
                                    work,
                                    cancelled,
                                )? {
                                    run.octave = Some(other.bpm);
                                }
                            }
                            if mode.targeted_selection()
                                && path[..left].iter().all(Option::is_none)
                                && path[right..].iter().all(Option::is_none)
                                && let Some(hint) = loop_hint
                                && let Some(challenger) = verify::select_tempo(
                                    envelope,
                                    verify::TempoDispute {
                                        run: &run,
                                        span: span.clone(),
                                        contexts: &contexts,
                                        leader: &periods,
                                        duration_bpm: hint,
                                    },
                                    work,
                                    cancelled,
                                )?
                            {
                                run = challenger;
                            }
                            append_run(result, run, &periods);
                        }
                        Decoded::Ambiguous => result.ambiguous = true,
                        Decoded::Weak => {}
                    }
                }
            }
            if mode == Mode::PercussionRecovery
                && !any_guard
                && loop_hint.is_some()
                && result.ambiguous
                && result.runs.is_empty()
                && path[..left].iter().all(Option::is_none)
                && path[right..].iter().all(Option::is_none)
            {
                recovery = Some((left..right, range, periods));
            }
        }
        left = right;
    }
    if let Some((indices, range, periods)) = recovery
        && let verify::Decision::Run(run) = verify::decide(
            envelope,
            range,
            &times[indices.clone()],
            &path[indices.clone()],
            &periods,
            &contexts[indices],
            false,
            loop_hint,
            true,
            work,
            cancelled,
        )?
    {
        result.ambiguous = false;
        append_run(result, run, &periods);
    }
    result.outside |= any_guard && result.runs.is_empty();
    Ok(())
}

fn append_run(result: &mut Track, run: Run, periods: &[f64]) {
    if let Some((&first, &last)) = periods.first().zip(periods.last())
        && run.beats.len() >= 6
        && (last / first).ln().abs() >= 0.06
    {
        result.variable = true;
    }
    result.runs.push(run);
}

fn candidates_for_context(
    envelope: &Envelope,
    range: Range<usize>,
    hint: Option<f64>,
    mode: Mode,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<Candidate>, Error> {
    let mut primary = candidates(
        &envelope.smooth[range.clone()],
        hint,
        mode,
        None,
        work,
        cancelled,
    )?;
    let Some(bands) = &envelope.balanced else {
        return Ok(primary);
    };
    let secondary = &bands.smooth[range.clone()];
    let peaks: Vec<_> = landmarks(secondary)
        .into_iter()
        .filter(|&(i, _)| {
            let bin = range.start + i;
            envelope.flux[bin.saturating_sub(2)..(bin + 3).min(envelope.flux.len())]
                .iter()
                .any(|&strength| strength >= 0.009)
                && envelope.rms[bin.saturating_sub(3)..(bin + 4).min(envelope.rms.len())]
                    .iter()
                    .any(|&level| level >= 0.0003)
        })
        .collect();
    let needs_search = if let Some(&leader) = primary.first() {
        let contested = primary.iter().any(|c| {
            (c.period / leader.period - 1.0).abs() > 0.065 && c.score >= leader.score - 0.14
        });
        let mut changed = false;
        if peaks.len() >= 3
            && let Some(bpm) = hint
        {
            let grid = Grid::new(&peaks);
            work.visit(peaks.len() * 16 * 3)?;
            let reference = grid.fit(leader.period, secondary.len());
            changed = [6000.0 / bpm, 12000.0 / bpm].into_iter().any(|period| {
                (period / leader.period - 1.0).abs() > 0.065
                    && !is_octave(leader.period, period)
                    && grid.fit(period, secondary.len()) >= (reference - 0.07).max(0.47)
            });
        }
        outside(leader.period) || leader.score < 0.56 || contested || changed
    } else {
        true
    };
    if needs_search && peaks.len() >= 3 {
        let alternative = candidates(secondary, hint, mode, Some(peaks), work, cancelled)?;
        // Keep room for corroborated broadband alternatives and for band candidates.
        // Replacing a near-duplicate preserves the stronger evidence and its cue.
        for candidate in alternative.into_iter().take(6) {
            if let Some(existing) = primary
                .iter_mut()
                .find(|c| (c.period / candidate.period - 1.0).abs() < 0.025)
            {
                if candidate.score > existing.score + 0.045 {
                    *existing = candidate;
                }
            } else if primary.len() < MAX_CANDIDATES {
                primary.push(candidate);
            } else if let Some(last) = primary.last_mut()
                && candidate.score > last.score
            {
                *last = candidate;
            }
            sort_candidates(&mut primary);
        }
    }
    Ok(primary)
}

fn sort_candidates(values: &mut [Candidate]) {
    values.sort_unstable_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.period.total_cmp(&b.period))
    });
}

fn outside(period: f64) -> bool {
    !(MIN_PERIOD - 0.12..=MAX_PERIOD + 0.6).contains(&period)
}

fn is_octave(a: f64, b: f64) -> bool {
    (a.max(b) / a.min(b) - 2.0).abs() <= 0.06
}

fn candidates(
    signal: &[f64],
    hint: Option<f64>,
    mode: Mode,
    verified_peaks: Option<Vec<(usize, f64)>>,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<Candidate>, Error> {
    let balanced = verified_peaks.is_some();
    let peaks = verified_peaks.unwrap_or_else(|| landmarks(signal));
    if peaks.len() < 3 {
        return Ok(Vec::new());
    }
    let mut acf = [0.0; 4 * MAX_LAG + 2];
    let mut sums = Vec::with_capacity(signal.len() + 1);
    sums.push([0.0; 2]);
    for &x in signal {
        let [s, q] = sums[sums.len() - 1];
        sums.push([s + x, q + x * x]);
    }
    for (lag, value) in acf.iter_mut().enumerate().skip(MIN_LAG - 1) {
        if lag * 5 > signal.len() * 3 {
            break;
        }
        if lag % 16 == 0 {
            check_cancelled(cancelled)?;
        }
        let n = signal.len() - lag;
        work.visit(n)?;
        let [a, a_sq] = sums[n];
        let [b, b_sq] = [
            sums[signal.len()][0] - sums[lag][0],
            sums[signal.len()][1] - sums[lag][1],
        ];
        let inv = 1.0 / n as f64;
        let cross: f64 = signal.iter().zip(&signal[lag..]).map(|(x, y)| x * y).sum();
        let normalizer = ((a_sq - a * a * inv).max(0.0) * (b_sq - b * b * inv).max(0.0)).sqrt();
        *value = (cross - a * b * inv) / normalizer.max(1e-10);
    }
    let enhanced =
        |lag: usize| acf[lag] + 0.5 * acf[2 * lag].max(0.0) + 0.25 * acf[4 * lag].max(0.0);
    let mut peaks_by_lag: Vec<_> = (MIN_LAG..=MAX_LAG)
        .filter(|&lag| acf[lag] >= 0.11 && acf[lag] >= acf[lag - 1] && acf[lag] > acf[lag + 1])
        .map(|lag| (enhanced(lag), refine(&acf, lag)))
        .collect();
    peaks_by_lag.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.total_cmp(&b.1)));
    let mut pool = Vec::with_capacity(MAX_CANDIDATES);
    let hint_first = mode.band_evidence() || mode.rival_paths();
    if let Some(bpm) = hint.filter(|_| hint_first) {
        add_candidate(&mut pool, 6000.0 / bpm);
        add_candidate(&mut pool, 12000.0 / bpm);
    }
    for (_, period) in peaks_by_lag.into_iter().take(10) {
        for factor in [1.0, 0.5, 2.0] {
            add_candidate(&mut pool, period * factor);
        }
    }
    if let Some(bpm) = hint.filter(|_| !hint_first) {
        add_candidate(&mut pool, 6000.0 / bpm);
        add_candidate(&mut pool, 12000.0 / bpm);
    }
    let reused = (!mode.original_grid()).then(|| Grid::new(&peaks));
    if let Some(trace) = work.profile.as_mut().and_then(|p| p.contexts.last_mut()) {
        trace.landmarks += peaks.len();
        trace.proposed_periods.extend(pool.iter().copied());
    }
    let mut out = Vec::new();
    for period in pool {
        check_cancelled(cancelled)?;
        work.visit(peaks.len() * 22)?;
        let fit = reused.as_ref().map_or_else(
            || original_grid_fit(&peaks, period, signal.len()),
            |grid| grid.fit(period, signal.len()),
        );
        let lag = period.round() as usize;
        let corr = acf[lag].max(0.0);
        let harmonic = acf[2 * lag].max(0.0);
        if fit >= 0.38 && corr.max(harmonic * 0.7) >= 0.12 {
            out.push(Candidate {
                period,
                score: 0.70 * fit + 0.22 * corr + 0.08 * harmonic,
                balanced,
            });
        }
    }
    sort_candidates(&mut out);
    if let Some(trace) = work.profile.as_mut().and_then(|p| p.contexts.last_mut()) {
        trace.qualified_periods.extend(out.iter().map(|c| c.period));
    }
    Ok(out)
}

fn add_candidate(pool: &mut Vec<f64>, period: f64) {
    if (MIN_LAG as f64..=MAX_LAG as f64).contains(&period)
        && pool.len() < MAX_CANDIDATES
        && pool
            .iter()
            .all(|&other| (period / other - 1.0).abs() > 0.021)
    {
        pool.push(period);
    }
}

fn refine(signal: &[f64], i: usize) -> f64 {
    if i == 0 || i + 1 >= signal.len() {
        return i as f64;
    }
    let curvature = signal[i - 1] - 2.0 * signal[i] + signal[i + 1];
    if curvature < -1e-10 {
        i as f64 + (0.5 * (signal[i - 1] - signal[i + 1]) / curvature).clamp(-0.5, 0.5)
    } else {
        i as f64
    }
}

fn landmarks(signal: &[f64]) -> Vec<(usize, f64)> {
    let peak = signal.iter().copied().fold(0.0, f64::max);
    let mean = signal.iter().sum::<f64>() / signal.len().max(1) as f64;
    let floor = (peak * 0.075).max(mean * 1.12).max(0.008);
    let mut peaks = Vec::new();
    for i in 1..signal.len().saturating_sub(1) {
        let value = signal[i];
        if value < floor || value <= signal[i - 1] || value < signal[i + 1] {
            continue;
        }
        if let Some((old, weight)) = peaks.last_mut()
            && i - *old < 6
        {
            if value > *weight {
                (*old, *weight) = (i, value);
            }
        } else {
            peaks.push((i, value));
        }
    }
    peaks
}

struct Grid {
    peaks: Vec<(usize, f64)>,
    anchors: Vec<(usize, f64)>,
    total: f64,
}

impl Grid {
    fn new(source: &[(usize, f64)]) -> Self {
        let peak = source.iter().map(|p| p.1).fold(0.0, f64::max);
        let peaks: Vec<_> = source
            .iter()
            .map(|&(time, weight)| (time, (weight / peak).sqrt()))
            .collect();
        let total = peaks.iter().map(|p| p.1).sum();
        let mut anchors = source.to_vec();
        anchors.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        Self {
            peaks,
            anchors,
            total,
        }
    }

    fn fit(&self, period: f64, bins: usize) -> f64 {
        let tolerance = (period * 0.095).clamp(2.0, 4.0);
        let mut best = 0.0_f64;
        for &(phase, _) in self.anchors.iter().take(16) {
            let mut slots = 0;
            let mut adjacent = 0;
            let mut previous = i32::MIN;
            let mut strength = 0.0;
            let mut slot_max = 0.0_f64;
            let mut accents = [0.0_f64; 2];
            let mut counts = [0_usize; 2];
            for &(time, weight) in &self.peaks {
                let pos = (time as f64 - phase as f64) / period;
                let slot = pos.round() as i32;
                let error = (pos - f64::from(slot)).abs() * period;
                if error > tolerance {
                    continue;
                }
                let weight = weight * (1.0 - 0.20 * error / tolerance);
                if previous == slot {
                    strength += (weight - slot_max).max(0.0);
                    slot_max = slot_max.max(weight);
                } else {
                    adjacent += usize::from(previous + 1 == slot);
                    slots += 1;
                    previous = slot;
                    strength += weight;
                    slot_max = weight;
                    let parity = slot.rem_euclid(2) as usize;
                    accents[parity] += weight * weight;
                    counts[parity] += 1;
                }
            }
            if slots < 3 || adjacent < 2 {
                continue;
            }
            let coverage =
                (slots as f64 / ((bins as f64 - 1.0) / period).floor().max(1.0)).min(1.0);
            let explained = strength / self.total;
            let contrast = explained / (2.0 * tolerance / period).max(0.1);
            if contrast >= 1.4 {
                let contrast = if counts.iter().all(|&n| n >= 3) {
                    let even = accents[0] / counts[0] as f64;
                    let odd = accents[1] / counts[1] as f64;
                    let alternation = (even - odd).abs() / even.max(odd).max(1e-10);
                    (alternation - 0.25).max(0.0) * 0.40
                } else {
                    0.0
                };
                best = best.max(0.48 * coverage + 0.52 * explained - contrast);
            }
        }
        best
    }
}

fn original_grid_fit(peaks: &[(usize, f64)], period: f64, bins: usize) -> f64 {
    let peak = peaks.iter().map(|p| p.1).fold(0.0, f64::max);
    let total: f64 = peaks.iter().map(|p| (p.1 / peak).sqrt()).sum();
    let mut anchors = peaks.to_vec();
    anchors.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    let tolerance = (period * 0.095).clamp(2.0, 4.0);
    let mut best = 0.0_f64;
    for &(phase, _) in anchors.iter().take(16) {
        let mut slots = 0;
        let mut adjacent = 0;
        let mut previous = i32::MIN;
        let mut strength = 0.0;
        let mut slot_max = 0.0_f64;
        let mut accents = [0.0_f64; 2];
        let mut counts = [0_usize; 2];
        for &(time, weight) in peaks {
            let pos = (time as f64 - phase as f64) / period;
            let slot = pos.round() as i32;
            let error = (pos - f64::from(slot)).abs() * period;
            if error > tolerance {
                continue;
            }
            let weight = (weight / peak).sqrt() * (1.0 - 0.20 * error / tolerance);
            if previous == slot {
                strength += (weight - slot_max).max(0.0);
                slot_max = slot_max.max(weight);
            } else {
                adjacent += usize::from(previous + 1 == slot);
                slots += 1;
                previous = slot;
                strength += weight;
                slot_max = weight;
                let parity = slot.rem_euclid(2) as usize;
                accents[parity] += weight * weight;
                counts[parity] += 1;
            }
        }
        if slots < 3 || adjacent < 2 {
            continue;
        }
        let coverage = (slots as f64 / ((bins as f64 - 1.0) / period).floor().max(1.0)).min(1.0);
        let explained = strength / total;
        let contrast = explained / (2.0 * tolerance / period).max(0.1);
        if contrast >= 1.4 {
            let contrast = if counts.iter().all(|&n| n >= 3) {
                let even = accents[0] / counts[0] as f64;
                let odd = accents[1] / counts[1] as f64;
                let alternation = (even - odd).abs() / even.max(odd).max(1e-10);
                (alternation - 0.25).max(0.0) * 0.40
            } else {
                0.0
            };
            best = best.max(0.48 * coverage + 0.52 * explained - contrast);
        }
    }
    best
}

fn tempo_path(
    contexts: &[Vec<Candidate>],
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<Option<f64>>, Error> {
    let mut trace = Vec::with_capacity(contexts.len());
    let mut prev_scores = vec![0.0];
    let mut previous: &[Candidate] = &[];
    for current in contexts {
        check_cancelled(cancelled)?;
        work.visit((current.len() + 1) * (previous.len() + 1))?;
        let mut row = Vec::with_capacity(current.len() + 1);
        let mut scores = Vec::with_capacity(current.len() + 1);
        for i in 0..=current.len() {
            let here = current.get(i);
            let reward = here.map_or(0.42, |c| c.score);
            let (from, score) = prev_scores
                .iter()
                .enumerate()
                .map(|(j, &score)| {
                    let penalty = match (here, previous.get(j)) {
                        (Some(a), Some(b)) => (a.period / b.period).ln().abs().min(0.8) * 0.45,
                        (None, None) => 0.0,
                        _ => 0.065,
                    };
                    (j, score - penalty)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            row.push(from);
            scores.push(reward + score);
        }
        trace.push(row);
        prev_scores = scores;
        previous = current;
    }
    let mut chosen = prev_scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    let mut path = Vec::with_capacity(contexts.len());
    for (row, candidates) in trace.iter().zip(contexts).rev() {
        path.push(candidates.get(chosen).map(|c| c.period));
        chosen = row[chosen];
    }
    path.reverse();
    Ok(path)
}

fn supported_rival(
    period: f64,
    contexts: &[Vec<Candidate>],
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Option<f64>, Error> {
    let mut conflicting = None;
    let mut octave = None;
    let mut conflict_count = 0;
    let mut octave_count = 0;
    for options in contexts {
        check_cancelled(cancelled)?;
        work.visit(options.len() * 3)?;
        let Some(best) = options
            .iter()
            .find(|c| (c.period / period).ln().abs() <= 0.1)
        else {
            continue;
        };
        let mut found_other = false;
        let mut found_octave = false;
        for rival in options {
            let ratio = rival.period / best.period;
            if (ratio - 1.0).abs() <= 0.065 {
                continue;
            }
            let same_family = is_octave(best.period, rival.period);
            if best.score - rival.score > if same_family { 0.055 } else { 0.032 } {
                continue;
            }
            if same_family {
                octave = Some(rival.period);
                found_octave = true;
            } else {
                conflicting = Some(rival.period);
                found_other = true;
            }
        }
        conflict_count += usize::from(found_other);
        octave_count += usize::from(found_octave);
    }
    Ok(if conflict_count * 2 >= contexts.len() {
        conflicting
    } else if octave_count * 2 >= contexts.len() {
        octave
    } else {
        None
    })
}

fn interpolate_path(range: Range<usize>, times: &[usize], periods: &[Option<f64>]) -> Vec<f64> {
    let mut i = 0;
    range
        .map(|bin| {
            while i + 1 < times.len() && bin >= times[i + 1] {
                i += 1;
            }
            let left = periods[i].unwrap();
            let Some(right) = periods.get(i + 1).and_then(|p| *p) else {
                return left;
            };
            let delta =
                bin.saturating_sub(times[i]) as f64 / (times[i + 1] - times[i]).max(1) as f64;
            left + delta * (right - left)
        })
        .collect()
}

fn cumulative(
    signal: &[f64],
    periods: &[f64],
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<f64>, Error> {
    let mut history = vec![0.0_f64; signal.len()];
    let mut weights = Vec::new();
    let mut saved_period = 0.0;
    let mut first = 0;
    for (i, (&strength, &period)) in signal.iter().zip(periods).enumerate() {
        if i % 32 == 0 {
            check_cancelled(cancelled)?;
        }
        let key = (period * 4.0).round() / 4.0;
        if key != saved_period {
            first = (key * 0.5).ceil() as usize;
            weights.clear();
            weights.extend(
                (first..=(key * 2.0).floor() as usize)
                    .map(|lag| (-0.5 * (4.0 * (lag as f64 / key).ln()).powi(2)).exp()),
            );
            saved_period = key;
        }
        let mut maximum = 0.0_f64;
        if i >= first {
            let count = (i - first + 1).min(weights.len());
            work.visit(count)?;
            for (offset, &weight) in weights[..count].iter().enumerate() {
                maximum = maximum.max(history[i - first - offset] * weight);
            }
        }
        history[i] = 0.25 * strength + 0.75 * maximum;
    }
    Ok(history)
}

fn decode(
    envelope: &Envelope,
    range: Range<usize>,
    periods: &[f64],
    balanced: bool,
    verify_period: bool,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Decoded, Error> {
    let clock = work.start();
    let result = decode_inner(
        envelope,
        range,
        periods,
        balanced,
        verify_period,
        work,
        cancelled,
    );
    work.finish(Stage::Decode, clock);
    result
}

fn decode_inner(
    envelope: &Envelope,
    range: Range<usize>,
    periods: &[f64],
    balanced: bool,
    verify_period: bool,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Decoded, Error> {
    let view = envelope.balanced.as_ref().filter(|_| balanced);
    let signal = &view.map_or(&envelope.smooth, |v| &v.smooth)[range.clone()];
    let flux = view.map_or(&envelope.flux, |v| &v.flux);
    let onsets = landmarks(signal);
    if onsets.len() < 3 {
        return Ok(Decoded::Weak);
    }
    let peak = signal.iter().copied().fold(0.0, f64::max);
    let clock = work.start();
    let cbss = cumulative(signal, periods, work, cancelled)?;
    work.finish(Stage::Recurrence, clock);
    let mean = cbss.iter().sum::<f64>() / cbss.len() as f64;
    let mut crests = Vec::new();
    for i in 1..cbss.len().saturating_sub(1) {
        if cbss[i] <= cbss[i - 1] || cbss[i] < cbss[i + 1] {
            continue;
        }
        if !onsets.iter().any(|&(index, _)| index.abs_diff(i) <= 4) {
            continue;
        }
        let radius = (periods[i] * 0.24).round() as usize;
        let around = i.saturating_sub(radius)..(i + radius + 1).min(cbss.len());
        work.visit(around.len() + onsets.len())?;
        let prominence = cbss[i] - mean;
        if prominence > 0.0
            && !cbss[around.clone()]
                .iter()
                .enumerate()
                .any(|(j, &value)| value > cbss[i] || (value == cbss[i] && around.start + j < i))
        {
            crests.push(i);
        }
    }
    for &(index, _) in &onsets {
        if (index <= periods[index] as usize || signal.len() - index <= periods[index] as usize)
            && !crests.iter().any(|&peak| index.abs_diff(peak) <= 4)
        {
            crests.push(index);
        }
    }
    crests.sort_unstable();
    if crests.len() < 3 {
        return Ok(Decoded::Weak);
    }
    let phase: Vec<_> = std::iter::once(0.0)
        .chain(periods.iter().scan(0.0, |phase, p| {
            *phase += 1.0 / p;
            Some(*phase)
        }))
        .collect();
    let mut nodes = Vec::with_capacity(crests.len());
    for (i, &bin) in crests.iter().enumerate() {
        if i % 16 == 0 {
            check_cancelled(cancelled)?;
        }
        let mut best = (weight(signal, &cbss, bin, peak), None);
        for (j, &previous) in crests[..i].iter().enumerate().rev() {
            let delta = phase[bin] - phase[previous];
            if delta > 2.18 {
                break;
            }
            work.visit(1)?;
            let steps = delta.round();
            if steps < 1.0 || (delta - steps).abs() > 0.18 {
                continue;
            }
            let (score, _) = nodes[j];
            let score = score + weight(signal, &cbss, bin, peak)
                - 1.8 * (delta - steps).abs()
                - 0.85 * (steps - 1.0);
            if score > best.0 {
                best = (score, Some(j));
            }
        }
        nodes.push(best);
    }
    let mut cursor = nodes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.0.total_cmp(&b.1.0))
        .map(|(i, _)| i)
        .unwrap();
    let mut path = Vec::new();
    loop {
        path.push(crests[cursor]);
        let Some(previous) = nodes[cursor].1 else {
            break;
        };
        cursor = previous;
    }
    path.reverse();
    let initial_quality = sequence_quality(&path, &phase, signal, &cbss, peak);
    let mut sequences = vec![(initial_quality, path)];
    let mut anchors = crests.clone();
    anchors.sort_unstable_by(|&a, &b| cbss[b].total_cmp(&cbss[a]));
    for anchor in anchors.into_iter().take(16) {
        check_cancelled(cancelled)?;
        work.visit(crests.len())?;
        let mut pulse: Vec<usize> = Vec::new();
        for &bin in &crests {
            let relative = phase[bin] - phase[anchor];
            if (relative - relative.round()).abs() > 0.13 {
                continue;
            }
            if let Some(last) = pulse.last_mut()
                && (phase[*last] - phase[anchor]).round() == relative.round()
            {
                if cbss[bin] > cbss[*last] {
                    *last = bin;
                }
            } else {
                pulse.push(bin);
            }
        }
        let quality = sequence_quality(&pulse, &phase, signal, &cbss, peak);
        if pulse.len() >= 3 {
            sequences.push((quality, pulse));
        }
    }
    sequences.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (quality, path) = &sequences[0];
    if path.len() < 3 {
        return Ok(Decoded::Weak);
    }
    for (alternative_quality, alternative) in &sequences[1..] {
        if alternative.len() * 5 < path.len() * 4
            || quality - alternative_quality > path.len() as f64 * 0.012
        {
            continue;
        }
        let separation = phase[path[0]] - phase[alternative[0]];
        let overlap =
            alternative[0] < *path.last().unwrap() && path[0] < *alternative.last().unwrap();
        if overlap && (separation - separation.round()).abs() > 0.25 {
            return Ok(Decoded::Ambiguous);
        }
    }
    let mut intervals = Vec::new();
    let mut beats = Vec::new();
    for (i, &bin) in path.iter().enumerate() {
        let clock = (range.start + bin) as f64;
        let raw_peak = (bin.saturating_sub(2)..(bin + 3).min(signal.len()))
            .max_by(|&a, &b| flux[range.start + a].total_cmp(&flux[range.start + b]));
        let time = raw_peak.map_or(clock, |p| {
            (refine(flux, range.start + p)).clamp(
                (clock - 2.5).max(range.start as f64),
                (clock + 2.5).min(range.end as f64 - 0.5),
            )
        });
        if i > 0 {
            let previous = path[i - 1];
            let distance = phase[bin] - phase[previous];
            if (distance - 1.0).abs() <= 0.20 {
                intervals.push(time - beats.last().map(|&(time, _)| time).unwrap_or(time));
            } else if (distance - 2.0).abs() <= 0.13 {
                let middle = (bin + previous) / 2;
                let nearby = &envelope.rms[range.start + middle.saturating_sub(3)
                    ..(range.start + middle + 4).min(range.end)];
                if cbss[middle] >= 0.62 * cbss[bin].min(cbss[previous])
                    && nearby.iter().any(|&rms| rms >= 0.0003)
                {
                    beats.push(((range.start + middle) as f64, true));
                }
            }
        }
        beats.push((time, false));
    }
    if intervals.len() < 2 {
        return Ok(Decoded::Weak);
    }
    intervals.sort_unstable_by(f64::total_cmp);
    let n = intervals.len();
    let period = (intervals[(n - 1) / 2] + intervals[n / 2]) * 0.5;
    let bpm = 6000.0 / period;
    let bpm = if verify_period {
        let local = periods[periods.len() / 2];
        if (period / local).ln().abs() > 0.08 {
            return Ok(Decoded::Weak);
        }
        bpm
    } else {
        bpm.clamp(45.0, 240.0)
    };
    Ok(Decoded::Run(Run {
        range,
        bpm,
        octave: None,
        beats,
    }))
}

fn weight(signal: &[f64], cbss: &[f64], bin: usize, peak: f64) -> f64 {
    (0.65 * signal[bin] + 0.35 * cbss[bin]) / peak.max(1e-10)
}

fn sequence_quality(path: &[usize], phase: &[f64], signal: &[f64], cbss: &[f64], peak: f64) -> f64 {
    if path.len() < 3 {
        return 0.0;
    }
    let penalties: f64 = path
        .windows(2)
        .map(|pair| {
            let delta = phase[pair[1]] - phase[pair[0]];
            1.8 * (delta - delta.round()).abs() + 0.85 * (delta.round() - 1.0).abs()
        })
        .sum();
    path.iter()
        .map(|&bin| weight(signal, cbss, bin, peak))
        .sum::<f64>()
        - penalties
}
