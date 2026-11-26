//! Live-meter aggregation, reset epochs and displayed history.
use crate::Observation;
use audio_analysis::loudness::IntegratedStatistics;
use daw_unit_core::{LOUDNESS_HISTORY, LOUDNESS_SCALARS, LoudnessReading};
pub struct Statistics {
    pub epoch: u64,
    measurements: IntegratedStatistics,
    history: [[f32; LOUDNESS_HISTORY]; 2],
}
impl Statistics {
    pub fn new() -> Self {
        Self {
            epoch: 0,
            measurements: IntegratedStatistics::default(),
            history: [[f32::NEG_INFINITY; LOUDNESS_HISTORY]; 2],
        }
    }
    pub fn accept(&mut self, o: Observation) -> LoudnessReading {
        if self.epoch != o.epoch {
            self.epoch = o.epoch;
            self.measurements.reset();
            self.history.fill([f32::NEG_INFINITY; LOUDNESS_HISTORY]);
        }
        let result = self.measurements.accept(o.measurement, o.incomplete);
        if o.measurement.ticks.is_multiple_of(10) {
            for (h, v) in self
                .history
                .iter_mut()
                .zip([result.momentary, result.short_term])
            {
                h.rotate_left(1);
                h[LOUDNESS_HISTORY - 1] = v;
            }
        }
        let mut r = LoudnessReading::default();
        r.values[..LOUDNESS_SCALARS].copy_from_slice(&[
            result.momentary,
            result.short_term,
            result.integrated,
            result.max_momentary,
            result.max_short_term,
            result.max_true_peak,
            result.loudness_range,
            result.range_low,
            result.range_high,
            result.true_peak,
            result.peak_to_short_term,
            result.peak_to_integrated,
            o.measurement.seconds as f32,
            f32::from(result.incomplete),
        ]);
        for (c, h) in self.history.iter().enumerate() {
            r.values[LOUDNESS_SCALARS + c * LOUDNESS_HISTORY
                ..LOUDNESS_SCALARS + (c + 1) * LOUDNESS_HISTORY]
                .copy_from_slice(h);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn power(l: f64) -> f64 {
        10.0_f64.powf((l + 0.691) / 10.0)
    }
    fn observation(tick: u64, l: f64) -> Observation {
        Observation {
            epoch: 1,
            measurement: audio_analysis::loudness::Observation {
                momentary_power: power(l),
                short_term_power: power(l),
                max_momentary_power: power(l),
                max_short_term_power: power(l),
                true_peak: 0.2,
                seconds: tick as f64 / 10.0,
                ticks: tick,
            },
            incomplete: false,
        }
    }
    #[test]
    fn gates_exclude_absolute_and_relative_silence_without_unbounded_storage() {
        let mut s = Statistics::new();
        for t in 1..=100 {
            s.accept(observation(t, -23.0));
        }
        for t in 101..=200 {
            s.accept(observation(t, -80.0));
        }
        let r = s.accept(observation(201, -50.0));
        assert!((r.values[2] + 23.0).abs() < 0.01, "{}", r.values[2]);
    }
    #[test]
    fn range_uses_ten_and_ninety_five_percentiles_and_history_rolls_at_one_hertz() {
        let mut s = Statistics::new();
        let mut r = Default::default();
        for t in 1..=2000 {
            r = s.accept(observation(t, if t <= 1000 { -30.0 } else { -20.0 }));
        }
        assert!((r.values[6] - 10.0).abs() < 0.02);
        assert!((r.values[7] + 30.0).abs() < 0.02);
        assert!((r.values[8] + 20.0).abs() < 0.02);
        assert!((r.values[LOUDNESS_SCALARS] + 30.0).abs() < 0.02);
        assert!((r.values[LOUDNESS_SCALARS + LOUDNESS_HISTORY - 1] + 20.0).abs() < 0.02);
        let mut o = observation(1, -40.0);
        o.epoch = 2;
        let r = s.accept(o);
        assert!(r.values[2].is_infinite());
        assert!(r.values[LOUDNESS_SCALARS..].iter().all(|v| v.is_infinite()));
    }
}
