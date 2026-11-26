//! Measurements over borrowed PCM and precomputed power bins.

use std::{fmt, ops::Range};

/// Sum scalar sample power in input order using double precision.
#[inline]
pub fn sum_squared(samples: &[f32]) -> f64 {
    samples
        .iter()
        .map(|&sample| f64::from(sample).powi(2))
        .sum()
}

/// Geometric-to-arithmetic power ratio using the caller's mean.
/// Returns zero for empty bins, silence or invalid input.
pub fn spectral_flatness(power: &[f64], mean: f64) -> f64 {
    if power.is_empty() || !mean.is_finite() || mean <= 0.0 {
        return 0.0;
    }
    let Some(sum) = power.iter().try_fold(0.0, |sum, &bin| {
        (bin.is_finite() && bin >= 0.0).then(|| sum + (bin / mean).max(1e-30).ln())
    }) else {
        return 0.0;
    };
    (sum / power.len() as f64).exp()
}

pub struct Diagnostics {
    pub peak: f64,
    pub rms: f64,
    pub nonfinite_samples: usize,
}

/// Report scalar diagnostics, preserving the original IEEE arithmetic on nonfinite input.
pub fn diagnostics(samples: &[f32]) -> Diagnostics {
    Diagnostics {
        peak: samples
            .iter()
            .map(|&s| f64::from(s).abs())
            .fold(0.0, f64::max),
        rms: (sum_squared(samples) / samples.len().max(1) as f64).sqrt(),
        nonfinite_samples: samples.iter().filter(|s| !s.is_finite()).count(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementError {
    InvalidLayout,
    InvalidRange,
    InsufficientCycles,
    RelationshipRequiresStereo,
    PhaseRequiresStereo,
    InvalidFrequency,
    InvalidEventRange,
    MissingEvent,
    MissingOnset,
    MissingEnergy,
    ResourceLimit,
}

impl fmt::Display for MeasurementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLayout => "invalid measurement PCM layout or sample rate",
            Self::InvalidRange => "measurement frame range or channel is outside audio",
            Self::InsufficientCycles => "tone metric needs more complete cycles",
            Self::RelationshipRequiresStereo => "channel relationship requires stereo",
            Self::PhaseRequiresStereo => "phase metric requires stereo",
            Self::InvalidFrequency => "phase metric requires a positive finite frequency",
            Self::InvalidEventRange => "event region is outside audio or too short to measure",
            Self::MissingEvent => "expected event is missing",
            Self::MissingOnset => "missing event onset",
            Self::MissingEnergy => "missing event energy",
            Self::ResourceLimit => "event measurement scratch allocation failed",
        })
    }
}
impl std::error::Error for MeasurementError {}

/// Caller-owned interleaved PCM. All frame ranges use absolute indices into this view.
#[derive(Clone, Copy)]
pub struct Pcm<'a> {
    samples: &'a [f32],
    channels: usize,
    rate: u32,
}

pub struct Tone {
    pub frequency_hz: f64,
    pub rms: f64,
}

pub struct Event {
    pub onset_frame: usize,
    pub width_frames: usize,
}

impl<'a> Pcm<'a> {
    pub fn new(samples: &'a [f32], channels: usize, rate: u32) -> Result<Self, MeasurementError> {
        if rate == 0 || channels == 0 || !samples.len().is_multiple_of(channels) {
            return Err(MeasurementError::InvalidLayout);
        }
        Ok(Self {
            samples,
            channels,
            rate,
        })
    }

    pub fn frames(self) -> usize {
        self.samples.len() / self.channels
    }

    pub fn tone(
        self,
        frames: Range<usize>,
        channel: usize,
        min_cycles: usize,
    ) -> Result<Tone, MeasurementError> {
        if frames.start > frames.end || frames.end > self.frames() || channel >= self.channels {
            return Err(MeasurementError::InvalidRange);
        }
        let mut crossings = 0;
        let mut first = 0.0;
        let mut last = 0.0;
        let mut energy = 0.0;
        for i in frames.start..frames.end.saturating_sub(1) {
            let a = f64::from(self.samples[i * self.channels + channel]);
            let b = f64::from(self.samples[(i + 1) * self.channels + channel]);
            energy += a * a;
            if a <= 0.0 && b > 0.0 {
                last = i as f64 - a / (b - a);
                if crossings == 0 {
                    first = last;
                }
                crossings += 1;
            }
        }
        if crossings <= min_cycles.max(1) {
            return Err(MeasurementError::InsufficientCycles);
        }
        Ok(Tone {
            frequency_hz: (crossings - 1) as f64 * f64::from(self.rate) / (last - first),
            rms: (energy / (frames.len() - 1) as f64).sqrt(),
        })
    }

    /// Return residual power and left-channel reference power.
    pub fn channel_residual(self, factor: f64) -> Result<(f64, f64), MeasurementError> {
        if self.channels != 2 {
            return Err(MeasurementError::RelationshipRequiresStereo);
        }
        let mut residual = 0.0;
        let mut reference = 0.0;
        for frame in self.samples.chunks_exact(2) {
            residual += (f64::from(frame[1]) - factor * f64::from(frame[0])).powi(2);
            reference += f64::from(frame[0]).powi(2);
        }
        Ok((residual, reference))
    }

    /// Measure stereo phases in radians at a known frequency.
    pub fn stereo_phase(
        self,
        frames: Range<usize>,
        frequency_hz: f64,
    ) -> Result<[f64; 2], MeasurementError> {
        if self.channels != 2 {
            return Err(MeasurementError::PhaseRequiresStereo);
        }
        if frames.is_empty() || frames.end > self.frames() {
            return Err(MeasurementError::InvalidRange);
        }
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            return Err(MeasurementError::InvalidFrequency);
        }
        let mut phases = [0.0; 2];
        for (channel, phase) in phases.iter_mut().enumerate() {
            let (mut sine, mut cosine) = (0.0, 0.0);
            for i in frames.clone() {
                let angle = i as f64 * frequency_hz * std::f64::consts::TAU / f64::from(self.rate);
                let value = f64::from(self.samples[2 * i + channel]);
                sine += value * angle.sin();
                cosine += value * angle.cos();
            }
            *phase = cosine.atan2(sine);
        }
        Ok(phases)
    }

    /// Measure a forward mean-square onset and cumulative energy width.
    /// Call off the audio thread. Scratch allocation is bounded by the region length.
    pub fn event(
        self,
        frames: Range<usize>,
        window: usize,
        onset_fraction: f64,
        width_fractions: Range<f64>,
        minimum_peak: f64,
    ) -> Result<Event, MeasurementError> {
        if frames.is_empty()
            || frames.end > self.frames()
            || window == 0
            || frames.len() < window
            || !(0.0..=1.0).contains(&onset_fraction)
            || !(0.0..=1.0).contains(&width_fractions.start)
            || !(width_fractions.start..=1.0).contains(&width_fractions.end)
            || !minimum_peak.is_finite()
            || minimum_peak < 0.0
        {
            return Err(MeasurementError::InvalidEventRange);
        }
        let mut energy = Vec::new();
        energy
            .try_reserve_exact(frames.len())
            .map_err(|_| MeasurementError::ResourceLimit)?;
        energy.extend(
            self.samples[frames.start * self.channels..frames.end * self.channels]
                .chunks_exact(self.channels)
                .map(sum_squared),
        );
        let mut envelope = Vec::new();
        envelope
            .try_reserve_exact(energy.len() - window + 1)
            .map_err(|_| MeasurementError::ResourceLimit)?;
        envelope.extend(
            energy
                .windows(window)
                .map(|s| s.iter().sum::<f64>() / window as f64),
        );
        let peak = envelope.iter().copied().fold(0.0, f64::max);
        if peak <= minimum_peak {
            return Err(MeasurementError::MissingEvent);
        }
        let onset_frame = frames.start
            + envelope
                .iter()
                .position(|&e| e >= peak * onset_fraction)
                .ok_or(MeasurementError::MissingOnset)?;
        let total: f64 = energy.iter().sum();
        let (mut sum, mut low, mut high) = (0.0, None, None);
        for (i, e) in energy.iter().enumerate() {
            sum += e;
            if low.is_none() && sum >= total * width_fractions.start {
                low = Some(i);
            }
            if high.is_none() && sum >= total * width_fractions.end {
                high = Some(i);
            }
        }
        Ok(Event {
            onset_frame,
            width_frames: high.ok_or(MeasurementError::MissingEnergy)?
                - low.ok_or(MeasurementError::MissingEnergy)?,
        })
    }
}
