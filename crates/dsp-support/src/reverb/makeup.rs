//! Offline-calibrated wet gain, outside the tank and its feedback loops.
//! Pink noise and chord/percussion fixtures target roughly -6 dB wet/input RMS
//! at default decay. See specs/effects/reverbs.md for calibration and bounds.

#[derive(Clone)]
pub(crate) struct MakeupRamp {
    value: f64,
    rise: f64,
    fall: f64,
}

impl MakeupRamp {
    pub fn new(rate: u32, value: f64) -> Self {
        Self {
            value,
            // Give the previous, louder stored tail time to dissipate before
            // raising short-decay compensation. Never inspect signal energy.
            rise: 1.0 - (-1.0 / (0.5 * f64::from(rate))).exp(),
            fall: 1.0 - (-1.0 / (0.005 * f64::from(rate))).exp(),
        }
    }

    pub fn sample(&mut self, target: f64) -> f64 {
        let alpha = if target > self.value {
            self.rise
        } else {
            self.fall
        };
        self.value += (target - self.value) * alpha;
        self.value
    }

    pub fn reset(&mut self, value: f64) {
        self.value = value;
    }
}

pub(crate) fn wet_makeup(decay: f64, quality: f64, gate: f64, plate: bool) -> f64 {
    let (nominal, reference_decay, hq_gain) = if plate {
        (6.309573444801933, 3.0, 0.7079457843841379) // +16 dB; HQ correction -3 dB.
    } else {
        (3.1622776601683795, 7.5, 0.8413951416451951) // +10 dB; HQ correction -1.5 dB.
    };
    // Quality has no effect in gated mode. All inputs are existing smoothed
    // controls. Bound compensation to unity..four times nominal; no signal detector.
    let quality = quality * (1.0 - gate);
    let decay = decay * (1.0 - gate) + 0.18 * gate;
    (nominal * reference_decay / decay.max(0.18) * (1.0 + (hq_gain - 1.0) * quality))
        .clamp(1.0, nominal * 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_rises_gently_falls_quickly_and_reset_matches_fresh_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut ramp = MakeupRamp::new(rate, 1.0);
            let mut previous = 1.0;
            for _ in 0..rate / 2 {
                let value = ramp.sample(9.0);
                assert!((previous..=9.0).contains(&value));
                previous = value;
            }
            assert!((previous - (1.0 + 8.0 * (1.0 - (-1.0_f64).exp()))).abs() < 1e-8);
            ramp.reset(9.0);
            for _ in 0..rate / 200 {
                previous = ramp.sample(1.0);
                assert!((1.0..=9.0).contains(&previous));
            }
            // Integer sample rounding at 44.1 kHz differs by half a sample.
            assert!((previous - (1.0 + 8.0 * (-1.0_f64).exp())).abs() < 0.01);
            ramp.reset(3.0);
            assert_eq!(ramp.sample(3.0), MakeupRamp::new(rate, 3.0).sample(3.0));
        }
    }

    #[test]
    fn compensation_is_bounded_continuous_and_quality_independent_when_gated() {
        for plate in [false, true] {
            let maximum = wet_makeup(0.18, 0.0, 1.0, plate);
            for quality in [0.0, 0.25, 0.5, 1.0] {
                assert_eq!(wet_makeup(3.0, quality, 1.0, plate), maximum);
                let mut previous = wet_makeup(0.18, quality, 0.0, plate);
                for i in 1..60000 {
                    let gain = wet_makeup(0.18 + f64::from(i) * 0.001, quality, 0.0, plate);
                    assert!((1.0..=maximum).contains(&gain));
                    assert!(gain <= previous && previous - gain < previous * 0.002);
                    previous = gain;
                }
            }
        }
    }
}
