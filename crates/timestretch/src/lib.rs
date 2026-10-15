//! Independent offline time stretching. Input and output are interleaved f32 frames.
//! Call on a worker: processing allocates and blocks. No files, devices, or threads.
#![forbid(unsafe_code)]
mod spectral;
use std::{fmt, ops::ControlFlow, str::FromStr};

pub const REVISION: u32 = 1;
pub const SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRatio,
    InvalidRate,
    InvalidLayout,
    NonfiniteInput,
    OutputTooShort,
    ResourceLimit,
    NumericalFailure,
    Cancelled,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRatio => "duration ratio must be between 1/8 and 8",
            Self::InvalidRate => "unsupported sample rate",
            Self::InvalidLayout => "expected complete interleaved mono or stereo frames",
            Self::NonfiniteInput => "input contains NaN or infinity",
            Self::OutputTooShort => "nonempty input rounds to zero output frames",
            Self::ResourceLimit => "time stretch exceeds arithmetic or memory limits",
            Self::NumericalFailure => "time stretch produced an invalid numerical result",
            Self::Cancelled => "time stretch cancelled",
        })
    }
}
impl std::error::Error for Error {}

/// Output duration / input duration, reduced to positive integer components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ratio {
    numerator: u64,
    denominator: u64,
}
impl Ratio {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, Error> {
        if numerator == 0
            || denominator == 0
            || u128::from(numerator) * 8 < u128::from(denominator)
            || u128::from(numerator) > u128::from(denominator) * 8
        {
            return Err(Error::InvalidRatio);
        }
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            numerator: numerator / a,
            denominator: denominator / a,
        })
    }
    pub fn numerator(self) -> u64 {
        self.numerator
    }
    pub fn denominator(self) -> u64 {
        self.denominator
    }
    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
    pub fn output_frames(self, frames: usize) -> Result<usize, Error> {
        let result = (frames as u128 * u128::from(self.numerator)
            + u128::from(self.denominator / 2))
            / u128::from(self.denominator);
        let result = usize::try_from(result).map_err(|_| Error::ResourceLimit)?;
        if frames != 0 && result == 0 {
            return Err(Error::OutputTooShort);
        }
        Ok(result)
    }
}
impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}
impl FromStr for Ratio {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        let integer = |s: &str| -> Result<u64, Error> {
            if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
                return Err(Error::InvalidRatio);
            }
            s.parse().map_err(|_| Error::InvalidRatio)
        };
        if let Some((p, q)) = text.split_once('/') {
            return Self::new(integer(p)?, integer(q)?);
        }
        if let Some((whole, fraction)) = text.split_once('.') {
            let q = 10u64
                .checked_pow(fraction.len().try_into().map_err(|_| Error::InvalidRatio)?)
                .ok_or(Error::InvalidRatio)?;
            let p = integer(whole)?
                .checked_mul(q)
                .and_then(|n| n.checked_add(integer(fraction).ok()?))
                .ok_or(Error::InvalidRatio)?;
            return Self::new(p, q);
        }
        Self::new(integer(text)?, 1)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub sample_rate: u32,
    pub channels: usize,
    pub ratio: Ratio,
    /// Upper bound for final output plus engine working storage. Default: 1 GiB.
    /// FFT planner internals and allocator overhead are not included.
    pub memory_limit_bytes: usize,
}
impl Request {
    pub fn new(sample_rate: u32, channels: usize, ratio: Ratio) -> Self {
        Self {
            sample_rate,
            channels,
            ratio,
            memory_limit_bytes: 1024 * 1024 * 1024,
        }
    }
    pub fn process(self, input: &[f32]) -> Result<Vec<f32>, Error> {
        self.process_with_progress(input, |_| ControlFlow::Continue(()))
    }
    /// Returning Break cancels without publishing partial output. Progress is 0..=1.
    pub fn process_with_progress(
        self,
        input: &[f32],
        mut progress: impl FnMut(f32) -> ControlFlow<()>,
    ) -> Result<Vec<f32>, Error> {
        checkpoint(&mut progress, 0.0)?;
        if !SAMPLE_RATES.contains(&self.sample_rate) {
            return Err(Error::InvalidRate);
        }
        if !(1..=2).contains(&self.channels) || !input.len().is_multiple_of(self.channels) {
            return Err(Error::InvalidLayout);
        }
        let frames = input.len() / self.channels;
        let output_frames = self.ratio.output_frames(frames)?;
        let samples = output_frames
            .checked_mul(self.channels)
            .ok_or(Error::ResourceLimit)?;
        // f64 overlap accumulation, final f32 output, and bounded FFT/hop workspace.
        let storage = samples
            .checked_mul(12)
            .and_then(|n| n.checked_add(8 * 1024 * 1024))
            .ok_or(Error::ResourceLimit)?;
        if storage > self.memory_limit_bytes {
            return Err(Error::ResourceLimit);
        }
        let mut constant = true;
        for (i, block) in input.chunks(8192).enumerate() {
            if block.iter().any(|s| !s.is_finite()) {
                return Err(Error::NonfiniteInput);
            }
            for (offset, sample) in block.iter().enumerate() {
                if sample.to_bits() != input[(i * 8192 + offset) % self.channels].to_bits() {
                    constant = false;
                }
            }
            checkpoint(
                &mut progress,
                0.05 * (i * 8192) as f32 / input.len().max(1) as f32,
            )?;
        }
        if frames == 0 || self.ratio.numerator == self.ratio.denominator {
            let mut output = buffer(input.len(), 0.0)?;
            for (i, (src, dst)) in input.chunks(8192).zip(output.chunks_mut(8192)).enumerate() {
                dst.copy_from_slice(src);
                checkpoint(
                    &mut progress,
                    0.05 + 0.94 * (i * 8192) as f32 / input.len().max(1) as f32,
                )?;
            }
            checkpoint(&mut progress, 1.0)?;
            return Ok(output);
        }
        if constant {
            // Exact DC is invariant under duration change; this is not resampling.
            let mut output = buffer(samples, 0.0)?;
            for (i, chunk) in output.chunks_mut(8192).enumerate() {
                checkpoint(
                    &mut progress,
                    0.05 + 0.94 * (i * 8192) as f32 / samples as f32,
                )?;
                for (offset, value) in chunk.iter_mut().enumerate() {
                    *value = input[(i * 8192 + offset) % self.channels];
                }
            }
            checkpoint(&mut progress, 1.0)?;
            return Ok(output);
        }
        spectral::process(self, input, output_frames, &mut progress)
    }
}
fn checkpoint(progress: &mut impl FnMut(f32) -> ControlFlow<()>, value: f32) -> Result<(), Error> {
    match progress(value) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(Error::Cancelled),
    }
}
fn buffer<T: Clone>(size: usize, value: T) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(size)
        .map_err(|_| Error::ResourceLimit)?;
    v.resize(size, value);
    Ok(v)
}
