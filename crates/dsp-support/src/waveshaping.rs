//! Static transfer curves used in audio callbacks.

mod normalized;
pub use normalized::NormalizedClipper;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    AnalogClip,
    SoftSine,
    BassShaper,
    MediumCurve,
    HardCurve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipMode {
    NoClip,
    SoftClip,
    HardClip,
    TubeClip,
}

fn knee(input: f64, start: f64, ceiling: f64) -> f64 {
    let x = input.abs();
    if x <= start {
        input
    } else {
        (start + (ceiling - start) * ((x - start) / (ceiling - start)).tanh()).copysign(input)
    }
}
/// Shared static saturation transfer, with unity small-signal gain.
#[inline]
pub fn shape(shape: Shape, x: f64) -> f64 {
    match shape {
        Shape::AnalogClip => knee(x, 0.75, 1.5),
        Shape::SoftSine => {
            // Wide sine arc: gentle compression, separated from the firmer cubic mode.
            3.0 * (x / 3.0)
                .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2)
                .sin()
        }
        // Broad rational knee with unity small-signal gain and no frequency-dependent gain boost.
        Shape::BassShaper => x / (1.0 + (x / 1.8).powi(2)).sqrt(),
        Shape::MediumCurve => {
            // Zero slope at the plateau joins the cubic continuously to its ceiling.
            const THRESHOLD: f64 = 1.65;
            let y = x.clamp(-THRESHOLD, THRESHOLD);
            y - y.powi(3) / (3.0 * THRESHOLD * THRESHOLD)
        }
        // A real hard knee adds upper odd harmonics. Its 1.2 ceiling remains
        // distinct from the optional final Hard Clip stage at unity.
        Shape::HardCurve => x.clamp(-1.2, 1.2),
    }
}
/// Apply the Saturator clip transfer to one sample.
#[inline]
pub fn clip(mode: ClipMode, x: f64) -> f64 {
    match mode {
        ClipMode::NoClip => x,
        ClipMode::SoftClip => knee(x, 0.5, 1.0),
        ClipMode::HardClip => x.clamp(-1.0, 1.0),
        ClipMode::TubeClip => tube_clip(x),
    }
}

// Asymmetric clip with unity slope at zero and a maximum excursion of one.
fn tube_clip(input: f64) -> f64 {
    const OFFSET: f64 = 0.3;
    let baseline = OFFSET.tanh();
    let scale = 1.0 / (1.0 - baseline);
    biased_tanh(input * scale, OFFSET, baseline) / (1.0 + baseline)
}

/// Shift a tanh curve and subtract its precomputed value at zero.
#[inline]
pub fn biased_tanh(input: f64, offset: f64, baseline: f64) -> f64 {
    (input + offset).tanh() - baseline
}

/// Rational tanh approximation used by the reverb wet drive.
#[inline]
pub fn rational_tanh(input: f64) -> f64 {
    let squared = input * input;
    input * (((squared + 378.0) * squared + 17325.0) * squared + 135135.0)
        / (((28.0 * squared + 3150.0) * squared + 62370.0) * squared + 135135.0)
}
