use super::*;
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationContext, ModulationDestination,
    ModulationDestinationSettings, ModulationParameter as P, ModulationRange, ModulationRoute,
    ModulationScope, ModulationSourceId, ModulationValue, process_modulation,
};

fn source() -> ModulationValue {
    ModulationValue {
        source: ModulationSourceId {
            layer: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            unit: "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        },
        value: 1.0,
        weight: 1.0,
        release_age: None,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Contour,
    }
}
fn routes(rate: u32, amounts: [f32; 4]) -> Vec<ModulationRoute> {
    P::LIMITER
        .into_iter()
        .zip(amounts)
        .map(|(parameter, amount)| {
            ModulationRoute::new(
                Modulation {
                    source: Some(source().source),
                    amount,
                    ..Modulation::new(parameter)
                },
                rate,
            )
        })
        .collect()
}
fn process(
    unit: &mut Limiter,
    routes: &mut [ModulationRoute],
    input: [f32; 2],
    local: bool,
) -> [f32; 2] {
    let values = [source()];
    let (local, shared) = if local {
        (&values[..], &[][..])
    } else {
        (&[][..], &values[..])
    };
    process_modulation(
        unit,
        routes,
        input,
        local,
        shared,
        ModulationContext::default(),
    )
}
#[test]
fn typed_modulation_preserves_neutral_audio_scope_bounds_reset_bypass_and_fixed_latency() {
    for rate in crate::tests::RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let settings = LimiterSettings::default();
        let mut unit = Limiter::new(context, &settings);
        let mut reference = Limiter::new(context, &settings);
        assert!(unit.descriptor().consumes_modulation());
        assert_eq!(
            Limiter::MODULATION_PARAMETERS,
            LimiterSettings::MODULATION_PARAMETERS
        );
        assert_eq!(Limiter::MODULATION_SCOPE, ModulationScope::Shared);
        let mut cancelled = routes(rate, [12.0, 1.0, 4.0, -6.0]);
        cancelled.extend(routes(rate, [-12.0, -1.0, -4.0, 6.0]));
        for frame in 0..2000 {
            let input = [
                (frame as f32 * 0.13).sin() * 2.0,
                (frame as f32 * 0.17).cos(),
            ];
            assert_eq!(
                process(&mut unit, &mut cancelled, input, false),
                reference.process_sample(input)
            );
        }
        let mut incoming = routes(rate, [12.0, 1.0, 4.0, -6.0]);
        for _ in 0..100 {
            assert_eq!(
                process(&mut unit, &mut incoming, [0.3, -0.2], true),
                reference.process_sample([0.3, -0.2])
            );
        }
        process(&mut unit, &mut incoming, [1.0; 2], false);
        let first = unit.effective_parameters();
        assert!(first[0] > 0.0 && first[0] < 12.0 && first[1] > 100.0 && first[1] < 200.0);
        assert!(first[2] > 1.0 && first[2] < 5.0 && first[3] < -1.0 && first[3] > -7.0);
        for _ in 0..rate / 50 {
            process(&mut unit, &mut incoming, [1.0, -0.5], false);
        }
        for (actual, expected) in unit
            .effective_parameters()
            .into_iter()
            .zip([12.0, 200.0, 5.0, -7.0])
        {
            assert!((actual - expected).abs() < 0.001);
        }
        let latency = unit.latency_frames();
        for offsets in [
            [f32::MAX; 4],
            [f32::MIN; 4],
            [f32::NAN; 4],
            [f32::INFINITY; 4],
        ] {
            for _ in 0..100 {
                assert!(
                    unit.process_modulated([f32::MAX, -f32::MAX], offsets)
                        .iter()
                        .all(|v| v.is_finite())
                );
                let p = unit.effective_parameters();
                assert!((-24.001..=24.001).contains(&p[0]) && (1.0..=2000.0).contains(&p[1]));
                assert!((0.0..=10.0).contains(&p[2]) && (-24.001..=0.001).contains(&p[3]));
                assert_eq!(unit.latency_frames(), latency);
            }
        }
        unit.reset();
        reference.reset();
        for _ in 0..100 {
            assert_eq!(
                unit.process_sample([0.1; 2]),
                reference.process_sample([0.1; 2])
            );
        }
        let settings = LimiterSettings {
            enabled: false,
            mode: LimiterMode::TruePeak,
            ..settings
        };
        unit.retarget(&settings);
        reference.retarget(&settings);
        unit.reset();
        reference.reset();
        for _ in 0..2000 {
            assert_eq!(
                process(&mut unit, &mut incoming, [2.0, -1.0], false),
                reference.process_sample([2.0, -1.0])
            );
        }
        assert_eq!(unit.latency_frames(), latency);
    }
}
#[test]
fn modulated_ceiling_retains_peak_guard_and_fixed_setting_reconstruction_behavior() {
    let rate = 48000;
    for mode in [LimiterMode::Peak, LimiterMode::TruePeak] {
        for routing in [LimiterRouting::LeftRight, LimiterRouting::MidSide] {
            let settings = LimiterSettings {
                mode,
                routing,
                ..Default::default()
            };
            let mut unit = Limiter::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            let mut incoming = routes(rate, [12.0, -1.0, 2.0, -5.0]);
            // Settle controls before the burst; emitted history cannot follow a retrospective ceiling.
            for _ in 0..rate / 20 {
                process(&mut unit, &mut incoming, [0.0; 2], false);
            }
            let mut fixed = Limiter::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &LimiterSettings {
                    input_db: 12.0,
                    release_ms: 50.0,
                    lookahead_ms: 3.0,
                    ceiling_db: -6.0,
                    ..settings.clone()
                },
            );
            for _ in 0..rate / 20 {
                fixed.process_sample([0.0; 2]);
            }
            let mut fixed_output = Vec::new();
            let mut output = Vec::new();
            for frame in 0..2048 + unit.latency_frames() {
                let input = if frame < 2048 {
                    let x = (frame as f64 * 0.47 * std::f64::consts::TAU).sin() as f32 * 3.0;
                    [x, -0.25 * x]
                } else {
                    [0.0; 2]
                };
                output.push(process(&mut unit, &mut incoming, input, false));
                fixed_output.push(fixed.process_sample(input));
            }
            let ceiling = amplitude(-6.0);
            assert!(
                output
                    .iter()
                    .flatten()
                    .all(|v| f64::from(v.abs()) <= ceiling + 1e-6)
            );
            if mode == LimiterMode::TruePeak {
                let reconstructed = crate::tests::reconstructed_peak(&output);
                let fixed_peak = crate::tests::reconstructed_peak(&fixed_output);
                // This near-Nyquist M/S burst exceeds the guard's estimate with fixed settings too.
                // Preserve its existing reconstruction behavior; do not assert an exact analog bound.
                assert!(
                    (reconstructed - fixed_peak).abs() < 1e-6,
                    "{routing:?}: {reconstructed} vs {fixed_peak}"
                );
            }
            if routing == LimiterRouting::LeftRight {
                assert!(output.iter().all(|s| (s[0] + s[1] * 4.0).abs() < 1e-6));
            }
            assert!(unit.gain_reduction_db() > 0.0);
        }
    }
}
