//! Warm parallel reverb filters with bounded, tail-preserving slope transitions.
use daw_filter::svf::StateVariableFilter;
use daw_unit_core::{ModulationRamp, ReverbFilterSlope as Slope};

#[derive(Clone)]
pub(crate) struct SlopeFilter {
    filters: [[StateVariableFilter; 3]; 2],
    first: [f64; 2],
    first_alpha: f64,
    first_configured: Option<(f64, f64)>,
    highpass: bool,
    weights: [ModulationRamp; 3],
    current: [f32; 3],
    slope: Slope,
    transition_frames: u32,
}
impl SlopeFilter {
    pub fn new(rate: u32, slope: Slope, highpass: bool) -> Self {
        Self {
            filters: std::array::from_fn(|_| std::array::from_fn(|_| Default::default())),
            first: [0.0; 2],
            first_alpha: 0.0,
            first_configured: None,
            highpass,
            weights: Slope::ALL.map(|s| ModulationRamp::new(f32::from(s == slope))),
            current: Slope::ALL.map(|s| f32::from(s == slope)),
            slope,
            transition_frames: (rate / 200).max(1),
        }
    }
    pub fn retarget(&mut self, slope: Slope) {
        if self.slope == slope {
            return;
        }
        // Restart all weights together. Unchanged targets must use the same remaining
        // duration as changed targets when another edit interrupts an active fade.
        self.weights = self.current.map(ModulationRamp::new);
        for (weight, candidate) in self.weights.iter_mut().zip(Slope::ALL) {
            weight.set(f32::from(candidate == slope), self.transition_frames);
        }
        self.slope = slope;
    }
    pub fn weights(&mut self) -> [f64; 3] {
        self.current = self.weights.each_mut().map(ModulationRamp::next_value);
        self.current.map(f64::from)
    }
    pub fn sample(
        &mut self,
        channel: usize,
        input: f64,
        legacy: f64,
        cutoff: f64,
        rate: f64,
        weights: [f64; 3],
    ) -> f64 {
        let highpass = self.highpass;
        let run = |f: &mut StateVariableFilter, input, q| {
            if highpass {
                f.high(input, cutoff, q, rate)
            } else {
                f.sample(input, cutoff, q, rate, [1.0, 0.0, 0.0, 0.0])
            }
        };
        let f = &mut self.filters[channel];
        let second = run(&mut f[0], input, std::f64::consts::FRAC_1_SQRT_2);
        // Fourth-order Butterworth poles: two distinct Q values retain -3 dB at cutoff.
        let fourth = run(&mut f[1], input, 0.541196100146197);
        let fourth = run(&mut f[2], fourth, 1.306562964876377);
        legacy * weights[0] + second * weights[1] + fourth * weights[2]
    }
    /// Plate's first-order path uses a bilinear one-pole at the same cutoff as the SVFs.
    pub fn sample_input(
        &mut self,
        channel: usize,
        input: f64,
        cutoff: f64,
        rate: f64,
        weights: [f64; 3],
    ) -> f64 {
        if self.first_configured != Some((cutoff, rate)) {
            let g = (std::f64::consts::PI * cutoff / rate).tan();
            self.first_alpha = g / (1.0 + g);
            self.first_configured = Some((cutoff, rate));
        }
        let delta = (input - self.first[channel]) * self.first_alpha;
        let low = self.first[channel] + delta;
        self.first[channel] = low + delta;
        let first = if self.highpass { input - low } else { low };
        self.sample(channel, input, first, cutoff, rate, weights)
    }
    pub fn reset(&mut self) {
        self.first = [0.0; 2];
        for filter in self.filters.iter_mut().flatten() {
            filter.reset();
        }
        for weight in &mut self.weights {
            weight.reset();
        }
        self.current = Slope::ALL.map(|s| f32::from(s == self.slope));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn magnitude(rate: u32, frequency: f64, slope: Slope) -> f64 {
        let mut hp = SlopeFilter::new(rate, slope, true);
        let cutoff = 70.0;
        let c = 1.0 - (-std::f64::consts::TAU * cutoff / f64::from(rate)).exp();
        let mut low = 0.0;
        let mut input_energy = 0.0;
        let mut output_energy = 0.0;
        for i in 0..rate * 2 {
            let x = (std::f64::consts::TAU * frequency * f64::from(i) / f64::from(rate)).sin();
            low += c * (x - low);
            let weights = hp.weights();
            let y = hp.sample(0, x, x - low, cutoff, f64::from(rate), weights);
            if i >= rate {
                input_energy += x * x;
                output_energy += y * y;
            }
        }
        20.0 * (output_energy / input_energy).sqrt().log10()
    }
    #[test]
    fn slopes_have_matched_cutoffs_and_expected_low_frequency_attenuation() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for (slope, order) in Slope::ALL.into_iter().zip([1, 2, 4]) {
                assert!((magnitude(rate, 70.0, slope) + 3.0103).abs() < 0.06);
                let expected = -10.0 * (1.0 + 4.0_f64.powi(2 * order)).log10();
                assert!((magnitude(rate, 17.5, slope) - expected).abs() < 0.06);
            }
        }
    }
    #[test]
    fn plate_high_and_low_slopes_match_butterworth_responses_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let rate_f = f64::from(rate);
            for highpass in [false, true] {
                for (slope, order) in Slope::ALL.into_iter().zip([1, 2, 4]) {
                    for frequency in [250.0, 1000.0, 4000.0] {
                        let mut filter = SlopeFilter::new(rate, slope, highpass);
                        let mut input_energy = 0.0;
                        let mut output_energy = 0.0;
                        for i in 0..rate / 5 {
                            let x =
                                (std::f64::consts::TAU * frequency * f64::from(i) / rate_f).sin();
                            let weights = filter.weights();
                            let y = filter.sample_input(0, x, 1000.0, rate_f, weights);
                            if i >= rate / 10 {
                                input_energy += x * x;
                                output_energy += y * y;
                            }
                        }
                        let ratio = (std::f64::consts::PI * frequency / rate_f).tan()
                            / (std::f64::consts::PI * 1000.0 / rate_f).tan();
                        let ratio = if highpass { 1.0 / ratio } else { ratio };
                        let expected = -10.0 * (1.0 + ratio.powi(2 * order)).log10();
                        let measured = 10.0 * (output_energy / input_energy).log10();
                        assert!(
                            (measured - expected).abs() < 0.03,
                            "rate={rate} hp={highpass} slope={slope:?} f={frequency}: {measured} vs {expected}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn transitions_keep_unity_weights_and_reset_selects_the_target() {
        let mut hp = SlopeFilter::new(48000, Slope::Db6, true);
        for slope in [Slope::Db24, Slope::Db12, Slope::Db6] {
            hp.retarget(slope);
            for _ in 0..37 {
                let weights = hp.weights();
                assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-5);
                assert!(weights.iter().all(|w| (0.0..=1.0).contains(w)));
            }
        }
        hp.retarget(Slope::Db24);
        for _ in 0..240 {
            hp.weights();
        }
        assert_eq!(hp.weights(), [0.0, 0.0, 1.0]);
        hp.retarget(Slope::Db12);
        hp.reset();
        assert_eq!(hp.weights(), [0.0, 1.0, 0.0]);
    }
}
