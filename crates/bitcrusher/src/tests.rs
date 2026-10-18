use super::*;
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
    ModulationValue, process_modulation,
};
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn settle(unit: &mut Bitcrusher, rate: u32, offsets: [f32; 5]) {
    for _ in 0..rate / 10 {
        unit.process_modulated([0.0; 2], offsets);
    }
}
#[test]
fn quantization_has_expected_levels_symmetry_clipping_and_silence_at_every_depth() {
    for rate in RATES {
        for bits in 1..=24 {
            let mut unit = Bitcrusher::new(
                context(rate),
                &BitcrusherSettings {
                    resolution_bits: bits,
                    ..Default::default()
                },
            );
            assert_eq!(unit.process_sample([0.0; 2]), [0.0; 2]);
            assert_eq!(unit.process_sample([-2.0, 2.0]), [-1.0, 1.0]);
            for sample in [0.01, 0.2, 0.6, 0.99] {
                let output = unit.process_sample([sample, -sample]);
                assert_eq!(output[0], -output[1]);
                let interval = 1.0 / f64::from((1_u32 << bits) - 1);
                assert!((f64::from(output[0]) - f64::from(sample)).abs() <= interval + 6e-8);
            }
        }
        let mut one = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                resolution_bits: 1,
                ..Default::default()
            },
        );
        assert_eq!(one.process_sample([0.1, -0.1]), [1.0, -1.0]);
        assert_eq!(one.process_sample([f32::MAX, -f32::MAX]), [1.0, -1.0]);
        let mut three = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                resolution_bits: 3,
                ..Default::default()
            },
        );
        let out = three.process_sample([0.2, 0.6]);
        assert!((out[0] - 1.0 / 7.0).abs() < 1e-7 && (out[1] - 5.0 / 7.0).abs() < 1e-7);
    }
}
#[test]
fn every_whole_ratio_holds_independent_channels_on_one_capture_clock() {
    for rate in RATES {
        for ratio in 1..=40 {
            let mut unit = Bitcrusher::new(
                context(rate),
                &BitcrusherSettings {
                    downsampling: ratio,
                    ..Default::default()
                },
            );
            let mut captured = [0.0; 2];
            for frame in 0..120 {
                let input = [frame as f32 / 200.0, -(frame as f32 + 1.0) / 200.0];
                let output = unit.process_sample(input);
                if frame % u32::from(ratio) == 0 {
                    assert!(
                        output
                            .iter()
                            .zip(input)
                            .all(|(a, b)| (a - b).abs() < 1.2e-7)
                    );
                    captured = output;
                }
                assert_eq!(
                    output, captured,
                    "rate={rate}, ratio={ratio}, frame={frame}"
                );
            }
            unit.reset();
            let output = unit.process_sample([0.25, -0.75]);
            assert!((output[0] - 0.25).abs() < 1.2e-7 && (output[1] + 0.75).abs() < 1.2e-7);
        }
        let mut unit = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                downsampling: 2,
                ..Default::default()
            },
        );
        for frame in 0..20 {
            let input = if frame % 2 == 0 { 0.25 } else { -0.25 };
            assert!(
                (unit.process_sample([input; 2])[0] - 0.25).abs() < 1.2e-7,
                "sample holding must retain aliasing"
            );
        }
    }
}
#[test]
fn drive_precedes_clipping_and_output_gain_follows_the_linear_blend() {
    for rate in RATES {
        let settings = BitcrusherSettings {
            drive_db: 12.0,
            resolution_bits: 1,
            level_db: -6.0,
            dry_wet: 0.25,
            ..Default::default()
        };
        let mut unit = Bitcrusher::new(context(rate), &settings);
        let output = unit.process_sample([0.5, -0.25]);
        let gain = 10_f64.powf(-6.0 / 20.0);
        assert!((f64::from(output[0]) - (0.5 * 0.75 + 1.0 * 0.25) * gain).abs() < 1e-7);
        assert!((f64::from(output[1]) - (-0.25 * 0.75 - 1.0 * 0.25) * gain).abs() < 1e-7);
        let mut driven = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                drive_db: 12.0,
                ..Default::default()
            },
        );
        let output = driven.process_sample([0.1, 0.5]);
        assert!((f64::from(output[0]) - 0.1 * 10_f64.powf(12.0 / 20.0)).abs() < 1.2e-7);
        assert_eq!(output[1], 1.0);
        let mut silent = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                level_db: SILENT_GAIN_DB,
                dry_wet: 0.0,
                ..settings
            },
        );
        for _ in 0..rate / 100 {
            assert_eq!(
                silent.process_modulated([0.75; 2], [0.0, 0.0, 0.0, 24.0, 0.0]),
                [0.0; 2]
            );
        }
    }
}
#[test]
fn modulation_resolves_to_whole_steps_bounds_and_equivalent_fixed_controls() {
    for rate in RATES {
        let settings = BitcrusherSettings {
            resolution_bits: 12,
            downsampling: 20,
            ..Default::default()
        };
        let mut unit = Bitcrusher::new(context(rate), &settings);
        let offsets = [6.0, -3.2, 2.8, -6.0, -0.25];
        settle(&mut unit, rate, offsets);
        let p = unit.effective_parameters();
        for (a, b) in p.into_iter().zip([6.0, 9.0, 23.0, -6.0, 0.75]) {
            assert!((a - b).abs() < 1e-5);
        }
        let fixed_settings = BitcrusherSettings {
            drive_db: 6.0,
            resolution_bits: 9,
            downsampling: 23,
            level_db: -6.0,
            dry_wet: 0.75,
            ..Default::default()
        };
        let mut fixed = Bitcrusher::new(context(rate), &fixed_settings);
        // Start both capture clocks from reset, then settle offsets with the same silence.
        unit.reset();
        for _ in 0..rate / 10 {
            unit.process_modulated([0.0; 2], offsets);
            fixed.process_sample([0.0; 2]);
        }
        // Align the next capture independently of intermediate ratio transitions.
        // A constant input covers a full period before comparing the held output.
        for frame in 0..200 {
            let a = unit.process_modulated([0.3, -0.2], offsets);
            let b = fixed.process_sample([0.3, -0.2]);
            if frame >= 40 {
                assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 2e-7));
            }
        }
        for offsets in [[1000.0; 5], [-1000.0; 5]] {
            settle(&mut unit, rate, offsets);
            let p = unit.effective_parameters();
            assert!(p[1].fract() == 0.0 && p[2].fract() == 0.0);
            assert!(
                (0.0..=24.0).contains(&p[0])
                    && (1.0..=24.0).contains(&p[1])
                    && (1.0..=40.0).contains(&p[2])
            );
            assert!(p[3] <= 12.00001 && (0.0..=1.0).contains(&p[4]));
        }
    }
}
#[test]
fn shared_entry_point_uses_all_five_targets_and_rejects_per_note_sources() {
    for rate in RATES {
        let source = ModulationSourceId {
            layer: daw_unit_core::ModulationTargetId::new_v4(),
            unit: daw_unit_core::ModulationTargetId::new_v4(),
        };
        let value = ModulationValue {
            source,
            release_age: None,
            value: 1.0,
            weight: 1.0,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
        };
        let settings = BitcrusherSettings {
            resolution_bits: 12,
            downsampling: 10,
            ..Default::default()
        };
        let amounts = [6.0, -4.0, 3.0, -6.0, -0.5];
        let make_routes = || {
            ModulationParameter::BITCRUSHER
                .into_iter()
                .zip(amounts)
                .map(|(parameter, amount)| {
                    ModulationRoute::new(
                        Modulation {
                            source: Some(source),
                            amount,
                            ..Modulation::new(parameter)
                        },
                        rate,
                    )
                })
                .collect::<Vec<_>>()
        };
        let mut routes = make_routes();
        let mut unit = Bitcrusher::new(context(rate), &settings);
        let mut direct = Bitcrusher::new(context(rate), &settings);
        let mut local = Bitcrusher::new(context(rate), &settings);
        let mut local_routes = make_routes();
        let mut neutral = Bitcrusher::new(context(rate), &settings);
        for frame in 0..rate / 50 {
            let x = ((frame as f32) * 0.013).sin() * 0.25;
            let a = process_modulation(
                &mut unit,
                &mut routes,
                [x; 2],
                &[],
                &[value],
                ModulationContext::default(),
            );
            assert_eq!(a, direct.process_modulated([x; 2], amounts));
            let b = process_modulation(
                &mut local,
                &mut local_routes,
                [x; 2],
                &[value],
                &[],
                ModulationContext::default(),
            );
            assert_eq!(b, neutral.process_sample([x; 2]));
        }
        assert_eq!(unit.effective_parameters()[1..3], [8.0, 13.0]);
    }
}
#[test]
fn compatible_updates_retain_clock_bypass_and_unrelated_ramps_and_reset_is_repeatable() {
    for rate in RATES {
        let mut settings = BitcrusherSettings {
            downsampling: 40,
            ..Default::default()
        };
        let mut unit = Bitcrusher::new(context(rate), &settings);
        let held = unit.process_sample([0.25; 2]);
        for _ in 0..7 {
            unit.process_sample([0.5; 2]);
        }
        settings.modulation_open = true;
        unit.retarget(&settings);
        assert_eq!(
            unit.process_sample([0.75; 2]),
            held,
            "presentation changes must retain clock"
        );
        settings.enabled = false;
        unit.retarget(&settings);
        settle(&mut unit, rate, [0.0; 5]);
        let dry = [f32::MAX, f32::from_bits(1)];
        assert_eq!(unit.process_sample(dry), dry);
        settings.enabled = true;
        settings.downsampling = 1;
        settings.resolution_bits = 1;
        settings.level_db = -12.0;
        unit.retarget(&settings);
        settle(&mut unit, rate, [0.0; 5]);
        let output = unit.process_sample([0.2, -0.3]);
        assert!((output[0] - 10_f32.powf(-12.0 / 20.0)).abs() < 1e-7);
        unit.reset();
        let first = unit.process_sample([0.2, -0.3]);
        unit.reset();
        assert_eq!(first, unit.process_sample([0.2, -0.3]));
        unit.reset();
        assert_eq!(unit.process_sample([0.0; 2]), [0.0; 2]);
    }
}
#[test]
fn nonfinite_controls_inputs_and_extreme_headroom_do_not_escape_finite_output() {
    for rate in RATES {
        let mut unit = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                drive_db: 24.0,
                resolution_bits: 1,
                level_db: 12.0,
                dry_wet: 0.5,
                ..Default::default()
            },
        );
        for input in [
            [f32::NAN, f32::INFINITY],
            [f32::NEG_INFINITY, 0.0],
            [f32::MAX, -f32::MAX],
            [1e-30, -1e-30],
        ] {
            let output = unit.process_modulated(
                input,
                [
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NAN,
                    f32::NAN,
                ],
            );
            assert!(output.iter().all(|v| v.is_finite()));
            assert!(unit.effective_parameters().iter().all(|v| v.is_finite()));
        }
        let mut dry = Bitcrusher::new(
            context(rate),
            &BitcrusherSettings {
                dry_wet: 0.0,
                ..Default::default()
            },
        );
        for input in [[0.25, -0.5], [f32::MAX, -f32::MAX], [f32::from_bits(1); 2]] {
            assert_eq!(dry.process_sample(input), input);
        }
    }
}
