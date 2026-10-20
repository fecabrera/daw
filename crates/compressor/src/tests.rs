use super::*;

const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn unit(rate: u32, settings: &CompressorSettings) -> Compressor {
    Compressor::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        settings,
    )
}
fn settle(unit: &mut Compressor, rate: u32, input: [f32; 2]) -> [f32; 2] {
    let mut output = [0.0; 2];
    for _ in 0..rate / 2 {
        output = unit.process_sample(input);
    }
    output
}

#[test]
fn held_reduction_captures_every_sample_without_changing_audio_and_resets_on_bypass() {
    for rate in RATES {
        let mut settings = CompressorSettings {
            threshold_db: -36.0,
            attack_ms: 0.1,
            release_ms: 10.0,
            ..Default::default()
        };
        let mut monitored = unit(rate, &settings);
        let mut reference = unit(rate, &settings);
        let meter = Arc::new(PeakMeter::default());
        monitored.set_gain_reduction_peak_meter(meter.clone());
        let mut peak = 0.0_f32;
        for frame in 0..12_000 {
            let input = if frame < 2000 { [1.0, -0.25] } else { [0.0; 2] };
            assert_eq!(
                monitored.process_sample(input),
                reference.process_sample(input)
            );
            peak = peak.max(monitored.gain_reduction_db());
        }
        assert_eq!(meter.peak(), peak);
        assert!(peak > 20.0);
        assert!(meter.reading() < peak * 0.1);
        meter.reset_peak();
        let current = monitored.gain_reduction_db();
        assert_eq!(meter.reading(), current);
        monitored.process_sample([1.0, -0.25]);
        assert_eq!(meter.peak(), monitored.gain_reduction_db());
        settings.enabled = false;
        monitored.retarget(&settings);
        assert_eq!(meter.peak(), 0.0);
        for _ in 0..1000 {
            monitored.process_sample([1.0; 2]);
        }
        assert_eq!(meter.peak(), 0.0);
        settings.enabled = true;
        monitored.retarget(&settings);
        for _ in 0..1000 {
            monitored.process_sample([1.0; 2]);
        }
        assert!(meter.peak() > 0.0);
    }
}

#[test]
fn hard_and_soft_knees_join_continuously_and_follow_the_ratio() {
    let slope = 0.75;
    assert_eq!(reduction(-30.0, -18.0, slope, 0.0), 0.0);
    assert_eq!(reduction(-18.0, -18.0, slope, 0.0), 0.0);
    assert_eq!(reduction(0.0, -18.0, slope, 0.0), 13.5);
    assert!((reduction(-18.0, -18.0, slope, 6.0) - 0.5625).abs() < 1e-6);
    for boundary in [-21.0, -15.0] {
        let below = reduction(boundary - 0.0001, -18.0, slope, 6.0);
        let above = reduction(boundary + 0.0001, -18.0, slope, 6.0);
        assert!((above - below).abs() < 0.001);
    }
    let mut previous = 0.0;
    for level in -600..=200 {
        let current = reduction(level as f32 * 0.1, -18.0, slope, 24.0);
        assert!(current >= previous);
        previous = current;
    }
}

#[test]
fn linked_stereo_and_steady_gain_match_at_all_rates() {
    for rate in RATES {
        for detection in [CompressorDetection::Peak, CompressorDetection::Rms] {
            let settings = CompressorSettings {
                detection,
                knee_db: 0.0,
                ..Default::default()
            };
            let mut compressor = unit(rate, &settings);
            let result = settle(&mut compressor, rate, [1.0, -0.25]);
            assert!(
                (result[0] - amplitude(-13.5)).abs() < 1e-4,
                "{rate}, {result:?}"
            );
            assert_eq!(result[1], -result[0] * 0.25);
            assert!((compressor.gain_reduction_db() - 13.5).abs() < 0.001);
            let below = settle(&mut compressor, rate, [0.01, 0.0]);
            // Release is exponential; a half second leaves less than 0.1 dB of reduction.
            assert!(below[0] > 0.0098 && below[1] == 0.0);
        }
    }
}

#[test]
fn attack_and_release_are_time_constants_in_db_at_every_rate() {
    for rate in RATES {
        let settings = CompressorSettings {
            knee_db: 0.0,
            ..Default::default()
        };
        let mut compressor = unit(rate, &settings);
        let attack_frames = (rate as f64 * 0.01).round() as u32;
        for _ in 0..attack_frames {
            compressor.process_sample([1.0, 0.0]);
        }
        let expected = 13.5 * (1.0 - (-f64::from(attack_frames) / (f64::from(rate) * 0.01)).exp());
        assert!((f64::from(compressor.gain_reduction_db()) - expected).abs() < 0.001);
        let before = compressor.gain_reduction_db();
        for _ in 0..rate / 10 {
            compressor.process_sample([0.0; 2]);
        }
        assert!((compressor.gain_reduction_db() - before / std::f32::consts::E).abs() < 0.001);
    }
}

#[test]
fn rms_rejects_a_short_peak_and_both_modes_settle_on_dc() {
    for rate in RATES {
        let mut peak = unit(rate, &CompressorSettings::default());
        let mut rms = unit(
            rate,
            &CompressorSettings {
                detection: CompressorDetection::Rms,
                ..Default::default()
            },
        );
        peak.process_sample([1.0; 2]);
        rms.process_sample([1.0; 2]);
        assert!(peak.gain_reduction_db() > 0.0);
        assert_eq!(rms.gain_reduction_db(), 0.0);
        let p = settle(&mut peak, rate, [1.0; 2]);
        let r = settle(&mut rms, rate, [1.0; 2]);
        assert!((p[0] - r[0]).abs() < 0.001);
    }
}

#[test]
fn bypass_dry_wet_and_final_level_have_exact_endpoints() {
    for rate in RATES {
        let input = [0.5, -0.25];
        let mut bypass = unit(
            rate,
            &CompressorSettings {
                enabled: false,
                level_db: daw_unit_core::SILENT_GAIN_DB,
                ..Default::default()
            },
        );
        assert_eq!(settle(&mut bypass, rate, input), input);
        assert_eq!(bypass.gain_reduction_db(), 0.0);
        let mut dry = unit(
            rate,
            &CompressorSettings {
                dry_wet: 0.0,
                level_db: 6.0,
                ..Default::default()
            },
        );
        assert_eq!(
            settle(&mut dry, rate, input),
            input.map(|value| value * amplitude(6.0))
        );
        let mut neutral = unit(
            rate,
            &CompressorSettings {
                ratio: 1.0,
                ..Default::default()
            },
        );
        assert_eq!(settle(&mut neutral, rate, input), input);
        let mut silent = unit(
            rate,
            &CompressorSettings {
                dry_wet: 0.0,
                level_db: daw_unit_core::SILENT_GAIN_DB,
                ..Default::default()
            },
        );
        assert_eq!(silent.process_sample(input), [0.0; 2]);
        let mut wet = unit(rate, &CompressorSettings::default());
        let compressed = settle(&mut wet, rate, input);
        let mut mixed = unit(
            rate,
            &CompressorSettings {
                dry_wet: 0.25,
                ..Default::default()
            },
        );
        let output = settle(&mut mixed, rate, input);
        for channel in 0..2 {
            assert!(
                (output[channel] - (input[channel] * 0.75 + compressed[channel] * 0.25)).abs()
                    < 1e-6
            );
        }
    }
}

#[test]
fn compatible_edits_keep_histories_mode_transitions_are_smooth_and_reset_clears_meter() {
    let rate = 48000;
    let mut settings = CompressorSettings::default();
    let mut current = unit(rate, &settings);
    let mut reference = unit(rate, &settings);
    let meter = Arc::new(AtomicU32::new(0));
    current.set_gain_reduction_meter(meter.clone());
    settle(&mut current, rate, [1.0, 0.5]);
    settle(&mut reference, rate, [1.0, 0.5]);
    settings.collapsed = true;
    current.retarget(&settings);
    for _ in 0..100 {
        assert_eq!(
            current.process_sample([0.1, 0.05]),
            reference.process_sample([0.1, 0.05])
        );
    }
    settings.detection = CompressorDetection::Rms;
    settings.threshold_db = -36.0;
    current.retarget(&settings);
    let mut previous = current.process_sample([0.1, 0.05]);
    for _ in 0..1000 {
        let output = current.process_sample([0.1, 0.05]);
        assert!((output[0] - previous[0]).abs() < 0.01);
        assert_eq!(
            f32::from_bits(meter.load(Ordering::Relaxed)),
            current.gain_reduction_db()
        );
        previous = output;
    }
    current.reset();
    assert_eq!(meter.load(Ordering::Relaxed), 0);
    let mut fresh = unit(rate, &settings);
    for _ in 0..1000 {
        assert_eq!(
            current.process_sample([0.3, 0.1]),
            fresh.process_sample([0.3, 0.1])
        );
    }
}

#[test]
fn extreme_parameters_and_nonfinite_samples_do_not_produce_nonfinite_outputs() {
    for rate in RATES {
        for detection in [CompressorDetection::Peak, CompressorDetection::Rms] {
            let mut settings = CompressorSettings {
                detection,
                threshold_db: -60.0,
                ratio: 20.0,
                attack_ms: 0.1,
                release_ms: 2000.0,
                knee_db: 24.0,
                level_db: 12.0,
                ..Default::default()
            };
            let mut compressor = unit(rate, &settings);
            for index in 0..1000 {
                let sample = [f32::MAX, -f32::MAX, f32::NAN, f32::INFINITY, 0.0][index % 5];
                assert!(
                    compressor
                        .process_sample([sample, -sample])
                        .iter()
                        .all(|value| value.is_finite())
                );
                assert!(compressor.gain_reduction_db().is_finite());
                if index % 100 == 0 {
                    settings.enabled = !settings.enabled;
                    compressor.retarget(&settings);
                }
            }
        }
    }
}
