//! Independent pitch by compensated duration change and band-limited resampling.
//! Mathematical design and filter choices are recorded in PROVENANCE.md.
use crate::{Error, Ratio, Request, buffer, checkpoint};
use std::{borrow::Cow, f64::consts::PI, ops::ControlFlow, str::FromStr};

/// Version of the optional pitch path. Zero pitch retains `REVISION` unchanged.
pub const PITCH_REVISION: u32 = 1;

/// Constant equal-tempered pitch offset, independent of output duration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pitch(f64);
impl Pitch {
    pub const UNISON: Self = Self(0.0);
    pub fn from_semitones(semitones: f64) -> Result<Self, Error> {
        if !semitones.is_finite() || !(-24.0..=24.0).contains(&semitones) {
            return Err(Error::InvalidPitch);
        }
        Ok(Self(if semitones == 0.0 { 0.0 } else { semitones }))
    }
    pub fn semitones(self) -> f64 {
        self.0
    }
    pub fn factor(self) -> f64 {
        (self.0 / 12.0).exp2()
    }
}
impl FromStr for Pitch {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        Self::from_semitones(text.parse().map_err(|_| Error::InvalidPitch)?)
    }
}

/// An existing stretch request plus a validated pitch offset.
/// Configure the shared memory limit on `stretch`; no new thread or dependency is used.
#[derive(Clone, Copy, Debug)]
pub struct PitchRequest {
    pub stretch: Request,
    pub pitch: Pitch,
}
impl PitchRequest {
    pub fn process(self, input: &[f32]) -> Result<Vec<f32>, Error> {
        self.process_with_progress(input, |_| ControlFlow::Continue(()))
    }
    /// Progress includes validation, intermediate stretching, and filtered resampling.
    pub fn process_with_progress(
        self,
        input: &[f32],
        mut progress: impl FnMut(f32) -> ControlFlow<()>,
    ) -> Result<Vec<f32>, Error> {
        let request = self.stretch;
        if self.pitch == Pitch::UNISON {
            return request.process_with_progress(input, progress);
        }
        checkpoint(&mut progress, 0.0)?;
        let (frames, output_frames) = request.dimensions(input)?;
        if frames == 0 {
            return request.process_with_progress(input, progress);
        }
        // The final length is always the original integer-rational duration.
        // Rounding this private intermediate length cannot change that contract.
        let intermediate = (output_frames as f64 * self.pitch.factor())
            .round()
            .max(1.0);
        if intermediate >= i64::MAX as f64
            || intermediate >= usize::MAX as f64
            || output_frames >= i64::MAX as usize
        {
            return Err(Error::ResourceLimit);
        }
        let intermediate = intermediate as usize;
        let step = intermediate as f64 / output_frames as f64;
        let filter = Filter::layout(step)?;
        let resampling_storage = intermediate
            .checked_add(output_frames)
            .and_then(|n| n.checked_mul(request.channels))
            .and_then(|n| n.checked_mul(size_of::<f32>()))
            .and_then(|n| n.checked_add(filter.storage_bytes))
            .ok_or(Error::ResourceLimit)?;
        if request
            .storage(frames, intermediate)?
            .max(resampling_storage)
            > request.memory_limit_bytes
        {
            return Err(Error::ResourceLimit);
        }
        if request.validate_samples(input, &mut progress)? {
            // DC (including digital silence) is invariant under either operation.
            return request.process_with_progress(input, |p| progress(0.05 + 0.95 * p));
        }
        let prepared = if intermediate == frames {
            Cow::Borrowed(input)
        } else {
            // Only this private staging ratio can extend beyond the public 1/8–8 range.
            let internal = Request {
                ratio: Ratio {
                    numerator: intermediate as u64,
                    denominator: frames as u64,
                },
                ..request
            };
            Cow::Owned(crate::spectral::process(
                internal,
                input,
                intermediate,
                &mut |p| progress(0.05 + 0.75 * p),
            )?)
        };
        checkpoint(&mut progress, 0.8)?;
        filter.resample(
            &prepared,
            request.channels,
            output_frames,
            step,
            &mut progress,
        )
    }
}

const PHASES: usize = 1024;
struct Filter {
    radius: usize,
    taps: usize,
    cutoff: f64,
    storage_bytes: usize,
}
impl Filter {
    fn layout(step: f64) -> Result<Self, Error> {
        let cutoff = 0.94 / step.max(1.0);
        let radius = (32.0 / cutoff).ceil() as usize;
        let taps = radius
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::ResourceLimit)?;
        let storage_bytes = taps
            .checked_mul(PHASES + 1)
            .and_then(|n| n.checked_mul(size_of::<f64>()))
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            radius,
            taps,
            cutoff,
            storage_bytes,
        })
    }
    fn resample(
        self,
        input: &[f32],
        channels: usize,
        frames: usize,
        step: f64,
        progress: &mut impl FnMut(f32) -> ControlFlow<()>,
    ) -> Result<Vec<f32>, Error> {
        let mut kernels = buffer(self.storage_bytes / size_of::<f64>(), 0.0)?;
        for phase in 0..=PHASES {
            if phase % 16 == 0 {
                checkpoint(progress, 0.8 + 0.05 * phase as f32 / PHASES as f32)?;
            }
            let kernel = &mut kernels[phase * self.taps..(phase + 1) * self.taps];
            for (tap, value) in kernel.iter_mut().enumerate() {
                let distance = tap as f64 - self.radius as f64 - phase as f64 / PHASES as f64;
                if distance.abs() <= self.radius as f64 {
                    let x = PI * self.cutoff * distance;
                    let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
                    let angle = PI * distance / self.radius as f64;
                    let window = 0.42 + 0.5 * angle.cos() + 0.08 * (2.0 * angle).cos();
                    *value = self.cutoff * sinc * window;
                }
            }
            let sum: f64 = kernel.iter().sum();
            for value in kernel {
                *value /= sum;
            }
        }
        let mut output = buffer(
            frames.checked_mul(channels).ok_or(Error::ResourceLimit)?,
            0.0,
        )?;
        let source_frames = input.len() / channels;
        for frame in 0..frames {
            if frame % 256 == 0 {
                checkpoint(progress, 0.85 + 0.15 * frame as f32 / frames as f32)?;
            }
            let position = frame as f64 * step;
            let center = position.floor() as i64;
            let fractional = (position - center as f64) * PHASES as f64;
            let phase = (fractional.floor() as usize).min(PHASES - 1);
            let blend = fractional - phase as f64;
            let mut values = [0.0f64; 2];
            for tap in 0..self.taps {
                let source = center + tap as i64 - self.radius as i64;
                if (0..source_frames as i64).contains(&source) {
                    let a = kernels[phase * self.taps + tap];
                    let b = kernels[(phase + 1) * self.taps + tap];
                    let weight = a + (b - a) * blend;
                    for channel in 0..channels {
                        values[channel] +=
                            f64::from(input[source as usize * channels + channel]) * weight;
                    }
                }
            }
            for channel in 0..channels {
                let value = values[channel] as f32;
                if !value.is_finite() {
                    return Err(Error::NumericalFailure);
                }
                output[frame * channels + channel] = value;
            }
        }
        checkpoint(progress, 1.0)?;
        Ok(output)
    }
}
