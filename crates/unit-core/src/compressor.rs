use serde::{Deserialize, Serialize};

pub const COMPRESSOR_TYPE_ID: &str = "daw.compressor";
pub const COMPRESSOR_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressorDetection {
    #[default]
    Peak,
    Rms,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompressorSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub detection: CompressorDetection,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Soft-knee width in dB, centered on threshold. Zero selects a hard knee.
    pub knee_db: f32,
    /// Post-knee output curvature: negative exponential, positive logarithmic.
    #[serde(default)]
    pub curve: f32,
    pub level_db: f32,
    pub dry_wet: f32,
}
impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            version: COMPRESSOR_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            threshold_db: -18.0,
            ratio: 4.0,
            detection: CompressorDetection::Peak,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            curve: 0.0,
            level_db: 0.0,
            dry_wet: 1.0,
        }
    }
}
impl CompressorSettings {
    pub fn valid(&self) -> bool {
        fn bounded(value: f32, min: f32, max: f32) -> bool {
            value.is_finite() && (min..=max).contains(&value)
        }
        self.version == COMPRESSOR_STATE_VERSION
            && bounded(self.threshold_db, -60.0, 0.0)
            && bounded(self.ratio, 1.0, 20.0)
            && bounded(self.attack_ms, 0.1, 200.0)
            && bounded(self.release_ms, 10.0, 2000.0)
            && bounded(self.knee_db, 0.0, 24.0)
            && bounded(self.curve, -1.0, 1.0)
            && bounded(self.level_db, crate::SILENT_GAIN_DB, 12.0)
            && bounded(self.dry_wet, 0.0, 1.0)
            && crate::ModulationDestinationSettings::valid_modulations(self)
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

#[cfg(test)]
mod curve_tests {
    use super::*;
    #[test]
    fn post_knee_shape_is_continuous_monotone_downward_and_neutral_is_legacy_exact() {
        for threshold in [-60.0, -18.0, 0.0] {
            for ratio in [1.0_f32, 2.0, 4.0, 20.0] {
                for knee in [0.0, 6.0, 24.0] {
                    let slope = 1.0 - ratio.recip();
                    let edge = threshold + knee * 0.5;
                    for curve in [-1.0, -0.01, 0.0, 0.01, 1.0] {
                        let mut previous = f32::NEG_INFINITY;
                        for input in -90..=800 {
                            let input = input as f32;
                            let legacy =
                                compressor_gain_reduction_db(input, threshold, slope, knee);
                            let reduction = compressor_shaped_gain_reduction_db(
                                input, threshold, slope, knee, curve,
                            );
                            let output = input - reduction;
                            assert!(reduction.is_finite() && reduction >= 0.0);
                            assert!(output <= input + 1e-4 && output >= previous - 1e-4);
                            if previous.is_finite() {
                                assert!(output - previous <= 1.0002);
                            }
                            previous = output;
                            if curve == 0.0 || input <= edge || ratio == 1.0 {
                                assert_eq!(reduction.to_bits(), legacy.to_bits());
                            } else if curve < 0.0 {
                                assert!(reduction <= legacy + 1e-4);
                            } else {
                                assert!(reduction >= legacy - 1e-4);
                            }
                        }
                        let at = compressor_shaped_gain_reduction_db(
                            edge, threshold, slope, knee, curve,
                        );
                        let after = compressor_shaped_gain_reduction_db(
                            edge + 0.01,
                            threshold,
                            slope,
                            knee,
                            curve,
                        );
                        let derivative = 1.0 - (after - at) / 0.01;
                        assert!(
                            (derivative - ratio.recip()).abs() < 0.012,
                            "T={threshold} R={ratio} K={knee} C={curve} slope={derivative}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn legacy_settings_default_curve_to_zero_and_roundtrip_new_values() {
        let mut json = serde_json::to_value(CompressorSettings::default()).unwrap();
        json.as_object_mut().unwrap().remove("curve");
        let legacy: CompressorSettings = serde_json::from_value(json).unwrap();
        assert_eq!(legacy.curve, 0.0);
        assert!(legacy.valid());
        for curve in [-1.0, 0.0, 1.0] {
            let s = CompressorSettings {
                curve,
                ..legacy.clone()
            };
            assert_eq!(
                serde_json::from_str::<CompressorSettings>(&serde_json::to_string(&s).unwrap())
                    .unwrap(),
                s
            );
        }
        for curve in [-1.01, 1.01, f32::NAN, f32::INFINITY] {
            assert!(
                !CompressorSettings {
                    curve,
                    ..legacy.clone()
                }
                .valid()
            );
        }
    }
}
