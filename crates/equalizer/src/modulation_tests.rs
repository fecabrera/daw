use super::*;
use daw_unit_core::{
    EnvelopeInstance, Modulation, ModulationContext, ModulationDestination,
    ModulationDestinationSettings, ModulationParameter as P, ModulationRange, ModulationRoute,
    ModulationSourceId, ModulationValue, process_modulation,
};

fn source() -> ModulationSourceId {
    ModulationSourceId {
        layer: EnvelopeInstance::default().id,
        unit: EnvelopeInstance::default().id,
    }
}
fn value(source: ModulationSourceId, value: f32) -> ModulationValue {
    ModulationValue {
        source,
        value,
        weight: 1.0,
        range: ModulationRange::Bipolar,
        behavior: daw_unit_core::ModulationBehavior::Continuous,
        release_age: None,
    }
}
fn route(source: ModulationSourceId, parameter: P, amount: f32, rate: u32) -> ModulationRoute {
    ModulationRoute::new(
        Modulation {
            source: Some(source),
            amount,
            ..Modulation::new(parameter)
        },
        rate,
    )
}

#[test]
fn eq_typed_modulation_resolves_every_target_and_preserves_neutral_audio_at_all_rates() {
    assert_eq!(
        Equalizer::MODULATION_PARAMETERS,
        EqSettings::MODULATION_PARAMETERS
    );
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 32,
        };
        let settings = EqSettings::default();
        let mut eq = Equalizer::new(context, &settings);
        assert!(eq.descriptor().consumes_modulation());
        let mut reference = Equalizer::new(context, &settings);
        let source = source();
        let mut routes: Vec<_> = P::EQUALIZER
            .into_iter()
            .flat_map(|p| {
                [
                    route(source, p, p.limit(), rate),
                    route(source, p, -p.limit(), rate),
                ]
            })
            .collect();
        for frame in 0..2048 {
            let input = [(frame as f32 * 0.07).sin() * 0.01, 0.005];
            assert_eq!(
                process_modulation(
                    &mut eq,
                    &mut routes,
                    input,
                    &[],
                    &[value(source, 0.5)],
                    ModulationContext::default()
                ),
                reference.process_sample(input)
            );
        }
        let mut routes: Vec<_> = P::EQUALIZER
            .into_iter()
            .map(|p| route(source, p, p.limit(), rate))
            .collect();
        // Per-note buffers cannot reach this shared destination, even for standalone callers.
        for _ in 0..32 {
            process_modulation(
                &mut eq,
                &mut routes,
                [0.0; 2],
                &[value(source, 0.5)],
                &[],
                ModulationContext::default(),
            );
        }
        assert_eq!(
            eq.effective_parameters(),
            [
                0.0,
                250.0,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                2500.0,
                std::f32::consts::FRAC_1_SQRT_2
            ]
        );
        process_modulation(
            &mut eq,
            &mut routes,
            [0.0; 2],
            &[],
            &[value(source, 0.5)],
            ModulationContext::default(),
        );
        assert!(eq.effective_parameters()[0] > 0.0 && eq.effective_parameters()[0] < 12.0);
        for _ in 0..rate / 50 {
            let output = process_modulation(
                &mut eq,
                &mut routes,
                [0.001; 2],
                &[],
                &[value(source, 0.5)],
                ModulationContext::default(),
            );
            assert!(output.iter().all(|v| v.is_finite()));
        }
        let expected = [
            12.0,
            2000.0,
            std::f32::consts::FRAC_1_SQRT_2 + 5.0,
            12.0,
            12.0,
            20000.0,
            std::f32::consts::FRAC_1_SQRT_2 + 5.0,
        ];
        for (actual, expected) in eq.effective_parameters().into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < expected.abs().max(1.0) * 1e-4,
                "rate={rate}: {actual} != {expected}"
            );
        }
        assert_eq!(settings, EqSettings::default());
    }
}

#[test]
fn eq_modulation_bounds_reset_bypass_cuts_and_history_remain_finite() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut settings = EqSettings::default();
        let mut a = Equalizer::new(context, &settings);
        let mut b = Equalizer::new(context, &settings);
        for offsets in [[1e6; 7], [-1e6; 7], [f32::NAN; 7], [f32::INFINITY; 7]] {
            a.reset();
            b.reset();
            for frame in 0..rate / 50 {
                let input = [(frame as f32 * 0.17).sin() * 0.001, 0.0];
                assert_eq!(
                    a.process_modulated(input, offsets),
                    b.process_modulated(input, offsets)
                );
            }
            let expected = if offsets[0].is_finite() {
                if offsets[0] > 0.0 {
                    [24.0, 20000.0, 10.0, 24.0, 24.0, 20000.0, 10.0]
                } else {
                    [-24.0, 20.0, 0.1, -24.0, -24.0, 20.0, 0.1]
                }
            } else {
                [
                    0.0,
                    250.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    2500.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ]
            };
            assert_eq!(a.effective_parameters(), expected);
        }
        settings.modulation_open = true;
        a.retarget(&settings);
        for _ in 0..512 {
            assert_eq!(
                a.process_modulated([0.001; 2], [1.0; 7]),
                b.process_modulated([0.001; 2], [1.0; 7])
            );
        }
        settings.enabled = false;
        a.retarget(&settings);
        for _ in 0..rate / 200 {
            a.process_modulated([0.1; 2], [1.0; 7]);
        }
        assert_eq!(a.process_modulated([0.1; 2], [1.0; 7]), [0.1; 2]);
        settings.enabled = true;
        for band in &mut settings.bands {
            band.enabled = false;
        }
        a.retarget(&settings);
        a.reset();
        for _ in 0..512 {
            assert_eq!(a.process_modulated([0.1; 2], [1.0; 7]), [0.0; 2]);
        }
    }
}
