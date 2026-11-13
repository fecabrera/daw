use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Device time, rather than project rate or callback count, sets display release.
pub(super) fn release_for_frames(frames: usize, sample_rate_hz: u32) -> f32 {
    (-(frames as f32) / sample_rate_hz as f32 / daw_unit_core::LEVEL_METER_RELEASE_SECONDS).exp()
}

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
        self.publish_block_peaks(peaks, 0.0);
    }

    /// Apply display release once per device buffer. Holds and clipping always
    /// use raw peaks, so a reset never re-holds a decaying display value.
    pub(super) fn publish_block_peaks(&self, peaks: [f32; 2], release: f32) {
        for (channel, peak) in peaks.into_iter().enumerate() {
            let previous = f32::from_bits(self.peaks[channel].load(Ordering::Relaxed));
            let previous = if previous.is_finite() {
                previous.max(0.0)
            } else {
                0.0
            };
            let peak = if peak.is_finite() { peak.max(0.0) } else { 0.0 };
            let level = peak.max(previous * release);
            self.peaks[channel].store(level.to_bits(), Ordering::Relaxed);
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

    /// Read released display levels and one shared raw-signal clipping flag so
    /// both channel indicators agree within a UI frame.
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
    fn display_release_matches_elapsed_time_at_every_rate_and_buffer_size() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            for buffer in [32, 128, 512, 4096] {
                let meter = StereoMeter::default();
                meter.publish_block_peaks([1.25, 0.5], release_for_frames(buffer, rate));
                assert_eq!(meter.levels(), [(1.25, true), (0.5, true)]);
                let mut frames = rate as usize * 3 / 10;
                while frames > 0 {
                    let count = frames.min(buffer);
                    meter.publish_block_peaks([0.0; 2], release_for_frames(count, rate));
                    frames -= count;
                }
                for (actual, initial) in meter.levels().into_iter().zip([1.25, 0.5]) {
                    assert!((actual.0 - initial / std::f32::consts::E).abs() < 0.0002);
                }
                assert_eq!(meter.held_peaks(), [1.25, 0.5]);
                assert!(meter.is_clipped());
            }
        }
    }

    #[test]
    fn dynamic_blocks_do_not_blank_the_display_or_rehold_released_values() {
        let meter = StereoMeter::default();
        let release = release_for_frames(512, 48_000);
        for _ in 0..20 {
            meter.publish_block_peaks([1.25, 0.5], release);
            meter.publish_block_peaks([0.0; 2], release);
            assert!(meter.levels()[0].0 > 1.2);
            assert!(meter.levels()[1].0 > 0.48);
        }
        meter.reset();
        meter.publish_block_peaks([0.0; 2], release);
        assert!(meter.levels()[0].0 > 1.0);
        assert_eq!(meter.held_peaks(), [0.0; 2]);
        assert!(!meter.is_clipped());
        meter.publish_block_peaks([0.1, 0.2], release);
        assert_eq!(meter.held_peaks(), [0.1, 0.2]);
        assert!(!meter.is_clipped());
    }

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
