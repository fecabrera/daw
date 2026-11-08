//! Stretch-specific attack timing and compatibility adapter.
use audio_analysis::TransientDetector;

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
    let analysis = audio_analysis::TransientRequest {
        detector: mode,
        memory_limit_bytes: request.memory_limit_bytes,
        ..audio_analysis::TransientRequest::new(request.sample_rate, request.channels)
    };
    analysis
        .detect_with_progress(input, |p| progress(0.10 + 0.05 * p))
        .map_err(|error| match error {
            audio_analysis::Error::InvalidRate => crate::Error::InvalidRate,
            audio_analysis::Error::InvalidLayout => crate::Error::InvalidLayout,
            audio_analysis::Error::NonfiniteInput => crate::Error::NonfiniteInput,
            audio_analysis::Error::ResourceLimit => crate::Error::ResourceLimit,
            audio_analysis::Error::Cancelled => crate::Error::Cancelled,
        })
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
