//! Fixed atomic spectrum publication. Magnitudes are linked-stereo linear amplitudes.
use crate::MeterStorage;
use std::sync::atomic::{AtomicU32, Ordering};
pub const SPECTRUM_BINS: usize = 64;
#[derive(Clone, Copy, Debug)]
pub struct SpectrumReading {
    pub sample_rate_hz: u32,
    pub magnitudes: [f32; SPECTRUM_BINS],
}
impl Default for SpectrumReading {
    fn default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            magnitudes: [0.0; SPECTRUM_BINS],
        }
    }
}
/// Logarithmic bin centers, shared by analysis and painting.
pub fn spectrum_frequency(index: usize) -> f32 {
    20.0 * 1000.0_f32.powf(index as f32 / (SPECTRUM_BINS - 1) as f32)
}
pub struct SpectrumMeter {
    rate: AtomicU32,
    bins: [AtomicU32; SPECTRUM_BINS],
}
impl Default for SpectrumMeter {
    fn default() -> Self {
        Self {
            rate: AtomicU32::new(48_000),
            bins: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
}
impl SpectrumMeter {
    pub fn reading(&self) -> SpectrumReading {
        SpectrumReading {
            sample_rate_hz: self.rate.load(Ordering::Relaxed),
            magnitudes: std::array::from_fn(|i| {
                f32::from_bits(self.bins[i].load(Ordering::Relaxed))
            }),
        }
    }
}
impl MeterStorage for SpectrumMeter {
    type Reading = SpectrumReading;
    const ZERO: SpectrumReading = SpectrumReading {
        sample_rate_hz: 48_000,
        magnitudes: [0.0; SPECTRUM_BINS],
    };
    fn store(&self, v: SpectrumReading) {
        self.rate.store(v.sample_rate_hz, Ordering::Relaxed);
        for (cell, value) in self.bins.iter().zip(v.magnitudes) {
            cell.store(
                if value.is_finite() {
                    value.max(0.0).to_bits()
                } else {
                    0
                },
                Ordering::Relaxed,
            );
        }
    }
    fn clear(&self) {
        for cell in &self.bins {
            cell.store(0, Ordering::Relaxed);
        }
    }
}

/// Backend-neutral response of up to four serial biquad sections. Coefficients use ascending powers of z⁻¹.
#[derive(Clone, Copy, Debug)]
pub struct FrequencyResponse {
    pub numerator: [f64; 3],
    pub denominator: [f64; 3],
    pub sample_rate_hz: u32,
    additional_biquads: [Option<([f64; 3], [f64; 3])>; 3],
}
impl FrequencyResponse {
    pub fn from_biquad(numerator: [f64; 3], denominator: [f64; 3], rate: u32) -> Self {
        Self {
            numerator,
            denominator,
            sample_rate_hz: rate,
            additional_biquads: [None; 3],
        }
    }
    /// Append one section. A response supports at most four sections.
    pub fn with_biquad(mut self, numerator: [f64; 3], denominator: [f64; 3]) -> Self {
        let slot = self
            .additional_biquads
            .iter_mut()
            .find(|section| section.is_none())
            .expect("at most four response sections");
        *slot = Some((numerator, denominator));
        self
    }
    pub fn unity(rate: u32) -> Self {
        Self::from_biquad([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], rate)
    }
    pub fn db(self, frequency_hz: f32) -> f32 {
        let w =
            std::f64::consts::TAU * f64::from(frequency_hz) / f64::from(self.sample_rate_hz.max(1));
        let power = |c: [f64; 3]| {
            let re = c[0] + c[1] * w.cos() + c[2] * (2.0 * w).cos();
            let im = c[1] * w.sin() + c[2] * (2.0 * w).sin();
            re * re + im * im
        };
        let section_db = |n, d| 10.0 * (power(n).max(1e-24) / power(d).max(1e-24)).log10();
        let additional_db: f64 = self
            .additional_biquads
            .into_iter()
            .flatten()
            .map(|(n, d)| section_db(n, d))
            .sum();
        (section_db(self.numerator, self.denominator) + additional_db).clamp(-144.0, 144.0) as f32
    }
}
