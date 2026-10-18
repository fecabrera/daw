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
fn tone(frame: u32, hz: f32, rate: u32) -> f32 {
    (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.1
}
fn side_energy(rate: u32, hz: f32, settings: &StereoSpreadSettings) -> f64 {
    let mut unit = StereoSpread::new(context(rate), settings);
    let mut sum = 0.0;
    for frame in 0..rate / 2 {
        let x = tone(frame, hz, rate);
        let [l, r] = unit.process_sample([x; 2]);
        assert!(((l + r) * 0.5 - x).abs() < 1e-6, "mono fold-down changed");
        if frame > rate / 4 {
            sum += f64::from((l - r) * 0.5).powi(2);
        }
    }
    sum
}
#[test]
fn mono_widens_in_the_selected_range_and_zero_intensity_and_bypass_are_exact() {
    for rate in RATES {
        let mut s = StereoSpreadSettings {
            lower_intensity: 1.0,
            upper_intensity: 1.0,
            lower_frequency_hz: 300.0,
            upper_frequency_hz: 6000.0,
            ..Default::default()
        };
        for order in [2, 8, 12] {
            s.order = order;
            let inside = side_energy(rate, 1000.0, &s);
            assert!(inside > 0.001);
            assert!(side_energy(rate, 20.0, &s) < inside * 0.001);
            assert!(side_energy(rate, 18000.0, &s) < inside * 0.01);
        }
        s.lower_intensity = 0.0;
        s.upper_intensity = 0.0;
        let mut unit = StereoSpread::new(context(rate), &s);
        for input in [[0.25, -0.5], [f32::MAX, -f32::MAX], [f32::from_bits(1); 2]] {
            assert_eq!(unit.process_sample(input), input);
        }
        s.lower_intensity = 1.0;
        s.upper_intensity = 1.0;
        s.enabled = false;
        unit.retarget(&s);
        for _ in 0..rate / 100 {
            unit.process_sample([0.5, 0.25]);
        }
        assert_eq!(unit.process_sample([0.5, 0.25]), [0.5, 0.25]);
        unit.reset();
        assert_eq!(unit.process_sample([0.0; 2]), [0.0; 2]);
    }
}
#[test]
fn endpoint_intensities_control_opposite_ends_and_order_changes_the_pattern() {
    for rate in RATES {
        let lower = StereoSpreadSettings {
            lower_intensity: 1.0,
            upper_intensity: 0.0,
            lower_frequency_hz: 500.0,
            upper_frequency_hz: 8000.0,
            order: 2,
            ..Default::default()
        };
        let upper = StereoSpreadSettings {
            lower_intensity: 0.0,
            upper_intensity: 1.0,
            ..lower.clone()
        };
        assert!(side_energy(rate, 1000.0, &lower) > side_energy(rate, 1000.0, &upper) * 4.0);
        assert!(side_energy(rate, 4000.0, &upper) > side_energy(rate, 4000.0, &lower) * 4.0);
        let dense = StereoSpreadSettings {
            order: 12,
            ..lower.clone()
        };
        assert!(
            (side_energy(rate, 2000.0, &lower) - side_energy(rate, 2000.0, &dense)).abs() > 0.01
        );
    }
}

#[test]
fn neighboring_midrange_bands_favor_alternating_output_channels() {
    for rate in RATES {
        for order in [2, 8, 12] {
            let s = StereoSpreadSettings {
                lower_intensity: 1.0,
                upper_intensity: 1.0,
                lower_frequency_hz: 100.0,
                upper_frequency_hz: 10000.0,
                order,
                ..Default::default()
            };
            let indices = if order == 2 {
                0..2
            } else {
                order / 2 - 2..order / 2 + 2
            };
            for index in indices {
                let frequency = 100.0 * 100_f32.powf((f32::from(index) + 0.5) / f32::from(order));
                let mut unit = StereoSpread::new(context(rate), &s);
                let mut energies = [0_f64; 2];
                for frame in 0..rate / 5 {
                    let output = unit.process_sample([tone(frame, frequency, rate); 2]);
                    if frame > rate / 10 {
                        for ch in 0..2 {
                            energies[ch] += f64::from(output[ch]).powi(2);
                        }
                    }
                }
                assert_eq!(
                    energies[0] > energies[1],
                    index % 2 == 0,
                    "rate={rate} order={order} band={index}: {energies:?}"
                );
            }
        }
    }
}
#[test]
fn shared_modulation_matches_base_settings_and_local_routes_are_excluded() {
    for rate in RATES {
        let source = ModulationValue {
            source: ModulationSourceId {
                layer: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
                unit: "00000000-0000-0000-0000-000000000002".parse().unwrap(),
            },
            value: 1.0,
            weight: 1.0,
            release_age: None,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
        };
        let mut unit = StereoSpread::new(context(rate), &StereoSpreadSettings::default());
        let mut reference = StereoSpread::new(
            context(rate),
            &StereoSpreadSettings {
                lower_intensity: 0.4,
                upper_intensity: 0.7,
                lower_frequency_hz: 600.0,
                upper_frequency_hz: 10000.0,
                ..Default::default()
            },
        );
        let mut dry_routes: Vec<_> = ModulationParameter::STEREO_SPREAD
            .into_iter()
            .zip([0.4, 0.7, 30_f32.log2(), -1.0])
            .map(|(parameter, amount)| {
                ModulationRoute::new(
                    Modulation {
                        source: Some(source.source),
                        amount,
                        ..Modulation::new(parameter)
                    },
                    rate,
                )
            })
            .collect();
        let mut neutral = StereoSpread::new(context(rate), &StereoSpreadSettings::default());
        for frame in 0..rate / 5 {
            let input = [tone(frame, 1700.0, rate); 2];
            assert_eq!(
                process_modulation(
                    &mut unit,
                    &mut dry_routes,
                    input,
                    &[source],
                    &[],
                    ModulationContext::default()
                ),
                neutral.process_sample(input)
            );
        }
        unit.reset();
        for frame in 0..rate / 2 {
            let input = [tone(frame, 1700.0, rate); 2];
            let a = process_modulation(
                &mut unit,
                &mut dry_routes,
                input,
                &[],
                &[source],
                ModulationContext::default(),
            );
            let b = reference.process_sample(input);
            if frame > rate / 4 {
                for ch in 0..2 {
                    assert!((a[ch] - b[ch]).abs() < 2e-5, "{rate}: {a:?} {b:?}");
                }
            }
        }
        let p = unit.effective_parameters();
        for (actual, expected) in p.into_iter().zip([0.4, 0.7, 600.0, 10000.0]) {
            assert!((actual - expected).abs() < 0.02);
        }
    }
}
#[test]
fn rapid_order_and_frequency_changes_stay_finite_preserve_ramps_and_reset() {
    for rate in RATES {
        let mut s = StereoSpreadSettings::default();
        let mut unit = StereoSpread::new(context(rate), &s);
        for frame in 0..rate / 10 {
            if frame % 31 == 0 {
                s.order = 2 + (frame % 11) as u8;
                s.lower_frequency_hz = 20.0 + (frame % 19000) as f32;
                s.upper_frequency_hz = 20_000.0;
                unit.retarget(&s);
            }
            let output = unit.process_modulated([0.1, -0.05], [1.0, -1.0, 6.0, -6.0]);
            assert!(output.iter().all(|v| v.is_finite() && v.abs() < 5.0));
            let p = unit.effective_parameters();
            assert!((-1.0..=1.0).contains(&p[0]) && (-1.0..=1.0).contains(&p[1]));
            assert!(p[2] >= 20.0 && p[3] <= 20000.0 && p[3] - p[2] >= 1.0);
        }
        s.order = 12;
        unit.retarget(&s);
        for _ in 0..rate / 50 {
            unit.process_sample([0.0; 2]);
        }
        assert_eq!(unit.banks[unit.active].order, 12);
        unit.reset();
        assert_eq!(unit.process_sample([f32::NAN, f32::INFINITY]), [0.0; 2]);
        for _ in 0..1000 {
            assert!(
                unit.process_modulated(
                    [f32::MAX; 2],
                    [f32::MAX, f32::NAN, f32::INFINITY, -f32::MAX]
                )
                .iter()
                .all(|v| v.is_finite())
            );
        }
    }
}

#[test]
fn signed_intensities_reverse_the_generated_pattern_and_preserve_mono_sum() {
    for rate in RATES {
        let positive = StereoSpreadSettings {
            lower_intensity: 0.5,
            upper_intensity: 0.5,
            ..Default::default()
        };
        let negative = StereoSpreadSettings {
            lower_intensity: -0.5,
            upper_intensity: -0.5,
            ..positive.clone()
        };
        let mut a = StereoSpread::new(context(rate), &positive);
        let mut b = StereoSpread::new(context(rate), &negative);
        let mut crossed = StereoSpread::new(
            context(rate),
            &StereoSpreadSettings {
                lower_intensity: -1.0,
                upper_intensity: 1.0,
                ..positive
            },
        );
        for frame in 0..rate / 5 {
            let input = [tone(frame, 700.0, rate), tone(frame, 1900.0, rate)];
            let x = a.process_sample(input);
            let y = b.process_sample(input);
            let z = crossed.process_sample(input);
            for ch in 0..2 {
                assert!((x[ch] + y[ch] - 2.0 * input[ch]).abs() < 1e-6);
            }
            assert!((z.iter().sum::<f32>() - input.iter().sum::<f32>()).abs() < 1e-6);
        }
        // Signed route offsets cross zero and reach the negative endpoint without
        // altering the saved zero defaults or the unprocessed stereo contribution.
        let mut unit = StereoSpread::new(context(rate), &StereoSpreadSettings::default());
        for _ in 0..rate / 10 {
            unit.process_modulated([0.0; 2], [-2.0, -2.0, 0.0, 0.0]);
        }
        assert_eq!(&unit.effective_parameters()[..2], &[-1.0; 2]);
    }
}
