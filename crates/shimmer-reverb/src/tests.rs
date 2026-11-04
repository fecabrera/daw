use super::*;
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn unit(rate: u32, s: &Settings) -> ShimmerReverb {
    ShimmerReverb::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        s,
    )
}

#[test]
fn silence_dry_bypass_reset_and_meter_lifecycle_at_all_rates() {
    for rate in RATES {
        for settings in [
            Settings {
                enabled: false,
                ..Default::default()
            },
            Settings {
                dry_wet: 0.0,
                ..Default::default()
            },
        ] {
            let mut u = unit(rate, &settings);
            for i in 0..1024 {
                let x = [(i as f32 * 0.03).sin() * 0.2, -0.13];
                assert_eq!(u.process_sample(x), x);
            }
        }
        let mut settings = Settings {
            dry_wet: 1.0,
            pitch_1: 15,
            amount_1: 1.0,
            ..Default::default()
        };
        let mut u = unit(rate, &settings);
        let meter = Arc::new(BandMeter::<3>::default());
        u.set_output_peak_meter(meter.clone());
        for _ in 0..256 {
            assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
        }
        u.reset();
        let expected: Vec<_> = (0..rate / 4)
            .map(|i| u.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] }))
            .collect();
        assert!(expected.iter().flatten().any(|x| x.abs() > 1e-8));
        assert!(expected.iter().flatten().all(|x| x.is_finite()));
        assert!(meter.boost_peaks()[0].peak() > 0.0);
        let mut clone = u.clone();
        settings.enabled = false;
        clone.retarget(&settings);
        assert!(
            meter.boost_peaks()[0].peak() > 0.0,
            "cloned processor must detach its meter"
        );
        u.reset();
        for (i, y) in expected.into_iter().enumerate() {
            assert_eq!(
                u.process_sample(if i == 0 { [0.2, -0.1] } else { [0.0; 2] }),
                y
            );
        }
        u.retarget(&settings);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        for _ in 0..rate / 100 {
            u.process_sample([0.2, -0.1]);
        }
        assert_eq!(u.process_sample([0.2, -0.1]), [0.2, -0.1]);
        settings.enabled = true;
        settings.level_db = SILENT_GAIN_DB;
        u.retarget(&settings);
        for _ in 0..rate / 100 {
            u.process_sample([1.0; 2]);
        }
        assert_eq!(
            u.process_modulated_at([1.0; 2], 0, 120.0, [24.0; 12]),
            [0.0; 2]
        );
    }
}

#[test]
fn automation_modulation_and_extreme_input_stay_finite_and_bounded() {
    for rate in RATES {
        let mut s = Settings {
            size: 1.0,
            dry_wet: 1.0,
            amount_1: 1.0,
            amount_2: 1.0,
            pitch_1: 15,
            pitch_2: -15,
            ..Default::default()
        };
        let mut u = unit(rate, &s);
        for i in 0..rate / 2 {
            if i % 127 == 0 {
                let f = if i % 254 == 0 { 0.0 } else { 1.0 };
                s.size = f;
                s.depth = f;
                s.rate = 1.0 - f;
                s.vowel = f;
                s.vowel_amount = f;
                s.low_damp = f;
                s.high_damp = 1.0 - f;
                s.warm = f == 1.0;
                s.pitch_1 = (i % 31) as i8 - 15;
                s.pitch_2 = -s.pitch_1;
                s.predelay_ms = f * 1000.0;
                s.sync = i % 381 == 0;
                s.note = (i % 16) as u8;
                u.retarget(&s);
            }
            let x = if i < 256 {
                [(i as f32 * 0.2).sin() * 0.2; 2]
            } else {
                [0.0; 2]
            };
            let offsets = if i % 3 == 0 {
                [f32::NAN; 12]
            } else {
                [if i % 2 == 0 { 1.0 } else { -1.0 }; 12]
            };
            let y = u.process_modulated_at(
                x,
                u64::from(i),
                if i % 2 == 0 { 0.001 } else { 300.0 },
                offsets,
            );
            assert!(y.into_iter().all(|x| x.is_finite() && x.abs() < 10.0));
        }
        for x in [[f32::MAX, -f32::MAX], [f32::NAN, f32::INFINITY]] {
            for _ in 0..512 {
                assert!(u.process_sample(x).into_iter().all(f32::is_finite));
            }
        }
    }
}

#[test]
fn pitch_heads_shift_up_and_down_and_zero_interval_mutes_regeneration() {
    let rate = 48000;
    for (interval, expected_hz) in [(15, 1200.0), (-15, 300.0)] {
        let mut voice = pitch::Pitch::new(rate, interval, 0.8);
        let mut correlation = [(0.0, 0.0); 3];
        for n in 0..rate * 2 {
            let x = (std::f64::consts::TAU * 600.0 * f64::from(n) / f64::from(rate)).sin() * 0.1;
            let y = voice.sample([x, x], 0.8, true, 240)[0];
            if n >= rate {
                for (i, hz) in [expected_hz, 600.0, expected_hz * 1.25]
                    .into_iter()
                    .enumerate()
                {
                    let a = std::f64::consts::TAU * hz * f64::from(n) / f64::from(rate);
                    correlation[i].0 += y * a.cos();
                    correlation[i].1 += y * a.sin();
                }
            }
        }
        let powers = correlation.map(|(re, im)| re * re + im * im);
        assert!(
            powers[0] > powers[1] * 4.0 && powers[0] > powers[2] * 4.0,
            "interval {interval}, powers {powers:?}"
        );
    }
    let mut voice = pitch::Pitch::new(rate, 0, 0.0);
    for _ in 0..1024 {
        assert_eq!(voice.sample([0.1; 2], 1.0, false, 240), [0.0; 2]);
    }
}

#[test]
fn synchronized_predelay_clamps_and_retains_free_time() {
    let s = Settings {
        sync: true,
        note: 15,
        predelay_ms: 234.0,
        ..Default::default()
    };
    let mut u = unit(48000, &s);
    for _ in 0..4096 {
        u.process_modulated_at([0.0; 2], 0, 0.01, [0.0; 12]);
    }
    assert_eq!(u.effective_parameters()[1], 8000.0);
    let mut free = s.clone();
    free.sync = false;
    u.retarget(&free);
    for _ in 0..1024 {
        u.process_sample([0.0; 2]);
    }
    assert_eq!(u.effective_parameters()[1], 234.0);
}

#[test]
fn reset_after_unprocessed_retarget_matches_fresh_parameters_and_first_host_tempo() {
    for rate in [48000, 192000] {
        for sync in [false, true] {
            let mut u = unit(rate, &Settings::default());
            for _ in 0..1024 {
                u.process_sample([0.1; 2]);
            }
            let target = Settings {
                sync,
                note: 0,
                predelay_ms: 1.0,
                size: 1.0,
                pitch_1: 15,
                amount_1: 1.0,
                pitch_2: -15,
                amount_2: 0.7,
                dry_wet: 1.0,
                ..Default::default()
            };
            u.retarget(&target);
            u.reset();
            let mut fresh = unit(rate, &target);
            for n in 0..rate {
                let x = if n < 512 {
                    [(n as f32 * 0.17).sin() * 0.1, -0.1]
                } else {
                    [0.0; 2]
                };
                assert_eq!(
                    u.process_modulated_at(x, u64::from(n), 2000.0, [0.0; 12]),
                    fresh.process_modulated_at(x, u64::from(n), 2000.0, [0.0; 12]),
                    "rate={rate}, sync={sync}, frame={n}"
                );
            }
        }
    }
}

#[test]
fn reset_discards_modulated_pitch_amount_history_and_unfinished_amount_ramps() {
    let s = Settings {
        pitch_1: -15,
        pitch_2: 15,
        amount_1: 0.2,
        amount_2: 0.4,
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut u = unit(48000, &s);
    for n in 0..24000 {
        let mut offsets = [0.0; 12];
        offsets[4] = 0.8;
        offsets[5] = -0.4;
        u.process_modulated_at([0.1, -0.1], n, 120.0, offsets);
    }
    u.reset();
    let mut fresh = unit(48000, &s);
    for n in 0..48000 {
        let x = if n < 512 {
            [(n as f32 * 0.13).sin() * 0.1, -0.1]
        } else {
            [0.0; 2]
        };
        assert_eq!(
            u.process_sample(x),
            fresh.process_sample(x),
            "after modulation, frame {n}"
        );
    }
    let next = Settings {
        amount_1: 0.9,
        amount_2: 0.1,
        ..s
    };
    u.retarget(&next);
    for _ in 0..12 {
        u.process_sample([0.1; 2]);
    }
    u.reset();
    let mut fresh = unit(48000, &next);
    for n in 0..48000 {
        let x = if n < 512 { [0.1, -0.1] } else { [0.0; 2] };
        assert_eq!(
            u.process_sample(x),
            fresh.process_sample(x),
            "after partial ramp, frame {n}"
        );
    }
}

/// Run explicitly in release: covers the reported 44.86-second failure horizon,
/// all six rates, maximum regeneration, frozen motion and automated motion.
#[test]
#[ignore = "120-second six-rate tail/stability stress; run in release"]
fn long_tail_stability_at_all_rates() {
    for rate in RATES {
        for moving in [false, true] {
            let mut s = Settings {
                size: 1.0,
                dry_wet: 1.0,
                depth: 0.0,
                rate: 0.0,
                pitch_1: 15,
                pitch_2: -15,
                amount_1: 1.0,
                amount_2: 1.0,
                ..Default::default()
            };
            let mut u = unit(rate, &s);
            let mut first_peak = 0.0f32;
            let mut last_peak = 0.0f32;
            for n in 0..rate * 120 {
                if moving && n % 512 == 0 {
                    let phase = (f64::from(n) / f64::from(rate) * 0.3).sin() as f32;
                    s.depth = 0.5 + 0.5 * phase;
                    s.size = 0.75 + 0.25 * phase;
                    s.rate = 1.0 - s.depth;
                    u.retarget(&s);
                }
                let y = u.process_sample(if n == 0 { [0.1, -0.05] } else { [0.0; 2] });
                for x in y {
                    assert!(x.is_finite() && x.abs() < 4.0, "rate {rate}, frame {n}");
                    if n < rate * 5 {
                        first_peak = first_peak.max(x.abs());
                    }
                    if n > rate * 110 {
                        last_peak = last_peak.max(x.abs());
                    }
                }
            }
            eprintln!(
                "{rate} Hz moving={moving}: first={first_peak:e}, last={last_peak:e}, heap={} bytes",
                u.heap_bytes()
            );
            assert!(first_peak > 1e-7 && last_peak < first_peak * 1e-5);
        }
    }
}
