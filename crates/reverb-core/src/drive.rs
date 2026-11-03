//! Plate output-drive curves share gain, bypass and a bounded mode crossfade.
use daw_unit_core::ModulationRamp;

pub(crate) fn weights(ramps: &mut [ModulationRamp; 3]) -> [f64; 3] {
    let values = ramps.each_mut().map(|r| f64::from(r.next_value()));
    // Interrupted retargets can leave different remaining ramp lengths.
    // Normalize the mixture so every transition stays a convex combination.
    let sum = values.iter().sum::<f64>().max(f64::EPSILON);
    values.map(|v| v / sum)
}
fn soft(y: f64) -> f64 {
    // Preserve the original rational tanh approximation exactly.
    let z = y * y;
    y * (((z + 378.0) * z + 17325.0) * z + 135135.0)
        / (((28.0 * z + 3150.0) * z + 62370.0) * z + 135135.0)
}
fn tube(y: f64) -> f64 {
    // Bias adds even harmonics; subtract it for exact silence and normalize
    // to unity small-signal slope with a maximum excursion of one.
    let bias = 0.3_f64.tanh();
    ((y / (1.0 - bias) + 0.3).tanh() - bias) / (1.0 + bias)
}
pub(crate) fn shape(y: f64, weights: [f64; 3]) -> f64 {
    soft(y) * weights[0] + y.clamp(-1.0, 1.0) * weights[1] + tube(y) * weights[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_preserve_silence_and_soft_compatibility_and_tube_is_asymmetric() {
        for weights in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            assert_eq!(shape(0.0, weights), 0.0);
        }
        for x in [-4.0, -1.0, -0.1, 0.1, 1.0, 4.0] {
            assert_eq!(shape(x, [1.0, 0.0, 0.0]), soft(x));
            assert_eq!(shape(x, [0.0, 1.0, 0.0]), x.clamp(-1.0, 1.0));
            assert!((-1.0..=1.0).contains(&tube(x)));
        }
        assert!((soft(0.8) + soft(-0.8)).abs() < 1e-12);
        assert!((tube(0.8) + tube(-0.8)).abs() > 0.05);
        assert!((tube(1e-6) / 1e-6 - 1.0).abs() < 1e-5);
    }
    #[test]
    fn interrupted_mode_transitions_keep_unity_weights_and_settle() {
        let mut ramps = [
            ModulationRamp::new(1.0),
            ModulationRamp::new(0.0),
            ModulationRamp::new(0.0),
        ];
        for target in [2, 1, 0, 2] {
            for (i, ramp) in ramps.iter_mut().enumerate() {
                ramp.set(f32::from(i == target), 240);
            }
            for _ in 0..37 {
                let w = weights(&mut ramps);
                assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                assert!(w.iter().all(|v| (0.0..=1.0).contains(v)));
                assert!(shape(0.8, w).is_finite());
            }
        }
        for _ in 0..240 {
            weights(&mut ramps);
        }
        assert_eq!(weights(&mut ramps), [0.0, 0.0, 1.0]);
    }
}
