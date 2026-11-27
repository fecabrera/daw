//! Unit-side reverb facade and parameter adapters.
mod processor;
pub use daw_dsp_support::reverb::{
    BloomDelays, Delay, DelayRing, Noise, TapTransition, junction, mix_gains, rational_map,
    wet_width,
};
pub use processor::{Parameters, Reverb};

/// Convert between a product's ordered controls and the prepared reverb slots.
pub struct ParameterSlots<const N: usize>(pub [usize; N]);

impl<const N: usize> ParameterSlots<N> {
    pub fn gather(&self, values: [f32; 12]) -> [f32; N] {
        self.0.map(|slot| values[slot])
    }

    pub fn scatter(&self, values: [f64; N]) -> [f64; 12] {
        let mut offsets = [0.0; 12];
        for (slot, value) in self.0.into_iter().zip(values) {
            offsets[slot] = value;
        }
        offsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_maps_preserve_reference_midpoints_and_mix() {
        for (a, c, b) in [(0.3, 3.0, 15.0), (1.0, 7.5, 60.0), (24.0, 20.0, 16.0)] {
            assert_eq!(rational_map(0.0, a, c, b), a);
            assert!((rational_map(0.5, a, c, b) - c).abs() < 1e-12);
            assert_eq!(rational_map(1.0, a, c, b), b);
        }
        assert_eq!(mix_gains(0.0, false), [1.0, 0.0]);
        assert_eq!(mix_gains(1.0, true), [0.0, 1.0]);
        assert_eq!(mix_gains(0.5, false), [0.75, 0.75]);
        let plate = mix_gains(0.5, true);
        assert!((plate[0] - 0.858974358974359).abs() < 1e-12);
        assert!((plate[1] - 0.5963855421686747).abs() < 1e-12);
    }
    #[test]
    fn prime_mapping_matches_cached_delays_and_all_project_rate_bounds() {
        let table = BloomDelays::default();
        assert_eq!(
            table.frames(0.5, 0.5, 44100),
            [
                89, 239, 433, 647, 887, 1163, 1453, 1753, 2087, 2417, 2777, 3181, 3547, 3929, 4349,
                4789
            ]
        );
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for bloom in [0.0, 0.5, 1.0] {
                for decay in [0.0, 0.5, 1.0] {
                    let frames = table.frames(bloom, decay, rate);
                    assert!(frames.windows(2).all(|w| w[0] <= w[1]));
                    assert!(frames.iter().all(|f| *f > 0 && *f <= 104729));
                }
            }
        }
    }
    #[test]
    fn delay_wrap_interpolation_and_reset_preserve_timing() {
        let mut d = Delay::new(4);
        for i in 0..30 {
            d.write([f64::from(i), -f64::from(i)]);
        }
        assert_eq!(d.read(1.0), [29.0, -29.0]);
        assert_eq!(d.read(3.5), [26.5, -26.5]);
        d.reset();
        assert_eq!(d.read(4.0), [0.0; 2]);
    }
    #[test]
    fn noise_reset_is_repeatable_and_lanes_stay_in_range() {
        let mut n = Noise::new([1, 2, 3, 4]);
        let first = n.sample();
        for _ in 0..10000 {
            assert!(n.sample().iter().all(|x| (-1.0..1.0).contains(x)));
        }
        n.reset();
        assert_eq!(n.sample(), first);
        assert_ne!(first[0], first[1]);
    }
    #[test]
    fn width_changes_only_side() {
        assert_eq!(wet_width([1.0, -1.0], 0.0), [0.0; 2]);
        assert_eq!(wet_width([0.5, 0.5], 0.0), [0.5; 2]);
        assert_eq!(wet_width([0.5, -0.25], 1.0), [0.5, -0.25]);
    }
}
