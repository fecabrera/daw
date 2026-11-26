//! Signed waveform extrema. The caller chooses the seed and frame range.

/// An empty extrema pair, ordered as minimum then maximum.
pub const EMPTY: [f32; 2] = [f32::INFINITY, f32::NEG_INFINITY];

#[inline]
pub fn merge([minimum, maximum]: [f32; 2], [other_min, other_max]: [f32; 2]) -> [f32; 2] {
    [minimum.min(other_min), maximum.max(other_max)]
}

#[inline]
pub fn include(extrema: [f32; 2], sample: f32) -> [f32; 2] {
    merge(extrema, [sample; 2])
}

/// Update the two channel ranges independently.
#[inline]
pub fn include_stereo(extrema: &mut [[f32; 2]; 2], frame: [f32; 2]) {
    for (pair, sample) in extrema.iter_mut().zip(frame) {
        *pair = include(*pair, sample);
    }
}

/// Reduce a borrowed stereo block using the caller's channel seeds.
pub fn stereo_extrema(frames: &[[f32; 2]], mut seed: [[f32; 2]; 2]) -> [[f32; 2]; 2] {
    for &frame in frames {
        include_stereo(&mut seed, frame);
    }
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_and_reduction_order_match_the_previous_waveform_folds() {
        let zero = f32::from_bits(0x8000_0000);
        for seed in [[EMPTY; 2], [[0.0; 2]; 2], [[zero; 2]; 2]] {
            for frames in [
                &[][..],
                &[[0.2, -0.4], [0.8, -0.1]],
                &[[zero, 0.0], [0.0, zero], [-0.7, 0.7]],
                &[[0.0, 0.0], [f32::NAN, 0.1]],
            ] {
                let mut previous = seed;
                for &frame in frames {
                    for (extrema, sample) in previous.iter_mut().zip(frame) {
                        extrema[0] = extrema[0].min(sample);
                        extrema[1] = extrema[1].max(sample);
                    }
                }
                let bits = |values: [[f32; 2]; 2]| values.map(|pair| pair.map(f32::to_bits));
                let actual = stereo_extrema(frames, seed);
                assert_eq!(bits(actual), bits(previous));
            }
        }
    }
}
