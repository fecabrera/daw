use super::{
    Candidate, Decoded, Envelope, MAX_PERIOD, MIN_PERIOD, Run, Work, check_cancelled, decode,
    interpolate_path, is_octave, landmarks, outside,
};
use crate::Error;
use crate::obtain::Stage;
use std::ops::Range;

pub(super) enum Decision {
    Run(Run),
    Ambiguous,
    Outside,
    Weak,
}

pub(super) fn path_uses_balanced(contexts: &[Vec<Candidate>], path: &[Option<f64>]) -> bool {
    let count = contexts
        .iter()
        .zip(path)
        .filter(|(candidates, rate)| {
            rate.is_some_and(|rate| candidates.iter().any(|c| c.period == rate && c.balanced))
        })
        .count();
    count * 2 > path.len()
}

struct Contour {
    periods: Vec<f64>,
    balanced: bool,
}

fn compatible(value: f64, reference: f64) -> bool {
    (value / reference).ln().abs() <= 0.044
}

fn matches_loop(period: f64, hint: Option<f64>) -> bool {
    hint.is_some_and(|bpm| {
        let reference = 6000.0 / bpm;
        [reference * 0.5, reference, reference * 2.0]
            .into_iter()
            .any(|p| compatible(period, p))
    })
}

// Evaluate the same ratio in adjacent contexts. This avoids treating
// unrelated peaks in successive contexts as one rival.
fn rival_contours(
    times: &[usize],
    path: &[Option<f64>],
    contexts: &[Vec<Candidate>],
    range: Range<usize>,
    hint: Option<f64>,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<[Option<Contour>; 2], Error> {
    let mut hypotheses = Vec::<f64>::new();
    // Candidates are already ordered by local evidence. Sample candidate seeds
    // evenly so a single context at a boundary cannot own the shortlist.
    for (options, lead) in contexts
        .iter()
        .zip(path)
        .step_by((contexts.len() / 5).max(1))
    {
        for c in options.iter().take(12) {
            let ratio = c.period / lead.unwrap();
            if (ratio - 1.0).abs() > 0.067
                && hypotheses.iter().all(|&p| !compatible(p, ratio))
                && hypotheses.len() < 24
            {
                hypotheses.push(ratio);
            }
        }
    }
    let mut selected = [None, None];
    let mut best = [0.0_f64; 2];
    for ratio in hypotheses {
        check_cancelled(cancelled)?;
        let octave = is_octave(ratio, 1.0);
        let class = usize::from(!octave);
        let mut contour = Vec::with_capacity(contexts.len());
        let mut count = 0;
        let mut band = 0;
        let mut sum = 0.0;
        for (options, &lead) in contexts.iter().zip(path) {
            let lead = lead.unwrap();
            let target = ratio * lead;
            work.visit(options.len() * 2)?;
            let top = options
                .iter()
                .find(|c| compatible(c.period, lead))
                .map_or(0.5, |c| c.score);
            let slack = if octave { 0.16 } else { 0.145 };
            let rival = options.iter().find(|c| {
                let margin = if matches_loop(c.period, hint) {
                    slack + 0.07
                } else {
                    slack
                };
                compatible(c.period, target) && c.score >= (top - margin).max(0.39)
            });
            contour.push(Some(rival.map_or(target, |c| c.period)));
            if let Some(c) = rival {
                count += 1;
                band += usize::from(c.balanced);
                sum += c.score
                    + if matches_loop(c.period, hint) && !outside(c.period) {
                        0.09
                    } else {
                        0.0
                    };
            }
        }
        if count * 2 < contexts.len()
            || contour.windows(2).any(|pair| {
                let a = pair[0].unwrap();
                let b = pair[1].unwrap();
                (a / b).ln().abs() > 0.12
            })
        {
            continue;
        }
        let strength = sum / contexts.len() as f64;
        if strength > best[class] {
            best[class] = strength;
            selected[class] = Some(Contour {
                periods: interpolate_path(range.clone(), times, &contour),
                balanced: band * 2 > count,
            });
        }
    }
    Ok(selected)
}

struct Evidence {
    anchors: Vec<(usize, f64)>,
    total: f64,
}

struct Support {
    score: f64,
    coverage: f64,
    direct_intervals: usize,
    mean_attack: f64,
}

impl Evidence {
    fn new(envelope: &Envelope, range: Range<usize>, work: &mut Work) -> Result<Self, Error> {
        let mut candidates = landmarks(&envelope.smooth[range.clone()]);
        if let Some(bands) = &envelope.balanced {
            candidates.extend(landmarks(&bands.smooth[range.clone()]));
        }
        work.visit(candidates.len() * 14)?;
        let mut anchors: Vec<(usize, f64)> = Vec::new();
        candidates.sort_unstable_by_key(|&(i, _)| i);
        for (i, _) in candidates {
            let bin = i + range.start;
            let window = bin.saturating_sub(3).max(range.start)..(bin + 4).min(range.end);
            if !envelope.rms[window.clone()].iter().any(|&v| v >= 0.0003) {
                continue;
            }
            let Some(index) = window.max_by(|&a, &b| envelope.flux[a].total_cmp(&envelope.flux[b]))
            else {
                continue;
            };
            let power = envelope.flux[index];
            if power < 0.009 {
                continue;
            }
            let weight = power.sqrt();
            if let Some(last) = anchors.last_mut()
                && index.abs_diff(last.0) <= 5
            {
                if weight > last.1 {
                    *last = (index, weight);
                }
            } else {
                anchors.push((index, weight));
            }
        }
        let total = anchors.iter().map(|&(_, w)| w).sum();
        Ok(Self { anchors, total })
    }

    fn score(&self, run: &Run, work: &mut Work) -> Result<Option<Support>, Error> {
        if self.anchors.len() < 3 {
            return Ok(None);
        }
        work.visit(run.beats.len() * 12 + self.anchors.len())?;
        let first = self.anchors[0].0 as f64;
        let last = self.anchors[self.anchors.len() - 1].0 as f64;
        let period = 6000.0 / run.bpm;
        let mut matches = 0;
        let mut matched_weight = 0.0;
        let mut residual = 0.0;
        let mut directly_consecutive = 0;
        let mut consecutive = 0;
        let mut longest = 0;
        let mut previous = None;
        let mut found_halves = [false; 2];
        let mut cursor = 0;
        for &(time, inferred) in &run.beats {
            if inferred {
                continue;
            }
            while cursor + 1 < self.anchors.len()
                && (self.anchors[cursor + 1].0 as f64 - time).abs()
                    < (self.anchors[cursor].0 as f64 - time).abs()
            {
                cursor += 1;
            }
            let (frame, weight) = self.anchors[cursor];
            let error = (frame as f64 - time).abs();
            if error > 4.0 || previous.is_some_and(|(old, _)| old == cursor) {
                continue;
            }
            if previous
                .is_some_and(|(_, old): (usize, f64)| ((time - old) / period - 1.0).abs() < 0.20)
            {
                directly_consecutive += 1;
                consecutive += 1;
                longest = longest.max(consecutive);
            } else {
                consecutive = 0;
            }
            previous = Some((cursor, time));
            matches += 1;
            matched_weight += weight;
            residual += error;
            found_halves[usize::from(time >= (first + last) * 0.5)] = true;
        }
        if matches < 3
            || longest < 2
            || (last - first > 8.0 * period && found_halves.contains(&false))
        {
            return Ok(None);
        }
        let slots = (((last - first) / period).round() + 1.0).max(3.0);
        let coverage = (matches as f64 / slots).min(1.0);
        let adjacent = (directly_consecutive as f64 / (slots - 1.0)).min(1.0);
        let explained = (matched_weight / self.total).min(1.0);
        let phase = 1.0 - residual / (matches as f64 * 4.0);
        Ok(Some(Support {
            score: 0.40 * coverage + 0.31 * explained + 0.19 * adjacent + 0.10 * phase,
            coverage,
            direct_intervals: directly_consecutive,
            mean_attack: matched_weight / matches as f64,
        }))
    }
}

pub(super) struct TempoDispute<'a> {
    pub run: &'a Run,
    pub span: Range<usize>,
    pub contexts: &'a [Vec<Candidate>],
    pub leader: &'a [f64],
    pub duration_bpm: f64,
}

pub(super) fn select_tempo(
    envelope: &Envelope,
    dispute: TempoDispute<'_>,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Option<Run>, Error> {
    let TempoDispute {
        run,
        span,
        contexts,
        leader,
        duration_bpm,
    } = dispute;
    let predicted = 6000.0 / duration_bpm;
    // Retain the established octave and variable-tempo decisions.
    if contexts.len() < 3
        || outside(predicted)
        || [0.5, 1.0, 2.0]
            .iter()
            .any(|factor| (run.bpm * factor / duration_bpm).ln().abs() <= 0.10)
    {
        return Ok(None);
    }
    work.visit(leader.len() + contexts.iter().map(Vec::len).sum::<usize>())?;
    let (mut low, mut high) = (f64::INFINITY, 0.0_f64);
    for &period in leader {
        low = low.min(period);
        high = high.max(period);
    }
    if high / low > 1.03 {
        return Ok(None);
    }
    let mut periods = Vec::with_capacity(contexts.len());
    for candidates in contexts {
        check_cancelled(cancelled)?;
        if let Some(candidate) = candidates
            .iter()
            .find(|c| (c.period / predicted - 1.0).abs() <= 0.018)
        {
            periods.push(candidate.period);
        }
    }
    if periods.len() * 5 < contexts.len() * 4 {
        return Ok(None);
    }
    periods.sort_unstable_by(f64::total_cmp);
    let evidence = Evidence::new(envelope, span.clone(), work)?;
    // Compare both rates on the same active audio, including any opening
    // or closing contexts that the original contour left unsupported.
    let mut contour = Contour {
        periods: vec![6000.0 / run.bpm; span.len()],
        balanced: false,
    };
    let (incumbent, _) = score_path(
        envelope,
        &evidence,
        span.clone(),
        &contour,
        None,
        work,
        cancelled,
    )?;
    let Some(incumbent) = incumbent else {
        return Ok(None);
    };
    contour.periods.fill(periods[periods.len() / 2]);
    let (scored, _) = score_path(envelope, &evidence, span, &contour, None, work, cancelled)?;
    Ok(scored
        .filter(|other| {
            !outside(6000.0 / other.run.bpm)
                && (other.run.bpm / duration_bpm - 1.0).abs() <= 0.018
                && other.support.coverage >= 0.55
                && other.support.coverage >= incumbent.support.coverage + 0.10
                && other.support.direct_intervals > incumbent.support.direct_intervals
                && other.support.score >= incumbent.support.score + 0.09
        })
        .map(|other| other.run))
}

struct Scored {
    run: Run,
    support: Support,
}

fn score_path(
    envelope: &Envelope,
    evidence: &Evidence,
    range: Range<usize>,
    contour: &Contour,
    hint: Option<f64>,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<(Option<Scored>, bool), Error> {
    let decoded = decode(
        envelope,
        range,
        &contour.periods,
        contour.balanced,
        true,
        work,
        cancelled,
    )?;
    match decoded {
        Decoded::Run(run) => {
            let support = evidence.score(&run, work)?;
            let score = support.map(|mut support| {
                if matches_loop(6000.0 / run.bpm, hint) {
                    support.score += 0.08;
                }
                Scored { run, support }
            });
            Ok((score, false))
        }
        Decoded::Ambiguous => Ok((None, true)),
        Decoded::Weak => Ok((None, false)),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decide(
    envelope: &Envelope,
    range: Range<usize>,
    times: &[usize],
    path: &[Option<f64>],
    periods: &[f64],
    contexts: &[Vec<Candidate>],
    balanced: bool,
    hint: Option<f64>,
    percussion_only: bool,
    work: &mut Work,
    cancelled: &impl Fn() -> bool,
) -> Result<Decision, Error> {
    let clock = work.start();
    let rivals = rival_contours(times, path, contexts, range.clone(), hint, work, cancelled)?;
    work.finish(Stage::Contour, clock);
    let guard_dominates = contexts
        .iter()
        .filter(|candidates| {
            candidates.first().is_some_and(|guard| {
                outside(guard.period)
                    && !candidates.iter().any(|candidate| {
                        !outside(candidate.period) && candidate.score >= guard.score - 0.07
                    })
            })
        })
        .count()
        * 2
        > contexts.len();
    if rivals.iter().all(Option::is_none) && !balanced && !guard_dominates {
        if percussion_only {
            return Ok(Decision::Ambiguous);
        }
        // A clear broadband result incurs no optional path or ledger work.
        return Ok(
            match decode(envelope, range, periods, false, true, work, cancelled)? {
                Decoded::Run(mut run) if !outside(6000.0 / run.bpm) => {
                    run.bpm = run.bpm.clamp(6000.0 / MAX_PERIOD, 6000.0 / MIN_PERIOD);
                    Decision::Run(run)
                }
                Decoded::Run(_) => Decision::Outside,
                Decoded::Ambiguous => Decision::Ambiguous,
                Decoded::Weak => Decision::Weak,
            },
        );
    }
    let evidence = Evidence::new(envelope, range.clone(), work)?;
    if percussion_only {
        work.visit(evidence.anchors.len() * 16)?;
        if !crate::percussion::has_short_attacks(
            range.len(),
            |i| envelope.rms[range.start + i],
            evidence.anchors.iter().map(|&(i, _)| i - range.start),
        ) {
            return Ok(Decision::Ambiguous);
        }
    }
    let leader = Contour {
        periods: periods.to_vec(),
        balanced,
    };
    let mut decoded = Vec::with_capacity(3);
    let mut phase_tie = false;
    for contour in std::iter::once(&leader).chain(rivals.iter().flatten()) {
        check_cancelled(cancelled)?;
        let (result, ambiguous) = score_path(
            envelope,
            &evidence,
            range.clone(),
            contour,
            hint,
            work,
            cancelled,
        )?;
        phase_tie |= ambiguous;
        decoded.extend(result.filter(|scored| {
            scored.support.coverage >= 0.70
                && scored.support.score >= if guard_dominates { 0.84 } else { 0.70 }
        }));
    }
    let Some((mut index, _)) = decoded
        .iter()
        .enumerate()
        .max_by(|(a, first), (b, second)| {
            first
                .support
                .score
                .total_cmp(&second.support.score)
                .then(b.cmp(a))
        })
    else {
        return Ok(if guard_dominates {
            Decision::Outside
        } else if phase_tie {
            Decision::Ambiguous
        } else {
            Decision::Weak
        });
    };
    // Extra subdivisions alone cannot win against a nearly complete,
    // distinctly accented slower pulse with its own directly supported path.
    let leader = &decoded[index];
    if let Some((slower, _)) = decoded.iter().enumerate().find(|(_, rival)| {
        rival.run.bpm < leader.run.bpm
            && is_octave(rival.run.bpm, leader.run.bpm)
            && rival.support.coverage >= 0.78
            && rival.support.mean_attack > leader.support.mean_attack * 1.16
            && rival.support.score >= leader.support.score - 0.13
    }) {
        index = slower;
    }
    let Scored { mut run, support } = decoded.swap_remove(index);
    if outside(6000.0 / run.bpm) {
        return Ok(Decision::Outside);
    }
    if percussion_only && !matches_loop(6000.0 / run.bpm, hint) {
        return Ok(Decision::Ambiguous);
    }
    for other in decoded {
        if compatible(other.run.bpm, run.bpm) {
            continue;
        }
        if is_octave(other.run.bpm, run.bpm) && !outside(6000.0 / other.run.bpm) {
            if (support.score - other.support.score).abs() <= 0.08 {
                run.octave = Some(other.run.bpm);
            }
        } else if support.score - other.support.score <= 0.048 {
            return Ok(Decision::Ambiguous);
        }
    }
    run.bpm = run.bpm.clamp(6000.0 / MAX_PERIOD, 6000.0 / MIN_PERIOD);
    Ok(Decision::Run(run))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_contour_needs_repeatable_and_stronger_direct_support() {
        let span = 0..480;
        let mut signal = vec![0.0; span.len()];
        for bin in (20..280).step_by(60) {
            signal[bin] = 0.22;
        }
        for bin in (20..span.end).step_by(40) {
            signal[bin] = 1.0;
        }
        let envelope = Envelope {
            smooth: signal.clone(),
            flux: signal,
            rms: vec![0.1; span.len()],
            balanced: None,
        };
        let run = Run {
            range: span.clone(),
            bpm: 100.0,
            octave: None,
            beats: Vec::new(),
        };
        let leader = vec![60.0; span.len()];
        let qualified = vec![Candidate {
            period: 40.0,
            score: 0.6,
            balanced: false,
        }];
        let choose = |contexts: &[Vec<Candidate>]| {
            select_tempo(
                &envelope,
                TempoDispute {
                    run: &run,
                    span: span.clone(),
                    contexts,
                    leader: &leader,
                    duration_bpm: 150.0,
                },
                &mut Work::default(),
                &|| false,
            )
            .unwrap()
        };
        assert!(choose(&[qualified.clone(), qualified.clone(), Vec::new()]).is_none());
        let selected = choose(&[qualified.clone(), qualified.clone(), qualified]).unwrap();
        assert!((selected.bpm - 150.0).abs() < 0.1);
        assert_eq!(selected.beats.len(), 12);
        assert!(selected.beats.iter().all(|&(_, inferred)| !inferred));
    }
}
