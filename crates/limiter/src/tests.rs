use super::*;
pub(super) const RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];
fn unit(rate: u32, settings: &LimiterSettings) -> Limiter {
    Limiter::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        settings,
    )
}
fn render(rate: u32, settings: &LimiterSettings, input: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut limiter = unit(rate, settings);
    let delay = limiter.latency_frames() as usize;
    input
        .iter()
        .copied()
        .chain(std::iter::repeat_n([0.0; 2], delay))
        .map(|sample| limiter.process_sample(sample))
        .skip(delay)
        .collect()
}
#[test]
fn held_reduction_captures_sample_peaks_without_changing_audio_or_latency() {
    for rate in RATES {
        for mode in [LimiterMode::Peak, LimiterMode::TruePeak] {
            for routing in [LimiterRouting::LeftRight, LimiterRouting::MidSide] {
                let mut settings = LimiterSettings {
                    mode,
                    routing,
                    ceiling_db: -6.0,
                    release_ms: 1.0,
                    ..Default::default()
                };
                let mut monitored = unit(rate, &settings);
                let mut reference = unit(rate, &settings);
                let meter = Arc::new(PeakMeter::default());
                monitored.set_gain_reduction_peak_meter(meter.clone());
                assert_eq!(monitored.latency_frames(), reference.latency_frames());
                let mut peak = 0.0_f32;
                for frame in 0..6000 {
                    let input = if frame < 500 { [4.0, -1.0] } else { [0.0; 2] };
                    assert_eq!(
                        monitored.process_sample(input),
                        reference.process_sample(input)
                    );
                    peak = peak.max(monitored.gain_reduction_db());
                }
                assert_eq!(meter.peak(), peak);
                assert!(peak > 6.0);
                assert!(meter.reading() < peak * 0.1);
                let current = meter.reading();
                meter.reset_peak();
                assert_eq!(meter.peak(), 0.0);
                assert_eq!(meter.reading(), current);
                for _ in 0..2000 {
                    monitored.process_sample([4.0, -1.0]);
                }
                assert!(meter.peak() > 6.0);
                let held = meter.peak();
                monitored.retarget(&settings);
                monitored.reset();
                assert_eq!(meter.peak(), held);
                assert_eq!(meter.reading(), 0.0);
                settings.enabled = false;
                monitored.retarget(&settings);
                assert_eq!(meter.peak(), 0.0);
                for _ in 0..2000 {
                    monitored.process_sample([4.0, -1.0]);
                }
                assert_eq!(meter.reading(), 0.0);
                assert_eq!(meter.peak(), 0.0);
                settings.enabled = true;
                monitored.retarget(&settings);
                for _ in 0..2000 {
                    monitored.process_sample([4.0, -1.0]);
                }
                assert!(meter.peak() > 6.0);
            }
        }
    }
}

#[test]
fn bypass_retains_exact_audio_and_fixed_latency_and_reset_at_all_rates() {
    for rate in RATES {
        let s = LimiterSettings {
            enabled: false,
            input_db: 24.0,
            ..Default::default()
        };
        let input: Vec<_> = (0..1000)
            .map(|i| [(i as f32).sin() * 2.0, (i as f32).cos()])
            .collect();
        assert_eq!(render(rate, &s, &input), input);
        let mut limiter = unit(rate, &s);
        let latency = limiter.latency_frames();
        for _ in 0..300 {
            limiter.process_sample([3.0, -2.0]);
        }
        limiter.reset();
        for _ in 0..latency {
            assert_eq!(limiter.process_sample([0.0; 2]), [0.0; 2]);
        }
        assert_eq!(limiter.gain_reduction_db(), 0.0);
    }
}
#[test]
fn peak_ceiling_and_linked_balance_hold_for_transients_and_all_controls() {
    for rate in RATES {
        for routing in [LimiterRouting::LeftRight, LimiterRouting::MidSide] {
            for lookahead_ms in [0.0, 1.0, 10.0] {
                let s = LimiterSettings {
                    ceiling_db: -6.0,
                    input_db: 24.0,
                    release_ms: 1.0,
                    lookahead_ms,
                    routing,
                    ..Default::default()
                };
                let input: Vec<_> = (0..3000)
                    .map(|i| {
                        let a = if i % 113 == 0 {
                            10.0
                        } else {
                            (i as f32 * 0.17).sin()
                        };
                        [a, a * 0.25]
                    })
                    .collect();
                let output = render(rate, &s, &input);
                let ceiling = amplitude(s.ceiling_db) as f32;
                for sample in output {
                    assert!(
                        sample.iter().all(|v| v.is_finite() && v.abs() <= ceiling),
                        "{rate} {sample:?}"
                    );
                    if routing == LimiterRouting::LeftRight {
                        assert!((sample[1] - sample[0] * 0.25).abs() < 1e-6);
                    }
                }
            }
        }
    }
}
#[test]
fn mid_side_retains_more_side_energy_and_recombination_still_meets_ceiling() {
    let mut side = [0.0; 2];
    for (i, routing) in [LimiterRouting::LeftRight, LimiterRouting::MidSide]
        .into_iter()
        .enumerate()
    {
        let s = LimiterSettings {
            routing,
            ..Default::default()
        };
        let output = render(48_000, &s, &vec![[2.1, 1.9]; 4096]);
        let sample = output[3000];
        side[i] = (sample[0] - sample[1]) * 0.5;
        assert!(sample[0] <= amplitude(s.ceiling_db) as f32);
    }
    assert!(side[1] > side[0] * 1.5, "{side:?}");
}
// Independent 16x/128-tap reconstruction, deliberately different from the 8x guard.
pub(super) fn reconstructed_peak(input: &[[f32; 2]]) -> f64 {
    let mut peak: f64 = 0.0;
    for center in -64..input.len() as isize + 64 {
        for phase in 0..16 {
            let mut sum = 0.0;
            let mut output = [0.0; 2];
            for tap in -63..=64 {
                let x = tap as f64 - phase as f64 / 16.0;
                let sinc = if x.abs() < 1e-10 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                let coefficient =
                    sinc * (0.5 + 0.5 * (std::f64::consts::PI * x / 64.0).cos()).max(0.0);
                sum += coefficient;
                let index = center + tap;
                if (0..input.len() as isize).contains(&index) {
                    for ch in 0..2 {
                        output[ch] += coefficient * f64::from(input[index as usize][ch]);
                    }
                }
            }
            peak = peak.max(output[0].abs() / sum).max(output[1].abs() / sum);
        }
    }
    peak
}
#[test]
fn true_peak_controls_intersample_overshoot_with_an_independent_reconstruction() {
    for routing in [LimiterRouting::LeftRight, LimiterRouting::MidSide] {
        let input: Vec<_> = (0..1024)
            .map(|i| {
                let wave = (std::f32::consts::TAU * 0.25 * i as f32 + std::f32::consts::FRAC_PI_4)
                    .sin()
                    * 1.4;
                [wave, wave * 0.8]
            })
            .collect();
        let settings = LimiterSettings {
            routing,
            ceiling_db: -3.0,
            lookahead_ms: 0.0,
            release_ms: 1.0,
            ..Default::default()
        };
        let peak_output = render(48_000, &settings, &input);
        let true_output = render(
            48_000,
            &LimiterSettings {
                mode: LimiterMode::TruePeak,
                ..settings.clone()
            },
            &input,
        );
        let ceiling = amplitude(settings.ceiling_db);
        assert!(reconstructed_peak(&peak_output) > ceiling * 1.2);
        let actual = reconstructed_peak(&true_output);
        assert!(
            actual <= ceiling * 1.012,
            "{routing:?}: {actual} > {ceiling}"
        );
        assert!(true_output.iter().flatten().any(|v| v.abs() > 0.1));
    }
}
#[test]
fn lookahead_reduces_gain_before_a_transient_without_changing_latency() {
    let rate = 48_000;
    let mut input = vec![[0.25; 2]; 4000];
    input[2500] = [4.0; 2];
    let no = render(
        rate,
        &LimiterSettings {
            lookahead_ms: 0.0,
            ..Default::default()
        },
        &input,
    );
    let ahead = render(
        rate,
        &LimiterSettings {
            lookahead_ms: 10.0,
            ..Default::default()
        },
        &input,
    );
    assert!(ahead[2300][0] < no[2300][0] * 0.6);
    assert_eq!(ahead.len(), input.len());
}
#[test]
fn live_retargeting_retains_history_and_all_extreme_inputs_remain_finite() {
    let mut a = unit(48_000, &LimiterSettings::default());
    let mut b = unit(48_000, &LimiterSettings::default());
    for i in 0..4000 {
        let input = [(i as f32 * 0.2).sin() * 2.0; 2];
        if i % 137 == 0 {
            b.retarget(&LimiterSettings::default());
        }
        assert_eq!(a.process_sample(input), b.process_sample(input));
    }
    let settings = LimiterSettings {
        mode: LimiterMode::TruePeak,
        routing: LimiterRouting::MidSide,
        input_db: 24.0,
        ceiling_db: -24.0,
        release_ms: 1.0,
        ..Default::default()
    };
    a.retarget(&settings);
    for input in [[f32::NAN, f32::INFINITY], [f32::MAX, -f32::MAX], [0.0; 2]]
        .into_iter()
        .cycle()
        .take(2000)
    {
        assert!(a.process_sample(input).iter().all(|v| v.is_finite()));
        assert!(a.gain_reduction_db().is_finite());
    }
}

#[test]
fn true_peak_guard_handles_bursts_and_high_frequency_stereo_at_every_rate() {
    for rate in RATES {
        for frequency in [0.03, 0.1, 0.45, 0.49] {
            let input: Vec<_> = (0..512)
                .map(|i| {
                    let angle = std::f32::consts::TAU * frequency * i as f32;
                    let gate = if (100..380).contains(&i) { 2.0 } else { 0.2 };
                    [(angle + 0.7).sin() * gate, (angle - 0.4).cos() * gate]
                })
                .collect();
            let s = LimiterSettings {
                mode: LimiterMode::TruePeak,
                input_db: 12.0,
                ceiling_db: -6.0,
                lookahead_ms: 10.0,
                routing: LimiterRouting::MidSide,
                ..Default::default()
            };
            let actual = reconstructed_peak(&render(rate, &s, &input));
            assert!(
                actual <= amplitude(s.ceiling_db) * 1.012,
                "rate={rate} f={frequency}: {actual}"
            );
        }
    }
}

#[test]
fn enabled_quiet_audio_is_transparent_and_release_controls_recovery() {
    let quiet: Vec<_> = (0..1000)
        .map(|i| [(i as f32 * 0.17).sin() * 0.1, 0.05])
        .collect();
    for mode in [LimiterMode::Peak, LimiterMode::TruePeak] {
        assert_eq!(
            render(
                48_000,
                &LimiterSettings {
                    mode,
                    ..Default::default()
                },
                &quiet
            ),
            quiet
        );
    }
    let mut input = vec![[0.25; 2]; 4000];
    input[1000] = [4.0; 2];
    let fast = render(
        48_000,
        &LimiterSettings {
            release_ms: 1.0,
            ..Default::default()
        },
        &input,
    );
    let slow = render(
        48_000,
        &LimiterSettings {
            release_ms: 2000.0,
            ..Default::default()
        },
        &input,
    );
    assert!(fast[3500][0] > slow[3500][0] * 2.0);
}
