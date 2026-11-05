use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Track and Master monitoring share one latched stereo clipping flag.
#[derive(Default)]
pub struct StereoMeter {
    pub peaks: [AtomicU32; 2],
    held_peaks: [AtomicU32; 2],
    clipped: AtomicBool,
}

impl StereoMeter {
    /// Publish pre-clamp amplitudes without allocating or locking the audio callback.
    pub fn publish_peaks(&self, peaks: [f32; 2]) {
        for (channel, peak) in peaks.into_iter().enumerate() {
            self.peaks[channel].store(peak.to_bits(), Ordering::Relaxed);
        }
        // Positive finite f32 bit patterns have the same order as their amplitudes.
        for (hold, peak) in self.held_peaks.iter().zip(peaks) {
            if peak.is_finite() && peak > 0.0 {
                hold.fetch_max(peak.to_bits(), Ordering::Relaxed);
            }
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
        let [left, right] = self.held_peaks();
        left.max(right)
    }

    pub fn held_peaks(&self) -> [f32; 2] {
        self.held_peaks
            .each_ref()
            .map(|peak| f32::from_bits(peak.load(Ordering::Relaxed)))
    }

    /// Reset the held peak and stereo clipping warning. New audio can raise both again.
    pub fn reset(&self) {
        for peak in &self.held_peaks {
            peak.store(0, Ordering::Relaxed);
        }
        self.clipped.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_holds_retain_independent_maxima_through_silence_and_reset() {
        let meter = StereoMeter::default();
        meter.publish_peaks([0.75, 0.25]);
        meter.publish_peaks([0.1, 0.5]);
        meter.publish_peaks([0.0; 2]);
        assert_eq!(meter.held_peaks(), [0.75, 0.5]);
        assert_eq!(meter.held_peak(), 0.75);
        meter.publish_peaks([f32::NAN, f32::INFINITY]);
        assert_eq!(meter.held_peaks(), [0.75, 0.5]);
        meter.reset();
        assert_eq!(meter.held_peaks(), [0.0; 2]);
        assert!(!meter.is_clipped());
        meter.publish_peaks([0.2, 0.3]);
        assert_eq!(meter.held_peaks(), [0.2, 0.3]);
    }

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
