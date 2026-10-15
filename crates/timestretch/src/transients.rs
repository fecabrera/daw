//! Linked attack detection from short-time energy, independent of signal polarity.
//! Runs in source order and keeps constant storage, including for long sources.
pub(crate) struct Attacks {
    next: usize,
    block: usize,
    history: [f64; 8],
    index: usize,
    refractory: usize,
    last: Option<usize>,
}
impl Attacks {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            next: 0,
            block: (rate as usize / 1000).max(1),
            history: [0.; 8],
            index: 0,
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
            let background = self.history.iter().sum::<f64>() / self.history.len() as f64;
            // Relative thresholds preserve amplitude homogeneity. Two preceding
            // blocks must also be quieter; a sustained waveform's zero crossing
            // alone must not repeatedly trigger a low-frequency tone.
            let recent = self.history[(self.index + 7) % 8].max(self.history[(self.index + 6) % 8]);
            if energy > background * 6.0
                && energy > recent * 2.0
                && self
                    .last
                    .is_none_or(|last| self.next - last >= self.refractory)
            {
                self.last = Some(self.next);
                attack = true;
            }
            self.history[self.index] = energy;
            self.index = (self.index + 1) % self.history.len();
            self.next = end;
        }
        attack
    }
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
        let mut detector = Attacks::new(request.sample_rate);
        let count = frames / detector.refractory + 1;
        let mut onsets = Vec::new();
        onsets
            .try_reserve_exact(count)
            .map_err(|_| crate::Error::ResourceLimit)?;
        for source in (0..frames).step_by(detector.block) {
            crate::checkpoint(progress, 0.10 + 0.05 * source as f32 / frames as f32)?;
            if detector.advance(input, request.channels, source as i64) {
                onsets.push(source);
            }
        }
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
