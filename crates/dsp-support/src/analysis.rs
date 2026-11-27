//! Prepared numerical measurements with publication owned by the caller.
pub use daw_analysis_kernels::{loudness, spectrum};

pub fn release_spectrum(
    magnitudes: &mut [f32; spectrum::SPECTRUM_BINS],
    powers: &[f32; spectrum::SPECTRUM_BINS],
    elapsed: u64,
    rate: u32,
) {
    let release =
        (-(elapsed as f32) / rate as f32 / crate::levels::LEVEL_METER_RELEASE_SECONDS).exp();
    for (magnitude, &power) in magnitudes.iter_mut().zip(powers) {
        let amplitude = power.max(0.0).sqrt();
        *magnitude = amplitude.max(*magnitude * release);
    }
}
