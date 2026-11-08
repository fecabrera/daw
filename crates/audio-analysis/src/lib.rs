//! Portable offline audio analysis over interleaved f32 PCM.
//! Detection allocates; call it on a worker, never an audio callback.
#![forbid(unsafe_code)]
mod spectral_attacks;
mod transients;

use std::{fmt, ops::ControlFlow};
pub use transients::TransientDetector;

/// Initially supported processing rates. No project or device is required.
pub const SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRate,
    InvalidLayout,
    NonfiniteInput,
    ResourceLimit,
    Cancelled,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRate => "unsupported analysis sample rate",
            Self::InvalidLayout => "expected complete interleaved mono or stereo frames",
            Self::NonfiniteInput => "analysis input contains NaN or infinity",
            Self::ResourceLimit => "audio analysis exceeds arithmetic or memory limits",
            Self::Cancelled => "audio analysis cancelled",
        })
    }
}
impl std::error::Error for Error {}

/// Detection settings, independent of stretch ratio and rendered output size.
#[derive(Clone, Copy, Debug)]
pub struct TransientRequest {
    pub sample_rate: u32,
    pub channels: usize,
    pub detector: TransientDetector,
    /// Conservative candidate/FFT storage budget; excludes input, allocator and
    /// FFT planner internals. The returned vector is included in this budget.
    pub memory_limit_bytes: usize,
}
impl TransientRequest {
    /// Combined detection is the default; use Energy for baseline comparisons.
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            sample_rate,
            channels,
            detector: TransientDetector::Combined,
            memory_limit_bytes: 256 * 1024 * 1024,
        }
    }
    /// Conservative analysis storage, without any stretch output allocation.
    pub fn estimated_memory_bytes(self, frames: usize) -> Result<usize, Error> {
        if !SAMPLE_RATES.contains(&self.sample_rate) {
            return Err(Error::InvalidRate);
        }
        if !(1..=2).contains(&self.channels) {
            return Err(Error::InvalidLayout);
        }
        if frames > i64::MAX as usize {
            return Err(Error::ResourceLimit);
        }
        // Combined reserves two candidate lists plus the original spectral list.
        // Keep the same conservative estimate for Energy comparisons.
        (frames / (self.sample_rate as usize * 30 / 1000) + 1)
            .checked_mul(3 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(8 * 1024 * 1024))
            .ok_or(Error::ResourceLimit)
    }
    /// Return sorted source-frame positions, linked across stereo channels.
    pub fn detect(self, input: &[f32]) -> Result<Vec<usize>, Error> {
        self.detect_with_progress(input, |_| ControlFlow::Continue(()))
    }
    /// Progress is monotonic in 0..=1. Cancellation returns no partial result.
    ///
    /// ```
    /// use audio_analysis::TransientRequest;
    /// let onsets = TransientRequest::new(48000, 1).detect(&[0.0; 480])?;
    /// assert!(onsets.is_empty());
    /// # Ok::<(), audio_analysis::Error>(())
    /// ```
    pub fn detect_with_progress(
        self,
        input: &[f32],
        mut progress: impl FnMut(f32) -> ControlFlow<()>,
    ) -> Result<Vec<usize>, Error> {
        checkpoint(&mut progress, 0.0)?;
        // Validate rate/layout before division, including for empty inputs.
        self.estimated_memory_bytes(0)?;
        if !input.len().is_multiple_of(self.channels) {
            return Err(Error::InvalidLayout);
        }
        let frames = input.len() / self.channels;
        if self.estimated_memory_bytes(frames)? > self.memory_limit_bytes {
            return Err(Error::ResourceLimit);
        }
        for (i, chunk) in input.chunks(8192).enumerate() {
            if chunk.iter().any(|sample| !sample.is_finite()) {
                return Err(Error::NonfiniteInput);
            }
            checkpoint(
                &mut progress,
                0.1 * (i * 8192) as f32 / input.len().max(1) as f32,
            )?;
        }
        let onsets = transients::analyze(self, input, &mut progress)?;
        checkpoint(&mut progress, 1.0)?;
        Ok(onsets)
    }
}
fn checkpoint(progress: &mut impl FnMut(f32) -> ControlFlow<()>, value: f32) -> Result<(), Error> {
    match progress(value) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(Error::Cancelled),
    }
}
fn buffer<T: Clone>(size: usize, value: T) -> Result<Vec<T>, Error> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(size)
        .map_err(|_| Error::ResourceLimit)?;
    buffer.resize(size, value);
    Ok(buffer)
}
