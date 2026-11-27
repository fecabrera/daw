//! Stereo matrix processing.

fn lr_mid_gain(spread: f64) -> f64 {
    // Reference L/R Mid gains at Spread 1, 1.25, 1.5, 1.75, and 2.
    // Interpolate continuously; do not quantize the shared control or offsets.
    const GAINS: [f64; 5] = [1.0, 0.818, 0.65, 0.482, 0.3];
    let position = (spread - 1.0).max(0.0) * 4.0;
    let index = (position as usize).min(GAINS.len() - 2);
    GAINS[index] + (GAINS[index + 1] - GAINS[index]) * (position - index as f64)
}

/// Apply the direction and spread matrix with continuous input and wet blends.
pub fn direction(dry: [f32; 2], direction: f64, spread: f64, mode: f64, wet: f64) -> [f32; 2] {
    let [left, right] = dry.map(f64::from);
    let mid_lr = (left + right) * 0.5;
    let side_lr = (left - right) * 0.5;
    // L/R narrows by reducing Side below unity, then widens by reducing Mid.
    // The upper segments follow the measured reference gains.
    // M/S retains direct Side-level control. Blend complete interpretations
    // through the existing input-mode ramp to avoid a discontinuous switch.
    let mid_lr = mid_lr * lr_mid_gain(spread);
    let side_lr = side_lr * spread.min(1.0);
    let mid = mid_lr + (left - mid_lr) * mode;
    let side = side_lr + (right * spread - side_lr) * mode;
    // Positive angles pan the center right, matching clockwise knob movement.
    // ±180 exchanges sides without polarity inversion. Exact cardinal coefficients
    // keep swaps and mono endpoints exact.
    let (sin, cos) = match direction {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        -90.0 => (-1.0, 0.0),
        -180.0 | 180.0 => (0.0, -1.0),
        _ => direction.to_radians().sin_cos(),
    };
    let output = [
        mid * (1.0 - sin).sqrt() + side * cos,
        mid * (1.0 + sin).sqrt() - side * cos,
    ];
    std::array::from_fn(|ch| {
        (f64::from(dry[ch]) + (output[ch] - f64::from(dry[ch])) * wet)
            .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    })
}
