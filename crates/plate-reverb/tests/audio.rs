use daw_plate_reverb::PlateReverb;
use daw_unit_core::{EffectUnit, PlateReverbSettings as Settings, ProcessingContext};
fn unit(rate: u32, s: &Settings) -> PlateReverb {
    PlateReverb::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 4096,
        },
        s,
    )
}
#[test]
fn finite_stereo_wet_tail_and_repeatable_reset_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let s = Settings {
            dry_wet: 1.0,
            ..Default::default()
        };
        let mut u = unit(rate, &s);
        let mut reference = Vec::new();
        for i in 0..8192 {
            let y = u.process_sample(if i == 0 { [0.1, 0.0] } else { [0.0; 2] });
            assert!(y.iter().all(|x| x.is_finite()));
            reference.push(y);
        }
        assert!(reference[1024..].iter().any(|y| y[0] != 0.0 || y[1] != 0.0));
        u.reset();
        for (i, expected) in reference.into_iter().enumerate() {
            assert_eq!(
                u.process_sample(if i == 0 { [0.1, 0.0] } else { [0.0; 2] }),
                expected
            );
        }
    }
}
#[test]
fn exact_bypass_silence_and_live_retarget_keeps_tail() {
    let mut s = Settings::default();
    let mut u = unit(48000, &s);
    for _ in 0..2048 {
        u.process_sample([0.1, -0.05]);
    }
    s.level_db = daw_unit_core::SILENT_GAIN_DB;
    u.retarget(&s);
    for _ in 0..512 {
        u.process_sample([0.0; 2]);
    }
    assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
    s.enabled = false;
    u.retarget(&s);
    for _ in 0..512 {
        u.process_sample([0.2, -0.4]);
    }
    assert_eq!(u.process_sample([0.2, -0.4]), [0.2, -0.4]);
}
#[test]
fn gated_quality_is_independent() {
    let s = Settings {
        gated: true,
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut a = unit(48000, &s);
    let mut b = unit(48000, &Settings { hq: true, ..s });
    for i in 0..8192 {
        let input = if i == 0 { [0.1, 0.0] } else { [0.0; 2] };
        assert_eq!(a.process_sample(input), b.process_sample(input));
    }
}

#[test]
fn longer_decay_retains_more_late_energy() {
    let mut short = unit(
        48000,
        &Settings {
            decay_seconds: 0.3,
            dry_wet: 1.0,
            ..Default::default()
        },
    );
    let mut long = unit(
        48000,
        &Settings {
            decay_seconds: 15.0,
            dry_wet: 1.0,
            ..Default::default()
        },
    );
    let mut energy = [0.0_f64; 2];
    for i in 0..192000 {
        let x = if i == 0 { [0.1, 0.0] } else { [0.0; 2] };
        let a = short.process_sample(x);
        let b = long.process_sample(x);
        if i > 96000 {
            energy[0] += a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>();
            energy[1] += b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>();
        }
    }
    assert!(
        energy[1] > energy[0] * 4.0 && energy[1] > 1e-20,
        "late energies: {energy:?}"
    );
}

#[test]
fn filter_power_is_independent_and_bypassed_frequencies_do_not_change_audio() {
    let base = Settings {
        highpass_hz: 1000.0,
        lowpass_hz: 500.0,
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut outputs = Vec::new();
    for highpass_enabled in [false, true] {
        for lowpass_enabled in [false, true] {
            let mut u = unit(
                48000,
                &Settings {
                    highpass_enabled,
                    lowpass_enabled,
                    ..base.clone()
                },
            );
            outputs.push(
                (0..8192)
                    .map(|i| u.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] }))
                    .collect::<Vec<_>>(),
            );
        }
    }
    for i in 0..outputs.len() {
        for j in 0..i {
            assert_ne!(
                outputs[i], outputs[j],
                "each filter must affect the wet input independently"
            );
        }
    }
    let mut bypassed = unit(
        48000,
        &Settings {
            highpass_enabled: false,
            lowpass_enabled: false,
            ..Default::default()
        },
    );
    let mut other = unit(
        48000,
        &Settings {
            highpass_enabled: false,
            lowpass_enabled: false,
            highpass_hz: 1000.0,
            lowpass_hz: 500.0,
            highpass_slope: daw_unit_core::ReverbFilterSlope::Db6,
            lowpass_slope: daw_unit_core::ReverbFilterSlope::Db24,
            ..Default::default()
        },
    );
    for i in 0..8192 {
        let x = if i == 0 { [0.2, -0.1] } else { [0.0; 2] };
        assert_eq!(bypassed.process_sample(x), other.process_sample(x));
    }
}

#[test]
fn filter_power_edits_preserve_the_existing_tail() {
    let mut settings = Settings {
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut warm = unit(48000, &settings);
    for i in 0..4096 {
        warm.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] });
    }
    let mut baseline = warm.clone();
    let expected: Vec<_> = (0..512)
        .map(|_| baseline.process_sample([0.0; 2]))
        .collect();
    assert!(expected.iter().flatten().any(|v| v.abs() > 1e-8));
    for (high, low) in [(false, true), (true, false), (false, false)] {
        let mut edited = warm.clone();
        settings.highpass_enabled = high;
        settings.lowpass_enabled = low;
        edited.retarget(&settings);
        for sample in &expected {
            assert_eq!(edited.process_sample([0.0; 2]), *sample);
        }
    }
}

#[test]
fn each_filter_slope_changes_wet_audio_and_preserves_the_existing_tail() {
    let settings = Settings {
        highpass_hz: 1000.0,
        lowpass_hz: 500.0,
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut outputs = Vec::new();
    for highpass_slope in daw_unit_core::ReverbFilterSlope::ALL {
        for lowpass_slope in daw_unit_core::ReverbFilterSlope::ALL {
            let mut u = unit(
                48000,
                &Settings {
                    highpass_slope,
                    lowpass_slope,
                    ..settings.clone()
                },
            );
            outputs.push(
                (0..8192)
                    .map(|i| u.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] }))
                    .collect::<Vec<_>>(),
            );
        }
    }
    for i in 0..outputs.len() {
        for j in 0..i {
            assert_ne!(outputs[i], outputs[j]);
        }
    }
    let mut warm = unit(48000, &settings);
    for i in 0..4096 {
        warm.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] });
    }
    let mut baseline = warm.clone();
    let expected: Vec<_> = (0..512)
        .map(|_| baseline.process_sample([0.0; 2]))
        .collect();
    assert!(expected.iter().flatten().any(|v| v.abs() > 1e-8));
    for highpass_slope in daw_unit_core::ReverbFilterSlope::ALL {
        for lowpass_slope in daw_unit_core::ReverbFilterSlope::ALL {
            let mut edited = warm.clone();
            edited.retarget(&Settings {
                highpass_slope,
                lowpass_slope,
                ..settings.clone()
            });
            for sample in &expected {
                assert_eq!(edited.process_sample([0.0; 2]), *sample);
            }
        }
    }
}

#[test]
fn drive_only_processes_wet_audio_at_all_rates_and_mix_positions() {
    use daw_unit_core::PlateDriveMode as Mode;
    let input = |i: usize| {
        [
            ((i as f32) * 0.17).sin() * 0.8,
            ((i as f32) * 0.13).cos() * 0.6,
        ]
    };
    let [dry_gain, wet_gain] = daw_reverb_core::mix_gains(0.5, true);
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut baseline = unit(
            rate,
            &Settings {
                dry_wet: 1.0,
                ..Default::default()
            },
        );
        let clean_wet: Vec<_> = (0..4096)
            .map(|i| baseline.process_sample(input(i)))
            .collect();
        let mut outputs = Vec::new();
        for drive_mode in Mode::ALL {
            let settings = Settings {
                drive_enabled: true,
                drive_mode,
                drive_db: 12.0,
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut driven = unit(rate, &settings);
            let mut mixed = unit(
                rate,
                &Settings {
                    dry_wet: 0.5,
                    ..settings.clone()
                },
            );
            let mut dry = unit(
                rate,
                &Settings {
                    dry_wet: 0.0,
                    ..settings
                },
            );
            let mut wet_output = Vec::new();
            for i in 0..4096 {
                let x = input(i);
                let wet = driven.process_sample(x);
                let mix = mixed.process_sample(x);
                assert_eq!(
                    dry.process_sample(x),
                    x,
                    "Drive must not alter the dry signal"
                );
                for ch in 0..2 {
                    let expected = f64::from(x[ch]) * dry_gain + f64::from(wet[ch]) * wet_gain;
                    assert!(
                        (f64::from(mix[ch]) - expected).abs() < 1e-6,
                        "rate={rate} mode={drive_mode:?}"
                    );
                }
                wet_output.push(wet);
            }
            assert_ne!(wet_output, clean_wet, "Drive must process the wet branch");
            outputs.push(wet_output);
        }
        assert_ne!(outputs[0], outputs[1]);
        assert_ne!(outputs[1], outputs[2]);
        assert_ne!(outputs[0], outputs[2]);
    }
}

#[test]
fn dry_audio_survives_drive_amount_mode_and_power_changes() {
    let mut settings = Settings {
        drive_enabled: true,
        drive_db: 12.0,
        dry_wet: 0.0,
        ..Default::default()
    };
    let mut u = unit(48000, &settings);
    for i in 0..2048 {
        if i % 37 == 0 {
            settings.drive_mode = daw_unit_core::PlateDriveMode::ALL[(i / 37) % 3];
            settings.drive_enabled = i % 2 == 0;
            settings.drive_db = if i % 2 == 0 { 0.0 } else { 12.0 };
            settings.drive_tone = if i % 2 == 0 { 0.0 } else { 1.0 };
            u.retarget(&settings);
        }
        assert_eq!(u.process_sample([0.8, -0.4]), [0.8, -0.4]);
    }
    settings.drive_enabled = true;
    settings.dry_wet = 1.0;
    u.retarget(&settings);
    u.reset();
    let mut fresh = unit(48000, &settings);
    for i in 0..4096 {
        let x = if i == 0 { [0.8, -0.4] } else { [0.0; 2] };
        assert_eq!(u.process_sample(x), fresh.process_sample(x));
    }
}

#[test]
fn drive_tone_changes_wet_brightness_and_keeps_neutral_silence() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut energy = Vec::new();
        for drive_tone in [0.0, 0.5, 1.0] {
            let mut u = unit(
                rate,
                &Settings {
                    drive_enabled: true,
                    drive_tone,
                    dry_wet: 1.0,
                    ..Default::default()
                },
            );
            assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
            let mut sum = 0.0;
            for i in 0..8192 {
                let input = (std::f64::consts::TAU * 8000.0 * f64::from(i) / f64::from(rate)).sin()
                    as f32
                    * 0.1;
                let y = u.process_sample([input; 2]);
                assert!(y.iter().all(|v| v.is_finite()));
                sum += f64::from(y[0]).powi(2);
            }
            energy.push(sum);
        }
        assert!(
            energy[0] < energy[1] && energy[1] < energy[2],
            "rate={rate}: {energy:?}"
        );
    }
}
