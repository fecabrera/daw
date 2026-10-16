use super::*;
use daw_unit_core::{DriveColorShape, SILENT_GAIN_DB};
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn unit(rate: u32, settings: &DriveSettings) -> Drive {
    Drive::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        settings,
    )
}
fn clean() -> DriveSettings {
    DriveSettings {
        gain: 0.0,
        ..Default::default()
    }
}
fn response(rate: u32, settings: &DriveSettings, hz: f32) -> f32 {
    let mut drive = unit(rate, settings);
    let mut input_power = 0.0_f64;
    let mut output_power = 0.0_f64;
    for frame in 0..rate / 2 {
        let input = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.001;
        let output = drive.process_sample([input, 0.0]);
        assert!(output[0].is_finite());
        assert_eq!(output[1], 0.0, "channel state must be independent");
        if frame >= rate / 4 {
            input_power += f64::from(input).powi(2);
            output_power += f64::from(output[0]).powi(2);
        }
    }
    (output_power / input_power).sqrt() as f32
}
#[test]
fn color_shapes_have_the_expected_response_at_every_processing_rate() {
    for rate in RATES {
        let mut settings = DriveSettings {
            color_strength: 1.0,
            ..clean()
        };
        assert!((response(rate, &settings, 750.0) - 1.0).abs() < 1e-4);
        settings.color_shape = DriveColorShape::MidScoop;
        assert!(response(rate, &settings, 750.0) < 0.27);
        settings.color_shape = DriveColorShape::MidHump;
        assert!(response(rate, &settings, 750.0) > 3.9);
        let bass = response(rate, &settings, 40.0);
        settings.color_shape = DriveColorShape::MidHumpBassCut;
        assert!(response(rate, &settings, 40.0) < bass * 0.15);
        assert!(response(rate, &settings, 750.0) > 2.0);
        for shape in DriveColorShape::ALL {
            settings.color_shape = shape;
            settings.color_strength = 0.0;
            for hz in [40.0, 750.0, 8000.0] {
                assert!((response(rate, &settings, hz) - 1.0).abs() < 1e-4);
            }
        }
    }
}
#[test]
fn color_frequency_moves_the_mid_shape_and_linked_bass_cut() {
    let mut settings = DriveSettings {
        color_shape: DriveColorShape::MidHumpBassCut,
        color_strength: 1.0,
        color_frequency_hz: 250.0,
        ..clean()
    };
    let low_mid = response(48000, &settings, 250.0);
    let low_bass = response(48000, &settings, 400.0);
    settings.color_frequency_hz = 2000.0;
    assert!(response(48000, &settings, 250.0) < low_mid * 0.3);
    assert!(response(48000, &settings, 400.0) < low_bass * 0.3);
    assert!(response(48000, &settings, 2000.0) > 2.0);
}
#[test]
fn gain_adds_harmonics_and_volume_scales_the_processed_output() {
    for rate in RATES {
        let mut clean_drive = unit(rate, &clean());
        let settings = DriveSettings {
            gain: 1.0,
            ..Default::default()
        };
        let mut driven = unit(rate, &settings);
        let mut quiet = unit(
            rate,
            &DriveSettings {
                volume_db: -6.0,
                ..settings.clone()
            },
        );
        let mut silent = unit(
            rate,
            &DriveSettings {
                volume_db: SILENT_GAIN_DB,
                ..settings
            },
        );
        let mut third = 0.0_f64;
        for frame in 0..rate {
            let phase = frame as f32 * 250.0 * std::f32::consts::TAU / rate as f32;
            let input = [phase.sin() * 0.2, -phase.sin() * 0.1];
            let dry = clean_drive.process_sample(input);
            let wet = driven.process_sample(input);
            let scaled = quiet.process_sample(input);
            assert_eq!(silent.process_sample(input), [0.0; 2]);
            for ch in 0..2 {
                assert!((dry[ch] - input[ch]).abs() < 1e-6);
                assert!(wet[ch].abs() <= 1.00001);
                assert!((scaled[ch] - wet[ch] * amplitude(-6.0)).abs() < 1e-6);
            }
            third += f64::from(wet[0] * (phase * 3.0).sin());
        }
        assert!((third / f64::from(rate)).abs() > 0.1);
    }
}
#[test]
fn tone_has_a_neutral_center_and_dark_to_bright_response() {
    let mut settings = clean();
    let neutral = response(48000, &settings, 10000.0);
    settings.tone = 0.0;
    let dark = response(48000, &settings, 10000.0);
    settings.tone = 1.0;
    let bright = response(48000, &settings, 10000.0);
    assert!((neutral - 1.0).abs() < 1e-4);
    assert!(dark < 0.3 && bright > 3.5);
}
#[test]
fn bypass_and_parameter_updates_finish_in_five_ms_and_reset_is_deterministic() {
    for rate in RATES {
        let mut settings = DriveSettings::default();
        let mut drive = unit(rate, &settings);
        for _ in 0..100 {
            drive.process_sample([0.2, -0.1]);
        }
        settings.gain = 1.0;
        settings.color_shape = DriveColorShape::MidHumpBassCut;
        settings.color_frequency_hz = 2000.0;
        settings.tone = 0.0;
        settings.enabled = false;
        drive.retarget(&settings);
        for _ in 0..rate / 200 {
            drive.process_sample([0.2, -0.1]);
        }
        assert!(
            drive
                .ramps()
                .all(|ramp| ramp.remaining == 0 && ramp.value == ramp.target)
        );
        assert_eq!(drive.process_sample([0.2, -0.1]), [0.2, -0.1]);
        settings.enabled = true;
        drive.retarget(&settings);
        drive.reset();
        let mut fresh = unit(rate, &settings);
        for frame in 0..1000 {
            let input = [(frame as f32 * 0.3).sin(), (frame as f32 * 0.1).cos()];
            assert_eq!(drive.process_sample(input), fresh.process_sample(input));
        }
    }
}
#[test]
fn presentation_and_unrelated_edits_preserve_in_flight_ramps() {
    let mut settings = DriveSettings::default();
    let mut drive = unit(48000, &settings);
    settings.gain = 1.0;
    drive.retarget(&settings);
    for _ in 0..60 {
        drive.process_sample([0.1; 2]);
    }
    let remaining = drive.gain.remaining;
    settings.collapsed = true;
    settings.volume_db = -6.0;
    drive.retarget(&settings);
    assert_eq!(drive.gain.remaining, remaining);
    for _ in 0..remaining {
        drive.process_sample([0.1; 2]);
    }
    assert_eq!(drive.gain.value, 1.0);
    assert!(drive.volume.remaining > 0);
}
#[test]
fn maximum_controls_remain_finite_with_high_level_stereo_input() {
    for rate in RATES {
        for shape in DriveColorShape::ALL {
            for hz in [250.0, 2000.0] {
                let mut drive = unit(
                    rate,
                    &DriveSettings {
                        gain: 1.0,
                        tone: 1.0,
                        volume_db: 12.0,
                        color_strength: 1.0,
                        color_shape: shape,
                        color_frequency_hz: hz,
                        ..Default::default()
                    },
                );
                for frame in 0..4096 {
                    let input = if frame % 2 == 0 { 1000.0 } else { -1000.0 };
                    assert!(
                        drive
                            .process_sample([input, input * 0.25])
                            .iter()
                            .all(|sample| sample.is_finite())
                    );
                }
            }
        }
    }
}

#[test]
fn modulation_matches_static_settings_after_smoothing_and_preserves_neutral_audio() {
    for rate in RATES {
        let settings = DriveSettings {
            color_shape: DriveColorShape::MidHumpBassCut,
            ..Default::default()
        };
        let mut neutral = unit(rate, &settings);
        let mut legacy = unit(rate, &settings);
        for frame in 0..1000 {
            let input = [(frame as f32 * 0.1).sin() * 0.01, 0.0];
            assert_eq!(
                neutral.process_modulated(input, [0.0; 5]),
                legacy.process_sample(input)
            );
        }
        let target = DriveSettings {
            volume_db: -6.0,
            gain: 0.7,
            tone: 0.8,
            color_strength: 0.9,
            color_frequency_hz: 1500.0,
            ..settings.clone()
        };
        let mut modulated = unit(rate, &settings);
        let mut fixed = unit(rate, &target);
        let offsets = [-6.0, 0.5, 0.3, 0.4, 1.0];
        for _ in 0..rate / 20 {
            modulated.process_modulated([0.0; 2], offsets);
            fixed.process_sample([0.0; 2]);
        }
        let actual = modulated.effective_parameters();
        for (actual, expected) in actual.into_iter().zip([-6.0, 0.7, 0.8, 0.9, 1500.0]) {
            assert!(
                (actual - expected).abs() < 0.01,
                "{rate}: {actual} != {expected}"
            );
        }
        for frame in 0..1000 {
            let input = [(frame as f32 * 0.1).sin() * 0.01, 0.0];
            let a = modulated.process_modulated(input, offsets);
            let b = fixed.process_sample(input);
            assert!((a[0] - b[0]).abs() < 2e-5, "{rate}: {a:?} {b:?}");
            assert_eq!(a[1], 0.0);
        }
        modulated.reset();
        assert_eq!(
            modulated.process_sample([0.2; 2]),
            unit(rate, &settings).process_sample([0.2; 2])
        );
        for _ in 0..rate / 50 {
            assert!(
                modulated
                    .process_modulated([0.1; 2], [1000.0; 5])
                    .iter()
                    .all(|s| s.is_finite())
            );
        }
        let values = modulated.effective_parameters();
        assert!((values[0] - 12.0).abs() < 1e-5);
        assert_eq!(&values[1..4], &[1.0; 3]);
        assert!((values[4] - 2000.0).abs() < 0.01);
        for _ in 0..1000 {
            assert!(
                modulated
                    .process_modulated(
                        [0.0; 2],
                        [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.0]
                    )
                    .iter()
                    .all(|s| s.is_finite())
            );
        }
    }
}
