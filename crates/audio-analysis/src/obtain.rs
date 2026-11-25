//! Offline interpretation of OBTAIN (arXiv:1704.02216v2).
use crate::{
    Error,
    tempo::{Tempo, TempoWindows},
};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

mod onset;
mod track;

const HZ: u32 = 100;
const MAX_BINS: usize = 3_700;
const MAX_BEATS: usize = 192;
const BLOCK: usize = 8192;
pub const SCRATCH_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub frame: u64,
    pub inferred: bool,
}

#[derive(Debug, PartialEq)]
pub struct BeatRun {
    pub range: Range<u64>,
    pub bpm: f64,
    pub octave_bpm: Option<f64>,
    pub beats: Vec<Beat>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Summary {
    Stable { bpm: f64, nearest_100_bpm: f64 },
    Weak,
    Ambiguous,
    Variable,
    OutOfRange,
}

#[derive(Debug, PartialEq)]
pub struct Analysis {
    pub sample_rate: u32,
    pub analyzed: Vec<Range<u64>>,
    pub runs: Vec<BeatRun>,
    pub sampled: bool,
    pub summary: Summary,
}

#[derive(Default)]
pub struct Profile {
    /// Prepared reads, onset extraction, candidate scoring, contour selection,
    /// cumulative recurrence, beat decoding, and publication respectively.
    pub stage_time: [Duration; 7],
    pub read_calls: usize,
    pub read_frames: usize,
    pub transforms: usize,
    pub visits: usize,
    pub contexts: Vec<ContextTrace>,
}

pub struct ContextTrace {
    pub landmarks: usize,
    pub proposed_periods: Vec<f64>,
    pub qualified_periods: Vec<f64>,
    pub kept_periods: Vec<f64>,
    pub guard: bool,
    pub selected_period: Option<f64>,
}

#[derive(Clone, Copy)]
enum Stage {
    Read,
    Onset,
    Candidate,
    Contour,
    Recurrence,
    Decode,
    Publish,
}

#[derive(Default)]
struct Work<'a> {
    visits: usize,
    ffts: usize,
    profile: Option<&'a mut Profile>,
}

impl Work<'_> {
    fn start(&self) -> Option<Instant> {
        self.profile.as_ref().map(|_| Instant::now())
    }

    fn finish(&mut self, stage: Stage, start: Option<Instant>) {
        if let (Some(profile), Some(start)) = (&mut self.profile, start) {
            profile.stage_time[stage as usize] += start.elapsed();
        }
    }

    fn visit(&mut self, count: usize) -> Result<(), Error> {
        self.visits = self
            .visits
            .checked_add(count)
            .filter(|&n| n <= 256_000_000)
            .ok_or(Error::ResourceLimit)?;
        Ok(())
    }

    fn fft(&mut self) -> Result<(), Error> {
        self.ffts += 2;
        if self.ffts > 7400 {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
}

fn check_cancelled(cancelled: &impl Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Standard offline OBTAIN and reproducible comparison variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Legacy,
    Reuse,
    Onsets,
    Rivals,
    Combined,
    #[default]
    Selection,
}

impl Mode {
    fn band_evidence(self) -> bool {
        matches!(self, Self::Onsets | Self::Combined)
    }

    fn rival_paths(self) -> bool {
        matches!(self, Self::Rivals | Self::Combined)
    }

    fn original_grid(self) -> bool {
        matches!(self, Self::Legacy | Self::Selection)
    }
}

/// Analyze already prepared stereo on its source clock. The reader must fill
/// each requested slice exactly. Musical abstentions are successful results.
pub fn analyze<E: From<Error>>(
    rate: u32,
    frames: u64,
    cancelled: &impl Fn() -> bool,
    read: impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
) -> Result<Analysis, E> {
    analyze_mode(rate, frames, Mode::default(), cancelled, read)
}

/// Run a named research variant over the same prepared input contract.
pub fn analyze_mode<E: From<Error>>(
    rate: u32,
    frames: u64,
    mode: Mode,
    cancelled: &impl Fn() -> bool,
    read: impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
) -> Result<Analysis, E> {
    analyze_inner(rate, frames, mode, cancelled, read, &mut Work::default())
}

/// Diagnostic pass. Run separately from timing measurements.
pub fn analyze_profile<E: From<Error>>(
    rate: u32,
    frames: u64,
    mode: Mode,
    cancelled: &impl Fn() -> bool,
    read: impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
    profile: &mut Profile,
) -> Result<Analysis, E> {
    *profile = Profile::default();
    let mut work = Work {
        profile: Some(profile),
        ..Work::default()
    };
    let result = analyze_inner(rate, frames, mode, cancelled, read, &mut work);
    if let Some(profile) = work.profile {
        profile.visits = work.visits;
        profile.transforms = work.ffts;
    }
    result
}

fn analyze_inner<E: From<Error>>(
    rate: u32,
    frames: u64,
    mode: Mode,
    cancelled: &impl Fn() -> bool,
    mut read: impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
    work: &mut Work<'_>,
) -> Result<Analysis, E> {
    let schedule = TempoWindows::new(rate, frames)?;
    check_cancelled(cancelled)?;
    let analyzed = if schedule.sampled || frames == 0 {
        schedule
            .ranges
            .iter()
            .filter(|range| !range.is_empty())
            .cloned()
            .collect()
    } else {
        std::iter::once(0..frames).collect()
    };
    let mut analysis = Analysis {
        sample_rate: rate,
        analyzed,
        runs: Vec::new(),
        sampled: schedule.sampled,
        summary: Summary::Weak,
    };
    let mut spectrum = onset::Spectrum::new(rate, mode.band_evidence())?;
    let mut pcm = onset::Samples::new(rate, frames);
    let mut regions = 0;
    let mut ambiguous = false;
    let mut outside = false;
    let mut variable = false;
    let mut other_octaves = Vec::new();
    for range in &analysis.analyzed {
        let start = work.start();
        let onset = spectrum.measure(&mut pcm, range.clone(), work, cancelled, &mut read)?;
        work.finish(Stage::Onset, start);
        let result = track::estimate(&onset, schedule.loop_candidate(), mode, work, cancelled)?;
        let start = work.start();
        let before = analysis.runs.len();
        let baseline_summary = matches!(mode, Mode::Legacy | Mode::Reuse | Mode::Selection);
        if baseline_summary && !result.runs.is_empty() {
            regions += 1;
        }
        ambiguous |= result.ambiguous;
        outside |= result.outside;
        variable |= result.variable;
        for run in result.runs {
            let with_frames = |bin: f64| {
                let frame = (bin * f64::from(rate / HZ)).round().max(0.0) as u64;
                range.start.saturating_add(frame).min(range.end)
            };
            let beats: Vec<_> = run
                .beats
                .into_iter()
                .filter_map(|(bin, inferred)| {
                    let frame = with_frames(bin);
                    (frame < range.end).then_some(Beat { frame, inferred })
                })
                .collect();
            if beats.len() < 3 {
                continue;
            }
            if analysis.runs.iter().map(|r| r.beats.len()).sum::<usize>() + beats.len() > MAX_BEATS
            {
                return Err(Error::ResourceLimit.into());
            }
            other_octaves.push(run.octave);
            analysis.runs.push(BeatRun {
                range: with_frames(run.range.start as f64)..with_frames(run.range.end as f64),
                bpm: run.bpm,
                octave_bpm: run.octave,
                beats,
            });
        }
        if !baseline_summary && analysis.runs.len() > before {
            regions += 1;
        }
        work.finish(Stage::Publish, start);
    }
    check_cancelled(cancelled)?;
    let start = work.start();
    analysis.summary = summarize(
        &analysis.runs,
        &other_octaves,
        regions,
        analysis.sampled,
        ambiguous,
        outside,
    );
    if variable && matches!(analysis.summary, Summary::Stable { .. }) {
        analysis.summary = Summary::Variable;
    }
    work.finish(Stage::Publish, start);
    Ok(analysis)
}

fn summarize(
    runs: &[BeatRun],
    alternatives: &[Option<f64>],
    regions: usize,
    sampled: bool,
    ambiguous: bool,
    outside: bool,
) -> Summary {
    if ambiguous {
        return Summary::Ambiguous;
    }
    if outside {
        return Summary::OutOfRange;
    }
    let mut periods: Vec<_> = runs.iter().map(|run| 60.0 / run.bpm).collect();
    periods.sort_unstable_by(f64::total_cmp);
    let Some((&low, &high)) = periods.first().zip(periods.last()) else {
        return Summary::Weak;
    };
    if high > low * 1.03 {
        return Summary::Variable;
    }
    if sampled && regions < 2 {
        return Summary::Weak;
    }
    let n = periods.len();
    let bpm = 120.0 / (periods[(n - 1) / 2] + periods[n / 2]);
    let reference = Tempo::PREFERRED_OCTAVE_BPM;
    let mut preferred = bpm;
    for alternative in alternatives.iter().flatten().copied() {
        if alternatives
            .iter()
            .all(|candidate| candidate.is_some_and(|rate| (rate / alternative - 1.0).abs() <= 0.03))
            && ((alternative - reference).abs(), alternative)
                < ((preferred - reference).abs(), preferred)
        {
            preferred = alternative;
        }
    }
    Summary::Stable {
        bpm,
        nearest_100_bpm: preferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clicks_mode(rate: u32, tempo: f64, frames: u64, mode: Mode) -> Result<Analysis, Error> {
        let period = 60.0 * f64::from(rate) / tempo;
        let start = (0.23 * f64::from(rate)).round();
        analyze_mode(rate, frames, mode, &|| false, |offset, out| {
            for (i, frame) in out.iter_mut().enumerate() {
                let elapsed = (offset + i as u64) as f64 - start;
                let t = elapsed.rem_euclid(period) / f64::from(rate);
                let sound = if elapsed >= 0.0 && t < 0.018 {
                    (std::f64::consts::TAU * 997.0 * t).sin() * (-340.0 * t).exp() * 0.25
                } else {
                    0.0
                } as f32;
                *frame = [sound, -sound];
            }
            Ok(())
        })
    }

    #[test]
    fn reuse_preserves_full_and_sampled_results() {
        for seconds in [6, 40] {
            let frames = seconds * 48_000;
            assert_eq!(
                clicks_mode(48_000, 113.0, frames, Mode::Legacy).unwrap(),
                clicks_mode(48_000, 113.0, frames, Mode::Reuse).unwrap()
            );
        }
    }

    #[test]
    fn timestamps_match_antiphase_pulses_at_all_prepared_rates() {
        for rate in crate::SAMPLE_RATES {
            let analysis = clicks_mode(rate, 120.0, u64::from(rate) * 4, Mode::default()).unwrap();
            assert!(
                matches!(analysis.summary, Summary::Stable {bpm, ..} if (bpm - 120.0).abs() < 1.0)
            );
            let beats: Vec<_> = analysis.runs.iter().flat_map(|run| &run.beats).collect();
            assert_eq!(beats.len(), 8, "{rate}: {analysis:?}");
            for (i, beat) in beats.iter().enumerate() {
                let reference = (0.23 + i as f64 * 0.5) * f64::from(rate);
                assert!(
                    !beat.inferred
                        && (beat.frame as f64 - reference).abs() <= f64::from(rate) * 0.023
                );
            }
        }
    }

    #[test]
    fn sampling_preserves_source_clock_without_repeating_overlapping_reads() {
        let rate = 48_000;
        let frames = 36 * u64::from(rate) + 1;
        let mut frontier = 0;
        let analysis = analyze::<Error>(rate, frames, &|| false, |offset, out| {
            assert!(offset >= frontier && offset + out.len() as u64 <= frames);
            frontier = offset + out.len() as u64;
            out.fill([0.0; 2]);
            Ok(())
        })
        .unwrap();
        assert!(analysis.sampled);
        assert_eq!(analysis.analyzed.len(), 3);
        assert!(analysis.runs.is_empty());
        assert_eq!(analysis.summary, Summary::Weak);
        let cancelled = analyze::<Error>(rate, frames, &|| true, |_, _| unreachable!());
        assert_eq!(cancelled, Err(Error::Cancelled));
    }

    #[test]
    fn combined_profile_preserves_read_geometry_and_limits() {
        let rate = 192_000;
        let frames = 36 * u64::from(rate);
        let run = |mode, profile: &mut Profile| {
            analyze_profile::<Error>(
                rate,
                frames,
                mode,
                &|| false,
                |_, out| {
                    out.fill([0.0; 2]);
                    Ok(())
                },
                profile,
            )
            .unwrap()
        };
        let mut legacy = Profile::default();
        let mut combined = Profile::default();
        assert_eq!(
            run(Mode::Legacy, &mut legacy),
            run(Mode::Combined, &mut combined)
        );
        assert_eq!(legacy.read_calls, combined.read_calls);
        assert_eq!(combined.read_frames as u64, frames);
        assert_eq!(legacy.transforms, combined.transforms);
        assert!(combined.transforms <= 7400);
        assert!(combined.visits <= 256_000_000);

        let checks = std::cell::Cell::new(0);
        assert_eq!(
            analyze_mode::<Error>(
                rate,
                frames,
                Mode::Combined,
                &|| {
                    checks.set(checks.get() + 1);
                    checks.get() > 40
                },
                |_, out| {
                    out.fill([0.0; 2]);
                    Ok(())
                }
            ),
            Err(Error::Cancelled),
        );
    }
}
