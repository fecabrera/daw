//! Linked compression and expansion, with private detector backend.
use fundsp::dynamics::{Meter, MeterState};

#[derive(Clone, Copy)]
pub enum Direction {
    Compressor,
    Expander,
}

impl Direction {
    pub fn gain_reduction_db(
        self,
        input: f32,
        threshold: f32,
        slope: f32,
        knee: f32,
        curve: f32,
    ) -> f32 {
        match self {
            Self::Compressor => {
                compressor_shaped_gain_reduction_db(input, threshold, slope, knee, curve)
            }
            Self::Expander => {
                // Reflect the detector about the threshold and convert the
                // compressor slope (1 - 1/ratio) to an expansion slope (ratio - 1).
                compressor_shaped_gain_reduction_db(-input, -threshold, slope, knee, curve)
                    / (1.0 - slope)
            }
        }
    }
}

/// Soft-knee gain reduction in dB. The knee joins the two slopes continuously.
/// Inputs use validated compressor bounds; slope is `1 - 1 / ratio`.
pub fn compressor_gain_reduction_db(
    level_db: f32,
    threshold_db: f32,
    slope: f32,
    knee: f32,
) -> f32 {
    let over = level_db - threshold_db;
    if knee > 0.0 && over > -knee * 0.5 && over < knee * 0.5 {
        slope * (over + knee * 0.5).powi(2) / (2.0 * knee)
    } else {
        slope * over.max(0.0)
    }
}

/// Output shaping starts above the knee, preserving its value and tangent.
/// The exponential branch caps its output slope at unity, preventing expansion.
/// Zero retains the original arithmetic exactly, including legacy rounding.
#[inline]
pub fn compressor_shaped_gain_reduction_db(
    level_db: f32,
    threshold_db: f32,
    slope: f32,
    knee: f32,
    curve: f32,
) -> f32 {
    let edge = threshold_db + knee * 0.5;
    if curve == 0.0 || slope <= 0.0 || level_db <= edge {
        return compressor_gain_reduction_db(level_db, threshold_db, slope, knee);
    }
    let distance = f64::from(level_db) - f64::from(edge);
    let slope = f64::from(slope);
    let tangent = 1.0 - slope;
    // Full curvature uses a 12 dB characteristic distance. exp_m1/ln_1p
    // retain precision while the smoothed coefficient crosses zero.
    let coefficient = f64::from(curve.abs()) / 12.0;
    let extra = if curve < 0.0 {
        let limit = -tangent.ln() / coefficient;
        if distance >= limit {
            (-tangent.ln() - slope) / coefficient
        } else {
            distance - tangent * (coefficient * distance).exp_m1() / coefficient
        }
    } else {
        distance - tangent * (coefficient * distance).ln_1p() / coefficient
    };
    (slope * f64::from(knee) * 0.5 + extra).max(0.0) as f32
}

const RMS: Meter = Meter::Rms(0.01);
const EXPANDER_PEAK: Meter = Meter::Peak(0.030);
const EXPANDER_MAX_REDUCTION_DB: f32 = 120.0;

#[derive(Clone)]
pub struct DynamicsDetector {
    direction: Direction,
    rms: [MeterState; 2],
    peak_detector: Option<MeterState>,
    rate: f64,
    configured_times: [f32; 2],
    coefficients: [f64; 2],
    reduction_db: f64,
}

impl DynamicsDetector {
    pub fn new(sample_rate: u32, direction: Direction) -> Self {
        let rate = f64::from(sample_rate);
        let rms = std::array::from_fn(|_| {
            let mut state = MeterState::new(RMS);
            state.set_sample_rate(RMS, rate);
            state
        });
        Self {
            direction,
            rms,
            peak_detector: matches!(direction, Direction::Expander).then(|| {
                let mut state = MeterState::new(EXPANDER_PEAK);
                state.set_sample_rate(EXPANDER_PEAK, rate);
                state
            }),
            rate,
            configured_times: [f32::NAN; 2],
            coefficients: [0.0; 2],
            reduction_db: 0.0,
        }
    }

    pub fn reset(&mut self) {
        for state in &mut self.rms {
            state.reset(RMS);
        }
        if let Some(detector) = &mut self.peak_detector {
            detector.reset(EXPANDER_PEAK);
        }
        self.reduction_db = 0.0;
    }

    pub fn reduction_db(&self) -> f32 {
        self.reduction_db as f32
    }

    pub fn configure(&mut self, times: [f32; 2]) {
        for (index, time) in times.into_iter().enumerate() {
            if time != self.configured_times[index] {
                self.coefficients[index] =
                    (-1.0 / (f64::from(time).exp() * 0.001 * self.rate)).exp();
                self.configured_times[index] = time;
            }
        }
    }
    /// Process the stereo pair and return output, detector reading and input peak.
    pub fn sample(&mut self, input: [f32; 2], p: &[f32; 10]) -> ([f32; 2], f32, f32) {
        let input = input.map(|value| if value.is_finite() { value } else { 0.0 });
        let peak = input[0].abs().max(input[1].abs());
        let mut rms: f32 = 0.0;
        for (state, sample) in self.rms.iter_mut().zip(input) {
            // Bound only the detector input to prevent square overflow with extreme headroom.
            state.tick(RMS, sample.clamp(-1e15, 1e15));
            let level = state.level(RMS);
            if level < 1e-15 {
                state.reset(RMS);
            } else {
                rms = rms.max(level);
            }
        }
        // Follow the linked peak across zero crossings when expanding.
        let detected_peak = if let Some(detector) = &mut self.peak_detector {
            detector.tick(EXPANDER_PEAK, peak);
            let level = detector.level(EXPANDER_PEAK);
            if level < 1e-15 {
                detector.reset(EXPANDER_PEAK);
            }
            level
        } else {
            peak
        };
        let detected = detected_peak * (1.0 - p[5]) + rms * p[5];
        let level_db = 20.0 * detected.max(1e-20).log10();
        let target = self
            .direction
            .gain_reduction_db(level_db, p[0], p[1], p[2], p[9]);
        // Bound how far the envelope must travel to reopen after digital silence.
        let target = f64::from(match self.direction {
            Direction::Compressor => target,
            Direction::Expander => target.min(EXPANDER_MAX_REDUCTION_DB),
        });
        let releasing = match self.direction {
            Direction::Compressor => target < self.reduction_db,
            Direction::Expander => target > self.reduction_db,
        };
        let coefficient = self.coefficients[usize::from(releasing)];
        self.reduction_db = target + coefficient * (self.reduction_db - target);
        if self.reduction_db < 1e-8 {
            self.reduction_db = 0.0;
        }
        let gain = f64::from(10_f32.powf(-(self.reduction_db as f32) / 20.0));
        let output = input.map(|dry| {
            let dry = f64::from(dry);
            let mix = f64::from(p[7]);
            let enabled = f64::from(p[8]);
            let wet = dry * ((1.0 - mix) + gain * mix) * f64::from(p[6]);
            (dry * (1.0 - enabled) + wet * enabled).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
                as f32
        });
        (output, detected, peak)
    }
}
