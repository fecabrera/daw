use crate::UNIT_STATE_VERSION;
use serde::{Deserialize, Serialize};

pub const MAX_ENVELOPE_TIME_MS: f32 = 10_000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveShape {
    #[default]
    Linear,
    Exponential,
    Logarithmic,
    SCurve,
}
impl CurveShape {
    pub const ALL: [Self; 4] = [
        Self::Linear,
        Self::Exponential,
        Self::Logarithmic,
        Self::SCurve,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Exponential => "Exponential",
            Self::Logarithmic => "Logarithmic",
            Self::SCurve => "S-curve",
        }
    }
    /// Normalized stage progress, with exact endpoints. Falling stages reverse
    /// exponential/logarithmic curvature so exponential releases have a long tail.
    pub fn progress(self, position: f32, amount: f32, falling: bool) -> f32 {
        let t = position.clamp(0.0, 1.0);
        let amount = amount.clamp(0.0, 1.0);
        if amount == 0.0 {
            return t;
        }
        let power = 1.0 + 7.0 * amount;
        match self {
            Self::Linear => t,
            Self::Exponential if !falling => t.powf(power),
            Self::Exponential => 1.0 - (1.0 - t).powf(power),
            Self::Logarithmic if !falling => 1.0 - (1.0 - t).powf(power),
            Self::Logarithmic => t.powf(power),
            Self::SCurve => {
                let smooth = t * t * (3.0 - 2.0 * t);
                t + amount * (smooth - t)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeStageSettings {
    pub time_ms: f32,
    pub shape: CurveShape,
    /// 0–1 curve strength. Linear ignores it without discarding its saved value.
    pub amount: f32,
}
impl EnvelopeStageSettings {
    pub fn valid(&self) -> bool {
        self.time_ms.is_finite()
            && (0.0..=MAX_ENVELOPE_TIME_MS).contains(&self.time_ms)
            && self.amount.is_finite()
            && (0.0..=1.0).contains(&self.amount)
    }
    pub fn frames(&self, sample_rate_hz: u32) -> u64 {
        (f64::from(self.time_ms) * f64::from(sample_rate_hz) / 1000.0) as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeSettings {
    pub version: u32,
    pub attack: EnvelopeStageSettings,
    pub decay: EnvelopeStageSettings,
    /// Held level, 0–1. Sustain has no duration or curve.
    pub sustain: f32,
    pub release: EnvelopeStageSettings,
}
impl Default for EnvelopeSettings {
    fn default() -> Self {
        Self {
            version: UNIT_STATE_VERSION,
            attack: EnvelopeStageSettings {
                time_ms: 5.0,
                shape: CurveShape::Linear,
                amount: 0.5,
            },
            decay: EnvelopeStageSettings {
                time_ms: 100.0,
                shape: CurveShape::Exponential,
                amount: 0.5,
            },
            sustain: 0.8,
            release: EnvelopeStageSettings {
                time_ms: 200.0,
                shape: CurveShape::Exponential,
                amount: 0.5,
            },
        }
    }
}
impl EnvelopeSettings {
    /// Preserve the fixed 5 ms gate used before envelopes were saved.
    pub fn legacy() -> Self {
        let stage = EnvelopeStageSettings {
            time_ms: 5.0,
            shape: CurveShape::Linear,
            amount: 0.0,
        };
        Self {
            version: UNIT_STATE_VERSION,
            attack: stage,
            decay: EnvelopeStageSettings {
                time_ms: 0.0,
                ..stage
            },
            sustain: 1.0,
            release: stage,
        }
    }
    pub fn valid(&self) -> bool {
        self.version == UNIT_STATE_VERSION
            && self.attack.valid()
            && self.decay.valid()
            && self.release.valid()
            && self.sustain.is_finite()
            && (0.0..=1.0).contains(&self.sustain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_shapes_keep_endpoints_monotonic_and_zero_amount_is_linear() {
        for shape in CurveShape::ALL {
            for falling in [true, false] {
                for amount in [0.0, 0.5, 1.0] {
                    assert_eq!(shape.progress(0.0, amount, falling), 0.0);
                    assert_eq!(shape.progress(1.0, amount, falling), 1.0);
                    let mut previous = 0.0;
                    for i in 0..=100 {
                        let t = i as f32 / 100.0;
                        let value = shape.progress(t, amount, falling);
                        assert!((previous..=1.0).contains(&value));
                        if amount == 0.0 {
                            assert_eq!(value, t);
                        }
                        previous = value;
                    }
                }
            }
        }
    }
}
