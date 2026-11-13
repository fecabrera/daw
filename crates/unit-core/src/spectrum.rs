//! Coherent, bounded spectrum snapshots. One serialized producer owns publication.
use crate::MeterStorage;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
pub const SPECTRUM_BINS: usize = 512;
pub const SPECTRUM_UPPER_READY: u32 = 1;
pub const SPECTRUM_BASS_READY: u32 = 2;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumReading {
    pub sample_rate_hz: u32,
    pub magnitudes: [f32; SPECTRUM_BINS],
    pub valid_branches: u32,
    pub generation: u64,
    /// Exclusive source-frame clocks since the last analyzer reset. Bass excludes FIR delay.
    pub upper_end_frame: u64,
    pub bass_end_frame: u64,
}
impl SpectrumReading {
    pub const EMPTY: Self = Self {
        sample_rate_hz: 48_000,
        magnitudes: [0.0; SPECTRUM_BINS],
        valid_branches: 0,
        generation: 1,
        upper_end_frame: 0,
        bass_end_frame: 0,
    };
}
impl Default for SpectrumReading {
    fn default() -> Self {
        Self::EMPTY
    }
}
/// Logarithmic centers, shared by measured cells and presentation.
pub fn spectrum_frequency(index: usize) -> f32 {
    20.0 * 1000.0_f32.powf(index as f32 / (SPECTRUM_BINS - 1) as f32)
}
pub struct SpectrumMeter {
    version: AtomicU64,
    clear_epoch: AtomicU64,
    published_epoch: AtomicU64,
    generation: AtomicU64,
    rate: AtomicU32,
    valid: AtomicU32,
    upper_end: AtomicU64,
    bass_end: AtomicU64,
    bins: [AtomicU32; SPECTRUM_BINS],
}
impl Default for SpectrumMeter {
    fn default() -> Self {
        Self {
            version: AtomicU64::new(0),
            clear_epoch: AtomicU64::new(1),
            published_epoch: AtomicU64::new(1),
            generation: AtomicU64::new(1),
            rate: AtomicU32::new(48_000),
            valid: AtomicU32::new(0),
            upper_end: AtomicU64::new(0),
            bass_end: AtomicU64::new(0),
            bins: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
}
impl SpectrumMeter {
    /// Serialized active producer only; preparation must not invalidate live data.
    pub fn clear_at_rate(&self, sample_rate_hz: u32) {
        self.clear_epoch.fetch_add(1, Ordering::SeqCst);
        self.store(SpectrumReading {
            sample_rate_hz,
            generation: self.generation().wrapping_add(1),
            ..SpectrumReading::EMPTY
        });
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub fn clear_epoch(&self) -> u64 {
        self.clear_epoch.load(Ordering::SeqCst)
    }
    /// At most two attempts; no spin or wait for the producer. SeqCst payload
    /// accesses bind every cell to the two version observations in one total order.
    pub fn try_reading(&self) -> Option<SpectrumReading> {
        for _ in 0..2 {
            let version = self.version.load(Ordering::SeqCst);
            if version & 1 != 0 {
                continue;
            }
            let epoch = self.published_epoch.load(Ordering::SeqCst);
            let value = SpectrumReading {
                sample_rate_hz: self.rate.load(Ordering::SeqCst),
                magnitudes: std::array::from_fn(|i| {
                    f32::from_bits(self.bins[i].load(Ordering::SeqCst))
                }),
                valid_branches: self.valid.load(Ordering::SeqCst),
                generation: self.generation(),
                upper_end_frame: self.upper_end.load(Ordering::SeqCst),
                bass_end_frame: self.bass_end.load(Ordering::SeqCst),
            };
            if self.version.load(Ordering::SeqCst) == version && self.clear_epoch() == epoch {
                return Some(value);
            }
        }
        None
    }
    /// Convenience for diagnostics without a consumer cache. Live displays use SpectrumCache.
    pub fn reading(&self) -> SpectrumReading {
        self.try_reading().unwrap_or_default()
    }
}
/// Each UI/control consumer owns its cache; contention never blanks a valid frame,
/// and an in-progress clear cannot resurrect a frame from an earlier epoch.
#[derive(Clone, Default)]
pub struct SpectrumCache {
    last: Option<SpectrumReading>,
    epoch: u64,
}
impl SpectrumCache {
    pub fn read(&mut self, meter: &SpectrumMeter) -> Option<SpectrumReading> {
        let epoch = meter.clear_epoch();
        if epoch != self.epoch {
            self.last = None;
            self.epoch = epoch;
        }
        if let Some(value) = meter.try_reading() {
            self.last = (value.valid_branches != 0).then_some(value);
        }
        if meter.clear_epoch() != epoch {
            self.last = None;
        }
        self.last
    }
    pub fn clear(&mut self) {
        self.last = None;
    }
}
impl MeterStorage for SpectrumMeter {
    type Reading = SpectrumReading;
    const ZERO: SpectrumReading = SpectrumReading::EMPTY;
    fn store(&self, value: SpectrumReading) {
        // Sole serialized producer: preparation only attaches, it never writes.
        let previous = self.version.fetch_add(1, Ordering::SeqCst);
        debug_assert_eq!(previous & 1, 0, "concurrent spectrum producers");
        if self.generation() != value.generation {
            self.clear_epoch.fetch_add(1, Ordering::SeqCst);
        }
        self.published_epoch
            .store(self.clear_epoch(), Ordering::SeqCst);
        self.generation.store(value.generation, Ordering::SeqCst);
        self.rate.store(value.sample_rate_hz, Ordering::SeqCst);
        self.valid.store(value.valid_branches, Ordering::SeqCst);
        self.upper_end
            .store(value.upper_end_frame, Ordering::SeqCst);
        self.bass_end.store(value.bass_end_frame, Ordering::SeqCst);
        for (cell, value) in self.bins.iter().zip(value.magnitudes) {
            cell.store(
                if value.is_finite() {
                    value.max(0.0).to_bits()
                } else {
                    0
                },
                Ordering::SeqCst,
            );
        }
        self.version.fetch_add(1, Ordering::SeqCst);
    }
    fn initialize(&self, _: SpectrumReading) {} // Prepared binding is read-only.
    fn clear(&self) {
        // Invalidate caches before starting payload stores, including the interval
        // before the odd version is visible. published_epoch binds old payloads out.
        self.clear_at_rate(self.rate.load(Ordering::SeqCst));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn prepared_attachment_and_power_configuration_never_write_live_frames() {
        let meter = Arc::new(SpectrumMeter::default());
        let frame = SpectrumReading {
            magnitudes: [0.5; SPECTRUM_BINS],
            valid_branches: 3,
            generation: 7,
            upper_end_frame: 100,
            bass_end_frame: 90,
            ..Default::default()
        };
        meter.store(frame);
        let epoch = meter.clear_epoch();
        for enabled in [true, false] {
            let mut prepared = crate::UnitMeter::new(enabled);
            prepared.attach_prepared(meter.clone());
            prepared.set_prepared_enabled(!enabled);
            assert_eq!(meter.reading(), frame);
            assert_eq!(meter.clear_epoch(), epoch);
        }
    }

    #[test]
    fn cache_retains_contended_frames_but_never_resurrects_a_clear() {
        let meter = SpectrumMeter::default();
        let mut cache = SpectrumCache::default();
        let frame = SpectrumReading {
            magnitudes: [0.25; SPECTRUM_BINS],
            valid_branches: 3,
            ..Default::default()
        };
        meter.store(frame);
        assert_eq!(cache.read(&meter), Some(frame));
        meter.version.fetch_add(1, Ordering::SeqCst); // Paused normal publication.
        assert_eq!(cache.read(&meter), Some(frame));
        meter.version.fetch_add(1, Ordering::SeqCst);
        // Pause a clear after cache invalidation but before its odd version.
        meter.clear_epoch.fetch_add(1, Ordering::SeqCst);
        assert_eq!(cache.read(&meter), None);
        meter.clear();
        assert_eq!(cache.read(&meter), None);
        let next = SpectrumReading {
            generation: meter.generation(),
            ..frame
        };
        meter.store(next);
        assert_eq!(cache.read(&meter), Some(next));
    }

    #[test]
    fn concurrent_snapshots_never_mix_bins_or_metadata() {
        let meter = Arc::new(SpectrumMeter::default());
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let producer = {
            let meter = meter.clone();
            let done = done.clone();
            std::thread::spawn(move || {
                for frame in 1..=4000_u64 {
                    if frame % 31 == 0 {
                        meter.clear();
                    }
                    meter.store(SpectrumReading {
                        sample_rate_hz: 96_000,
                        magnitudes: [frame as f32; SPECTRUM_BINS],
                        valid_branches: 3,
                        generation: frame / 31 + 1,
                        upper_end_frame: frame,
                        bass_end_frame: frame,
                    });
                }
                done.store(true, Ordering::SeqCst);
            })
        };
        while !done.load(Ordering::SeqCst) {
            if let Some(frame) = meter.try_reading()
                && frame.valid_branches != 0
            {
                assert_eq!(frame.sample_rate_hz, 96_000);
                assert!(
                    frame
                        .magnitudes
                        .iter()
                        .all(|v| *v == frame.upper_end_frame as f32)
                );
                assert_eq!(frame.upper_end_frame, frame.bass_end_frame);
                assert_eq!(frame.generation, frame.upper_end_frame / 31 + 1);
            }
            std::thread::yield_now();
        }
        producer.join().unwrap();
    }
}
