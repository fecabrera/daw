use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Track and Master monitoring share one latched stereo clipping flag.
#[derive(Default)]
pub struct StereoMeter {
    pub peaks: [AtomicU32; 2],
    held_peak: AtomicU32,
    clipped: AtomicBool,
}

impl StereoMeter {
    /// Publish pre-clamp amplitudes without allocating or locking the audio callback.
    pub fn publish_peaks(&self, peaks: [f32; 2]) {
        for (channel, peak) in peaks.into_iter().enumerate() {
            self.peaks[channel].store(peak.to_bits(), Ordering::Relaxed);
        }
        // Positive finite f32 bit patterns have the same order as their amplitudes.
        let peak = peaks[0].max(peaks[1]);
        if peak.is_finite() && peak > 0.0 {
            self.held_peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
        }
        if peaks.into_iter().any(|peak| peak > 1.0) {
            self.clipped.store(true, Ordering::Relaxed);
        }
    }

    pub fn is_clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }

    /// Read the shared flag once so both channel indicators agree within a UI frame.
    pub fn levels(&self) -> [(f32, bool); 2] {
        let clipped = self.is_clipped();
        self.peaks
            .each_ref()
            .map(|peak| (f32::from_bits(peak.load(Ordering::Relaxed)), clipped))
    }

    pub fn held_peak(&self) -> f32 {
        f32::from_bits(self.held_peak.load(Ordering::Relaxed))
    }

    /// Reset the held peak and stereo clipping warning. New audio can raise both again.
    pub fn reset(&self) {
        self.held_peak.store(0, Ordering::Relaxed);
        self.clipped.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_channel_latches_both_until_reset_without_changing_levels() {
        for peaks in [[1.25, 0.5], [0.5, 1.25], [1.25; 2]] {
            let meter = StereoMeter::default();
            meter.publish_peaks([1.0; 2]);
            assert!(!meter.is_clipped());
            meter.publish_peaks(peaks);
            assert_eq!(meter.levels(), peaks.map(|peak| (peak, true)));
            meter.publish_peaks([0.25, 0.5]);
            assert_eq!(meter.levels(), [(0.25, true), (0.5, true)]);
            meter.reset();
            assert_eq!(meter.levels(), [(0.25, false), (0.5, false)]);
            assert_eq!(meter.held_peak(), 0.0);
            meter.publish_peaks(peaks);
            assert!(meter.is_clipped());
        }
    }
}
