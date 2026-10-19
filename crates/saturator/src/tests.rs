use super::*;
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
#[test]
fn shapes_are_distinct_symmetric_continuous_and_have_unity_small_signal_gain() {
    for s in SaturatorShape::ALL {
        assert_eq!(shape(s, 0.0), 0.0);
        assert!((shape(s, 1e-6) / 1e-6 - 1.0).abs() < 1e-6);
        for i in 1..10000 {
            let x = i as f64 / 1000.0;
            assert_eq!(shape(s, -x), -shape(s, x));
            assert!((shape(s, x) - shape(s, x - 0.001)).abs() <= 0.001001);
        }
        for other in SaturatorShape::ALL.into_iter().filter(|o| *o != s) {
            assert!((shape(s, 1.3) - shape(other, 1.3)).abs() > 0.001);
        }
    }
}
#[test]
fn dry_and_bypass_are_exact_delays_and_clipping_level_and_mix_keep_their_order() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for settings in [
            SaturatorSettings {
                enabled: false,
                ..Default::default()
            },
            SaturatorSettings {
                dry_wet: 0.0,
                level_db: 12.0,
                drive_db: 36.0,
                ..Default::default()
            },
        ] {
            let mut unit = Saturator::new(context(rate), &settings);
            let input: Vec<_> = (0..200)
                .map(|i| [(i as f32 * 0.113).sin() * 2.0, (i as f32 * 0.23).cos()])
                .collect();
            for (n, x) in input.iter().enumerate() {
                let y = unit.process_sample(*x);
                assert_eq!(y, if n < 32 { [0.0; 2] } else { input[n - 32] });
            }
        }
        for mode in SaturatorClipMode::ALL {
            let mut s = SaturatorSettings {
                drive_db: 36.0,
                clip_mode: mode,
                ..Default::default()
            };
            let mut unit = Saturator::new(context(rate), &s);
            for _ in 0..1000 {
                unit.process_sample([0.9, -0.7]);
            }
            let y = unit.process_sample([0.9, -0.7]);
            assert!(y[0] > 0.0 && y[1] < 0.0);
            if mode == SaturatorClipMode::NoClip {
                assert!(y[0] > 1.4);
            } else {
                assert!(y.iter().all(|v| v.abs() <= 1.0));
            }
            s.level_db = SILENT_GAIN_DB;
            unit.retarget(&s);
            for _ in 0..rate / 100 {
                unit.process_sample([0.9, -0.7]);
            }
            assert_eq!(unit.process_sample([0.9, -0.7]), [0.0; 2]);
        }
    }
}
#[test]
fn modulation_scope_bounds_transitions_reset_and_finite_input_share_the_common_path() {
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
        ModulationValue, process_modulation,
    };
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut s = SaturatorSettings::default();
        let a = Modulation::new(ModulationParameter::SaturatorDrive);
        let b = Modulation::new(ModulationParameter::Level);
        let source = ModulationSourceId {
            layer: a.id,
            unit: b.id,
        };
        let value = ModulationValue {
            source,
            value: 1.0,
            weight: 1.0,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
            release_age: None,
        };
        s.modulations = ModulationParameter::SATURATOR
            .into_iter()
            .zip([36.0, 24.0, -1.0])
            .map(|(parameter, amount)| Modulation {
                source: Some(source),
                amount,
                ..Modulation::new(parameter)
            })
            .collect();
        let routes = || {
            s.modulations
                .iter()
                .map(|r| ModulationRoute::new(*r, rate))
                .collect::<Vec<_>>()
        };
        let mut local = Saturator::new(context(rate), &s);
        let mut local_routes = routes();
        let mut shared = Saturator::new(context(rate), &s);
        let mut shared_routes = routes();
        for _ in 0..rate / 50 {
            process_modulation(
                &mut local,
                &mut local_routes,
                [0.2; 2],
                &[value],
                &[],
                Default::default(),
            );
            process_modulation(
                &mut shared,
                &mut shared_routes,
                [0.2; 2],
                &[],
                &[value],
                Default::default(),
            );
        }
        assert_eq!(local.effective_parameters(), [0.0, 0.0, 1.0]);
        let p = shared.effective_parameters();
        assert!((p[0] - 36.0).abs() < 1e-3 && (p[1] - 12.0).abs() < 1e-3 && p[2] < 1e-5);
        s.shape = SaturatorShape::BassShaper;
        s.clip_mode = SaturatorClipMode::HardClip;
        s.enabled = false;
        shared.retarget(&s);
        for _ in 0..rate / 100 {
            assert!(
                shared
                    .process_sample([f32::MAX, -f32::MAX])
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
        assert_eq!(shared.process_sample([1.0; 2]), [f32::MAX, -f32::MAX]);
        shared.reset();
        let mut fresh = Saturator::new(context(rate), &s);
        for n in 0..200 {
            let x = [(n as f32 * 0.12).sin(), f32::NAN];
            assert_eq!(shared.process_sample(x), fresh.process_sample(x));
        }
    }
}
#[test]
fn oversampling_reduces_folded_third_harmonic_and_retains_low_frequency_level_and_latency() {
    let mut s = SaturatorSettings {
        shape: SaturatorShape::HardCurve,
        drive_db: 18.0,
        ..Default::default()
    };
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut unit = Saturator::new(context(rate), &s);
        let mut filtered = [0.0f64; 2];
        let mut direct = [0.0f64; 2];
        for n in 0..5120 {
            let x = (2.0 * std::f64::consts::PI * 0.21875 * n as f64).sin() * 0.5;
            let y = unit.process_sample([x as f32, 0.0])[0];
            if n >= 1024 {
                let phase = 2.0 * std::f64::consts::PI * 0.34375 * n as f64;
                for (i, basis) in [phase.cos(), phase.sin()].into_iter().enumerate() {
                    filtered[i] += f64::from(y) * basis;
                    direct[i] += shape(s.shape, x * 10_f64.powf(18.0 / 20.0)) * basis;
                }
            }
        }
        let norm = |v: [f64; 2]| v[0].hypot(v[1]);
        let reduction = 20.0 * (norm(filtered) / norm(direct)).log10();
        eprintln!("{rate} Hz: folded third harmonic reduction {reduction:.1} dB");
        assert!(reduction < -15.0, "{reduction}");
        s.drive_db = 0.0;
        s.shape = SaturatorShape::AnalogClip;
        let mut unit = Saturator::new(context(rate), &s);
        let mut peak = (0, 0.0f32);
        for n in 0..100 {
            let y = unit.process_sample([if n == 0 { 0.001 } else { 0.0 }, 0.0]);
            if y[0].abs() > peak.1 {
                peak = (n, y[0].abs());
            }
            assert_eq!(y[1], 0.0);
        }
        assert_eq!(peak.0, Saturator::LATENCY_FRAMES as usize);
        for _ in 0..200 {
            unit.process_sample([0.1; 2]);
        }
        assert!((unit.process_sample([0.1; 2])[0] - 0.1).abs() < 1e-5);
        s.drive_db = 18.0;
        s.shape = SaturatorShape::HardCurve;
    }
}
#[test]
fn processing_cost_is_measured_at_each_rate() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut unit = Saturator::new(
            context(rate),
            &SaturatorSettings {
                drive_db: 18.0,
                ..Default::default()
            },
        );
        let start = std::time::Instant::now();
        let count = rate / 10;
        for n in 0..count {
            std::hint::black_box(unit.process_sample([(n as f32 * 0.13).sin() * 0.5; 2]));
        }
        eprintln!(
            "{rate} Hz: {:.2}% of one CPU core for one stereo unit",
            start.elapsed().as_secs_f64() / 0.1 * 100.0
        );
    }
}
#[test]
fn bass_shaper_reduces_high_order_bass_harmonics_against_the_firmer_analog_knee() {
    let energy = |s| {
        let mut harmonics = [0.0f64; 4];
        for n in 0..4096 {
            let phase = 2.0 * std::f64::consts::PI * n as f64 / 4096.0;
            let y = shape(s, phase.sin() * 4.0);
            for (sum, h) in harmonics.iter_mut().zip([1.0, 5.0, 7.0, 9.0]) {
                *sum += y * (phase * h).sin();
            }
        }
        harmonics[1..]
            .iter()
            .map(|v| (v / harmonics[0]).powi(2))
            .sum::<f64>()
    };
    assert!(energy(SaturatorShape::BassShaper) < energy(SaturatorShape::AnalogClip));
}
#[test]
fn rapid_selector_retargets_preserve_small_signal_level() {
    let mut s = SaturatorSettings::default();
    let mut unit = Saturator::new(context(48000), &s);
    for _ in 0..200 {
        unit.process_sample([0.0001; 2]);
    }
    for n in 0..2000 {
        s.shape = SaturatorShape::ALL[n % 5];
        s.clip_mode = SaturatorClipMode::ALL[n % 3];
        unit.retarget(&s);
        let y = unit.process_sample([0.0001; 2]);
        assert!((y[0] - 0.0001).abs() < 1e-7, "{n} {y:?}");
    }
}
