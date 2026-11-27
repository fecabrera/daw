//! Settled control-thread responses of the same filters used for audio.
use super::*;

pub fn response(settings: &EqSettings, rate: u32) -> [Vec<f32>; 3] {
    daw_dsp_support::filters::eq3::response(settings.bands.map(values), rate)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_matches_settled_audio_for_shelves_cuts_crossed_and_equal_boundaries() {
        for rate in [44_100, 48_000, 192_000] {
            for mask in 0..8 {
                for boundaries in [[250.0, 2500.0], [2500.0, 250.0], [1000.0, 1000.0]] {
                    let mut s = EqSettings::default();
                    for (i, b) in s.bands.iter_mut().enumerate() {
                        b.enabled = mask & (1 << i) != 0;
                        b.gain_db = [6.0, -9.0, 12.0][i];
                    }
                    s.bands[0].frequency_hz = boundaries[0];
                    s.bands[2].frequency_hz = boundaries[1];
                    let curves = response(&s, rate);
                    for bin in [35, 85, 130] {
                        let hz = 20.0 * 1000_f64.powf(bin as f64 / 159.0);
                        let mut eq = Equalizer::new(
                            ProcessingContext {
                                sample_rate_hz: rate,
                                max_block_frames: 512,
                            },
                            &s,
                        );
                        let mut input_power = 0.0;
                        let mut output_power = 0.0;
                        for i in 0..rate / 2 {
                            let input = (std::f64::consts::TAU * hz * f64::from(i)
                                / f64::from(rate))
                            .sin() as f32
                                * 0.01;
                            let output = eq.process_sample([input; 2])[0];
                            if i > rate / 4 {
                                input_power += f64::from(input).powi(2);
                                output_power += f64::from(output).powi(2);
                            }
                        }
                        let actual =
                            (10.0 * (output_power / input_power).max(1e-24).log10()).max(-144.0);
                        let expected = curves
                            .iter()
                            .map(|v| f64::from(v[bin]))
                            .sum::<f64>()
                            .max(-144.0);
                        assert!(
                            (actual - expected).abs() < 0.2,
                            "rate={rate} mask={mask} bounds={boundaries:?} hz={hz} actual={actual} graph={expected}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn spectrum_reports_audio_and_clears_on_bypass_and_reset() {
        use daw_unit_core::MeterStorage;
        let meter = std::sync::Arc::new(daw_unit_core::SpectrumMeter::default());
        let mut settings = EqSettings::default();
        let mut eq = Equalizer::new(
            ProcessingContext {
                sample_rate_hz: 48_000,
                max_block_frames: 512,
            },
            &settings,
        );
        eq.set_spectrum_meter(meter.clone());
        for i in 0..16000 {
            let v = (i as f32 * std::f32::consts::TAU * 100.0 / 48000.0).sin();
            eq.process_sample([v; 2]);
        }
        assert_ne!(meter.reading().valid_branches, 0);
        assert!(meter.reading().magnitudes.iter().any(|v| *v > 0.1));
        settings.enabled = false;
        eq.retarget(&settings);
        assert_eq!(meter.reading().valid_branches, 0);
        for _ in 0..5000 {
            eq.process_sample([1.0; 2]);
        }
        assert_eq!(meter.reading().valid_branches, 0);
        settings.enabled = true;
        eq.retarget(&settings);
        eq.reset();
        assert_eq!(meter.reading().valid_branches, 0);
        meter.clear();
    }
}
