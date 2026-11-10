use daw_harmonic_lift::HarmonicLift;
use daw_unit_core::{
    EffectUnit, HarmonicLiftSettings as Settings, HarmonicQuality, ProcessingContext,
};
fn unit(rate: u32, s: &Settings) -> HarmonicLift {
    HarmonicLift::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        s,
    )
}
#[test]
fn branch_levels_follow_gain_and_power_before_global_processing() {
    use daw_unit_core::BandMeter;
    use std::sync::Arc;
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for quality in [HarmonicQuality::Normal, HarmonicQuality::Hq] {
            let mut s = Settings {
                quality,
                ..Default::default()
            };
            for band in &mut s.bands {
                band.enabled = true;
            }
            let run = |s: &Settings| {
                let mut u = unit(rate, s);
                let meters: [_; 6] = std::array::from_fn(|_| Arc::new(BandMeter::<3>::default()));
                for (band, meter) in s.bands.iter().zip(&meters) {
                    u.set_band_level_meter(band.id, meter.clone());
                }
                for frame in 0..2048 {
                    let value = s
                        .bands
                        .iter()
                        .map(|b| {
                            (std::f32::consts::TAU * b.frequency_hz * frame as f32 / rate as f32)
                                .sin()
                                * 0.03
                        })
                        .sum::<f32>();
                    u.process_sample([value, -value * 0.5]);
                }
                meters.each_ref().map(|m| m.boost_peaks()[0].peak())
            };
            let base = run(&s);
            assert!(base.into_iter().all(|v| v > 0.0));
            let mut boosted = s.clone();
            for b in &mut boosted.bands {
                b.gain_db = 6.0;
            }
            for (gain, plain) in run(&boosted).into_iter().zip(base) {
                assert!((gain / plain - 10_f32.powf(6.0 / 20.0)).abs() < 1e-5);
            }
            let mut global = s.clone();
            global.drive = 1.0;
            global.push = 1.0;
            global.dry_wet = 0.0;
            global.level_db = -40.0;
            assert_eq!(run(&global), base);
            s.bands[3].enabled = false;
            let disabled = run(&s);
            assert_eq!(disabled[3], 0.0);
            assert!(disabled.iter().enumerate().all(|(i, v)| i == 3 || *v > 0.0));
        }
    }
}
#[test]
fn neutral_dry_and_bypass_have_fixed_sample_alignment_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for quality in [HarmonicQuality::Normal, HarmonicQuality::Hq] {
            for mode in 0..3 {
                let s = Settings {
                    quality,
                    enabled: mode != 2,
                    dry_wet: if mode == 1 { 0.0 } else { 1.0 },
                    ..Default::default()
                };
                let mut u = unit(rate, &s);
                assert_eq!(u.latency_frames(), 49);
                let y: Vec<_> = (0..256)
                    .map(|i| u.process_sample(if i == 0 { [1.0, -0.25] } else { [0.0; 2] }))
                    .collect();
                let peak = y
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a[0].abs().total_cmp(&b[0].abs()))
                    .unwrap()
                    .0;
                assert_eq!(peak, 49, "{rate} {quality:?} mode{mode}");
                if quality == HarmonicQuality::Normal || mode != 0 {
                    assert_eq!(y[49], [1.0, -0.25]);
                    assert!(y.iter().enumerate().all(|(i, v)| i == 49 || *v == [0.0; 2]));
                }
            }
        }
    }
}
#[test]
fn maximizer_links_channels_and_final_level_can_exceed_ceiling() {
    let s = Settings {
        push: 1.0,
        limit_db: -6.0,
        ..Default::default()
    };
    let mut u = unit(48000, &s);
    let mut max = 0_f32;
    for _ in 0..2000 {
        let y = u.process_sample([2.0, 0.5]);
        max = max.max(y[0]);
        assert!((y[0] - 4.0 * y[1]).abs() < 1e-5);
    }
    assert!(max <= 10_f32.powf(-6.0 / 20.0) + 1e-5);
    assert!(max > 0.4);
    let s = Settings {
        push: 1.0,
        level_db: 12.0,
        ..Default::default()
    };
    let mut u = unit(48000, &s);
    let y = (0..2000)
        .map(|_| u.process_sample([2.0; 2])[0])
        .fold(0_f32, f32::max);
    assert!(y > 1.0);
}
#[test]
fn branches_add_asymmetric_harmonics_and_reset_removes_history() {
    let mut s = Settings::default();
    s.bands[2].enabled = true;
    s.bands[2].frequency_hz = 1000.0;
    s.bands[2].gain_db = 12.0;
    let mut u = unit(48000, &s);
    let mut second = [0_f64; 2];
    let mut energy = 0.0;
    for n in 0..9600 {
        let phase = std::f64::consts::TAU * n as f64 / 48.0;
        let y = u.process_sample([(0.8 * phase.sin()) as f32; 2]);
        assert_eq!(y[0], y[1]);
        if n >= 4800 {
            let p = 2.0 * (phase - std::f64::consts::TAU * 49.0 / 48.0);
            second[0] += f64::from(y[0]) * p.cos();
            second[1] += f64::from(y[0]) * p.sin();
            energy += f64::from(y[0]).powi(2);
        }
    }
    assert!(second[0].hypot(second[1]) / 4800.0 > 0.01);
    assert!(energy > 1.0);
    u.reset();
    assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
}
#[test]
fn quality_automation_preserves_alignment_and_bypass_clears_meters() {
    let mut s = Settings::default();
    let mut u = unit(48000, &s);
    let output = std::sync::Arc::new(daw_unit_core::BandMeter::<3>::default());
    let reduction = std::sync::Arc::new(daw_unit_core::PeakMeter::default());
    u.set_output_peak_meter(output.clone());
    u.set_gain_reduction_peak_meter(reduction.clone());
    for _ in 0..2000 {
        u.process_sample([0.25; 2]);
    }
    s.quality = HarmonicQuality::Hq;
    assert!(u.retarget(&s));
    for _ in 0..1000 {
        let y = u.process_sample([0.25; 2]);
        assert!((y[0] - 0.25).abs() < 0.001, "{}", y[0]);
    }
    assert!(output.readings()[0] > 0.0);
    s.enabled = false;
    u.retarget(&s);
    assert_eq!(output.readings(), [0.0; 3]);
    assert_eq!(reduction.reading(), 0.0);
    for _ in 0..1000 {
        u.process_sample([1.0; 2]);
    }
    assert_eq!(output.readings(), [0.0; 3]);
}
#[test]
fn finite_extremes_and_nonfinite_inputs_do_not_poison_future_audio() {
    for quality in [HarmonicQuality::Normal, HarmonicQuality::Hq] {
        let mut s = Settings {
            quality,
            input_db: 12.0,
            drive: 1.0,
            push: 1.0,
            level_db: 12.0,
            ..Default::default()
        };
        for b in &mut s.bands {
            b.enabled = true;
            b.gain_db = 24.0;
        }
        let mut u = unit(48000, &s);
        for x in [f32::MAX, -f32::MAX, f32::NAN, f32::INFINITY, 0.0] {
            for _ in 0..100 {
                assert!(u.process_sample([x; 2]).iter().all(|v| v.is_finite()));
            }
        }
        u.reset();
        for _ in 0..256 {
            assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
        }
    }
}

#[test]
fn shared_modulation_targets_one_branch_and_clamps_native_ranges() {
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationParameter as P, ModulationRange,
        ModulationSourceId, ModulationValue,
    };
    let mut s = Settings::default();
    let source = ModulationSourceId {
        layer: s.bands[0].id,
        unit: s.bands[1].id,
    };
    for (parameter, target, amount) in [
        (P::EqBandGain, Some(s.bands[0].id), 24.0),
        (P::EqBandFrequency, Some(s.bands[0].id), 4.0),
        (P::HarmonicPush, None, 1.0),
    ] {
        let mut r = Modulation::new(parameter);
        r.target = target;
        r.source = Some(source);
        r.amount = amount;
        s.modulations.push(r);
    }
    assert!(s.valid());
    let original = s.clone();
    let mut u = unit(48000, &s);
    let shared = [ModulationValue {
        source,
        release_age: None,
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    }];
    for _ in 0..2048 {
        u.process_at([0.0; 2], &shared, Default::default());
    }
    assert_eq!(u.band_parameters(s.bands[0].id), Some([250.0, 24.0]));
    let other = u.band_parameters(s.bands[1].id).unwrap();
    assert!((other[0] - 375.0).abs() < 0.001);
    assert_eq!(other[1], 0.0);
    assert_eq!(u.effective_parameters()[2], 1.0);
    assert_eq!(s, original);
}

#[test]
fn hq_reduces_folded_drive_harmonics() {
    let alias = |quality| {
        let s = Settings {
            quality,
            drive: 0.8,
            ..Default::default()
        };
        let mut u = unit(48000, &s);
        let mut sum = [0_f64; 2];
        for n in 0..8192 {
            let x = (std::f64::consts::TAU * 3.0 * n as f64 / 16.0).sin() * 0.9;
            let y = f64::from(u.process_sample([x as f32; 2])[0]);
            if n >= 4096 {
                let p = std::f64::consts::TAU * 7.0 * n as f64 / 16.0;
                sum[0] += y * p.cos();
                sum[1] += y * p.sin();
            }
        }
        sum[0].hypot(sum[1]) / 4096.0
    };
    let normal = alias(HarmonicQuality::Normal);
    let hq = alias(HarmonicQuality::Hq);
    assert!(normal > 0.001);
    assert!(hq < normal * 0.5, "Normal {normal}, HQ {hq}");
}

#[test]
fn zero_push_boundary_ramps_on_hot_input_in_both_directions() {
    for quality in [HarmonicQuality::Normal, HarmonicQuality::Hq] {
        let mut s = Settings {
            quality,
            ..Default::default()
        };
        let mut u = unit(48000, &s);
        for _ in 0..4096 {
            u.process_sample([2.0; 2]);
        }
        let mut last = u.process_sample([2.0; 2])[0];
        for push in [0.0001, 0.0] {
            s.push = push;
            assert!(u.retarget(&s));
            for _ in 0..1024 {
                let y = u.process_sample([2.0; 2])[0];
                assert!((y - last).abs() < 0.01, "{quality:?}, {push}: {last} → {y}");
                last = y;
            }
        }
        assert!((last - 2.0).abs() < 0.001);
    }
}
