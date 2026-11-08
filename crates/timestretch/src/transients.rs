//! Linked attack detection from short-time energy, independent of signal polarity.
//! Runs in source order and keeps constant storage, including for long sources.
pub(crate) struct Attacks {
    next: usize,
    block: usize,
    history: [f64; 80],
    index: usize,
    observations: usize,
    refractory: usize,
    last: Option<usize>,
}
impl Attacks {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            next: 0,
            block: (rate as usize / 1000).max(1),
            history: [0.; 80],
            index: 0,
            observations: 0,
            refractory: rate as usize * 30 / 1000,
            last: None,
        }
    }
    /// Consume complete 1 ms energy observations up to this source coordinate.
    /// Both channels contribute power, so opposite polarity cannot hide an attack.
    pub(crate) fn advance(&mut self, input: &[f32], channels: usize, source: i64) -> bool {
        let frames = input.len() / channels;
        let mut attack = false;
        while self.next < frames && self.next as i64 <= source {
            let end = (self.next + self.block).min(frames);
            let energy = input[self.next * channels..end * channels]
                .iter()
                .map(|s| f64::from(*s).powi(2))
                .sum::<f64>()
                / ((end - self.next) * channels) as f64;
            let background = self.history.iter().sum::<f64>() / self.observations.max(1) as f64;
            // An 80 ms background spans several ordinary bass cycles and a close
            // dyad's beat period. An 8 ms background mistakes their energy minima
            // for silence. Keep 1 ms observations and the recent-rise check for
            // attack localization; relative thresholds retain gain/polarity invariance.
            let recent = self.history[(self.index + self.history.len() - 1) % self.history.len()]
                .max(self.history[(self.index + self.history.len() - 2) % self.history.len()]);
            if energy > background * 6.0
                && energy > recent * 2.0
                && self
                    .last
                    .is_none_or(|last| self.next - last >= self.refractory)
            {
                self.last = Some(self.next);
                attack = true;
            }
            self.observations = (self.observations + 1).min(self.history.len());
            self.history[self.index] = energy;
            self.index = (self.index + 1) % self.history.len();
            self.next = end;
        }
        attack
    }
}

/// Detector identity must accompany explicit-mode renders and caches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransientDetector {
    Energy,
    Combined,
}
impl TransientDetector {
    pub fn revision(self) -> &'static str {
        match self {
            Self::Energy => "energy-r4",
            Self::Combined => "energy-spectral-v5",
        }
    }
}

/// Analyze interleaved PCM on the calling worker, with cancellation and request
/// memory limits. Positions are sorted source-frame indices, linked across stereo.
pub fn detect_transients(
    request: crate::Request,
    input: &[f32],
    mode: TransientDetector,
    mut progress: impl FnMut(f32) -> std::ops::ControlFlow<()>,
) -> Result<Vec<usize>, crate::Error> {
    crate::checkpoint(&mut progress, 0.0)?;
    let (frames, output) = request.dimensions(input)?;
    let memory = request.storage(frames, output)?;
    if memory > request.memory_limit_bytes {
        return Err(crate::Error::ResourceLimit);
    }
    request.validate_samples(input, &mut progress)?;
    let result = analyze(request, input, mode, &mut progress)?;
    crate::checkpoint(&mut progress, 1.0)?;
    Ok(result)
}
fn analyze(
    request: crate::Request,
    input: &[f32],
    mode: TransientDetector,
    progress: &mut impl FnMut(f32) -> std::ops::ControlFlow<()>,
) -> Result<Vec<usize>, crate::Error> {
    let frames = input.len() / request.channels;
    let mut energy = Attacks::new(request.sample_rate);
    let count = frames / energy.refractory + 1;
    let mut onsets = Vec::new();
    onsets
        .try_reserve_exact(
            count
                .checked_mul(if mode == TransientDetector::Combined {
                    2
                } else {
                    1
                })
                .ok_or(crate::Error::ResourceLimit)?,
        )
        .map_err(|_| crate::Error::ResourceLimit)?;
    let mut spectral_onsets = Vec::new();
    if mode == TransientDetector::Combined {
        spectral_onsets
            .try_reserve_exact(count)
            .map_err(|_| crate::Error::ResourceLimit)?;
    }
    let mut spectral = if mode == TransientDetector::Combined && frames > 0 {
        Some(crate::spectral_attacks::Detector::new(request.sample_rate)?)
    } else {
        None
    };
    for source in (0..frames).step_by(energy.block) {
        crate::checkpoint(progress, 0.10 + 0.05 * source as f32 / frames as f32)?;
        if energy.advance(input, request.channels, source as i64) {
            onsets.push(source);
        }
        if let Some(detector) = &mut spectral
            && source.is_multiple_of(energy.block * 4)
            && let Some(at) = detector.observe(input, request.channels, source)
        {
            spectral_onsets.push(at);
        }
    }
    // Preserve baseline timing around existing attacks. Additional spectral
    // evidence may fill gaps, but must not move a known energy attack earlier.
    let energy_count = onsets.len();
    for at in spectral_onsets {
        let i = onsets[..energy_count].partition_point(|&event| event < at);
        let near = onsets[..energy_count]
            .get(i)
            .is_some_and(|event| at.abs_diff(*event) < energy.refractory)
            || i.checked_sub(1)
                .and_then(|i| onsets.get(i))
                .is_some_and(|event| at.abs_diff(*event) < energy.refractory);
        if !near {
            onsets.push(at);
        }
    }
    onsets.sort_unstable();
    let mut used = 0;
    for i in 0..onsets.len() {
        if used == 0 || onsets[i] - onsets[used - 1] >= energy.refractory {
            onsets[used] = onsets[i];
            used += 1;
        }
    }
    onsets.truncate(used);
    Ok(onsets)
}

#[derive(Clone, Copy)]
struct Region {
    source_start: f64,
    output_start: f64,
    output_end: f64,
}

/// Preserve the original speed around an attack; spend the requested extra time
/// between attacks. Each onset remains anchored at ratio * source_onset.
pub(crate) struct Timing {
    regions: Vec<Region>,
    cursor: usize,
    ratio: f64,
    frames_in: f64,
    frames_out: f64,
}
impl Timing {
    pub(crate) fn analyze(
        request: crate::Request,
        input: &[f32],
        width: usize,
        frames_out: usize,
        detector: TransientDetector,
        progress: &mut impl FnMut(f32) -> std::ops::ControlFlow<()>,
    ) -> Result<Self, crate::Error> {
        let frames = input.len() / request.channels;
        let ratio = request.ratio.as_f64();
        let mut timing = Self {
            regions: Vec::new(),
            cursor: 0,
            ratio,
            frames_in: frames as f64,
            frames_out: frames_out as f64,
        };
        if ratio <= 1.0 {
            return Ok(timing);
        }
        let onsets = analyze(request, input, detector, progress)?;
        timing
            .regions
            .try_reserve_exact(onsets.len())
            .map_err(|_| crate::Error::ResourceLimit)?;
        for (i, &onset) in onsets.iter().enumerate() {
            // Clip guards to one third of each neighboring event gap. Dense attacks cannot
            // demand overlapping incompatible source maps. The map stays monotonic.
            let before = (width / 2 + request.sample_rate as usize / 500).min(if i == 0 {
                onset
            } else {
                (onset - onsets[i - 1]) / 3
            });
            let after = (width / 2 + (request.sample_rate as usize / 100).min(width / 4)).min(
                if i + 1 == onsets.len() {
                    frames - onset
                } else {
                    (onsets[i + 1] - onset) / 3
                },
            );
            timing.regions.push(Region {
                source_start: (onset - before) as f64,
                output_start: onset as f64 * ratio - before as f64,
                output_end: onset as f64 * ratio + after as f64,
            });
        }
        Ok(timing)
    }
    /// Monotonic queries from the synthesis loop. The flag restores source phase
    /// inside a protected attack so overlapping frames reconstruct that waveform.
    pub(crate) fn at(&mut self, output: i64) -> (i64, bool) {
        let out = output as f64;
        if self.regions.is_empty() || out < 0. || out >= self.frames_out {
            return ((out / self.ratio).round() as i64, false);
        }
        while self.cursor < self.regions.len() && out > self.regions[self.cursor].output_end {
            self.cursor += 1;
        }
        let (source_lo, output_lo) = if self.cursor == 0 {
            (0., 0.)
        } else {
            let previous = self.regions[self.cursor - 1];
            (
                previous.source_start + previous.output_end - previous.output_start,
                previous.output_end,
            )
        };
        let (source_hi, output_hi) = if let Some(region) = self.regions.get(self.cursor) {
            if out >= region.output_start {
                return (
                    (region.source_start + out - region.output_start).round() as i64,
                    true,
                );
            }
            (region.source_start, region.output_start)
        } else {
            (self.frames_in, self.frames_out)
        };
        let position = if output_hi > output_lo {
            source_lo + (out - output_lo) * (source_hi - source_lo) / (output_hi - output_lo)
        } else {
            source_hi
        };
        (position.round() as i64, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn sustained_bass_beats_do_not_trigger_attacks() {
        for rate in crate::SAMPLE_RATES {
            for harmonics in [false, true] {
                let mono: Vec<_> = (0..rate as usize * 2)
                    .map(|frame| {
                        let time = frame as f64 / f64::from(rate);
                        [51.9130872, 65.40639133]
                            .into_iter()
                            .map(|hz| {
                                let phase = TAU * hz * time;
                                0.2 * phase.sin()
                                    + if harmonics {
                                        0.09 * (phase * 2.0 + 0.2).sin()
                                            + 0.04 * (phase * 3.0 + 0.4).sin()
                                    } else {
                                        0.0
                                    }
                            })
                            .sum::<f64>() as f32
                    })
                    .collect();
                for scale in [0.01, 1.0, 2.0] {
                    let input: Vec<_> = mono.iter().flat_map(|s| [s * scale, -s * scale]).collect();
                    let mut detector = Attacks::new(rate);
                    let mut onsets = vec![];
                    for at in (0..mono.len()).step_by(detector.block) {
                        if detector.advance(&input, 2, at as i64) {
                            onsets.push(at);
                        }
                    }
                    assert_eq!(onsets, [0], "{rate} {harmonics} {scale}: {onsets:?}");
                }
            }
        }
    }

    #[test]
    fn longer_background_retains_dense_attacks_and_polarity() {
        for rate in crate::SAMPLE_RATES {
            let mut detector = Attacks::new(rate);
            let events = [
                0,
                detector.block * 35,
                detector.block * 71,
                detector.block * 106,
            ];
            let mut input = vec![0.0; rate as usize / 5 * 2];
            for at in events {
                input[at * 2] = 0.5;
                input[at * 2 + 1] = -0.5;
            }
            let mut found = vec![];
            for at in (0..input.len() / 2).step_by(detector.block) {
                if detector.advance(&input, 2, at as i64) {
                    found.push(at);
                }
            }
            assert_eq!(found, events);
        }
    }
}
