use super::*;
use daw_unit_core::{
    Modulation, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationParameter as P, ModulationRange, ModulationRoute, ModulationSourceId,
    ModulationValue, process_modulation,
};
fn source() -> ModulationSourceId {
    ModulationSourceId {
        layer: Modulation::new(P::Level).id,
        unit: Modulation::new(P::Level).id,
    }
}
fn value(source: ModulationSourceId) -> ModulationValue {
    ModulationValue {
        source,
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Unipolar,
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
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 32,
    }
}
fn close(actual: [f32; 7], expected: [f32; 7]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!(
            (a - b).abs() < b.abs().max(1.0) * 1e-4,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn compressor_typed_modulation_preserves_neutral_audio_and_controls_every_target_at_all_rates() {
    assert_eq!(
        Compressor::MODULATION_PARAMETERS,
        CompressorSettings::MODULATION_PARAMETERS
    );
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let settings = CompressorSettings::default();
        let mut unit = Compressor::new(context(rate), &settings);
        assert!(unit.descriptor().consumes_modulation());
        let mut reference = Compressor::new(context(rate), &settings);
        let source = source();
        let mut cancel: Vec<_> = P::COMPRESSOR
            .into_iter()
            .flat_map(|p| {
                [
                    route(source, p, p.limit(), rate),
                    route(source, p, -p.limit(), rate),
                ]
            })
            .collect();
        for frame in 0..1024 {
            let input = [(frame as f32 * 0.17).sin() * 0.4, 0.2];
            assert_eq!(
                process_modulation(
                    &mut unit,
                    &mut cancel,
                    input,
                    &[],
                    &[value(source)],
                    ModulationContext::default()
                ),
                reference.process_sample(input)
            );
        }
        let amounts = [-12.0, 2.0, 1.0, -1.0, 3.0, 6.0, -0.25];
        let mut routes: Vec<_> = P::COMPRESSOR
            .into_iter()
            .zip(amounts)
            .map(|(p, a)| route(source, p, a, rate))
            .collect();
        // Shared destinations reject voice-local buffers through the common entry point.
        process_modulation(
            &mut unit,
            &mut routes,
            [0.0; 2],
            &[value(source)],
            &[],
            ModulationContext::default(),
        );
        close(
            unit.effective_parameters(),
            [-18.0, 4.0, 10.0, 100.0, 6.0, 0.0, 1.0],
        );
        unit.reset();
        reference.reset();
        for frame in 0..rate / 50 {
            let input = [(frame as f32 * 0.17).sin() * 0.4, 0.2];
            let output = process_modulation(
                &mut unit,
                &mut routes,
                input,
                &[],
                &[value(source)],
                ModulationContext::default(),
            );
            assert_eq!(output, reference.process_modulated(input, amounts));
            assert!(output.iter().all(|v| v.is_finite()));
            if frame == 0 {
                assert!(unit.effective_parameters()[0] > -30.0);
            }
        }
        close(
            unit.effective_parameters(),
            [-30.0, 6.0, 20.0, 50.0, 9.0, 6.0, 0.75],
        );
        assert!(unit.gain_reduction_db() > 0.0);
        let mut presentation = settings.clone();
        presentation.modulation_open = true;
        unit.retarget(&presentation);
        for _ in 0..128 {
            assert_eq!(
                unit.process_modulated([0.3, 0.15], amounts),
                reference.process_modulated([0.3, 0.15], amounts)
            );
        }
        assert_eq!(settings, CompressorSettings::default());
    }
}

#[test]
fn compressor_modulation_bounds_nonfinite_offsets_silence_and_bypass_are_safe() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut settings = CompressorSettings::default();
        let mut unit = Compressor::new(context(rate), &settings);
        for offsets in [[1e6; 7], [-1e6; 7], [f32::NAN; 7], [f32::INFINITY; 7]] {
            unit.reset();
            for _ in 0..rate / 50 {
                assert!(
                    unit.process_modulated([0.5, 0.25], offsets)
                        .iter()
                        .all(|v| v.is_finite())
                );
            }
            let expected = if offsets[0].is_finite() {
                if offsets[0] > 0.0 {
                    [0.0, 20.0, 200.0, 2000.0, 24.0, 12.0, 1.0]
                } else {
                    [-60.0, 1.0, 0.1, 10.0, 0.0, -96.0, 0.0]
                }
            } else {
                [-18.0, 4.0, 10.0, 100.0, 6.0, 0.0, 1.0]
            };
            close(unit.effective_parameters(), expected);
        }
        settings.enabled = false;
        unit.retarget(&settings);
        for _ in 0..rate / 200 {
            unit.process_modulated([0.5, 0.25], [1.0; 7]);
        }
        assert_eq!(unit.process_modulated([0.5, 0.25], [1.0; 7]), [0.5, 0.25]);
        settings.enabled = true;
        settings.level_db = daw_unit_core::SILENT_GAIN_DB;
        unit.retarget(&settings);
        unit.reset();
        for _ in 0..128 {
            assert_eq!(unit.process_modulated([0.5, 0.25], [1.0; 7]), [0.0; 2]);
        }
        assert_eq!(
            unit.effective_parameters()[5],
            daw_unit_core::SILENT_GAIN_DB
        );
    }
}

#[test]
fn time_modulation_changes_detector_response_and_read_only_reduction() {
    let mut base = Compressor::new(context(48000), &CompressorSettings::default());
    let mut modulated = Compressor::new(context(48000), &CompressorSettings::default());
    let offsets = [0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
    for _ in 0..480 {
        base.process_sample([0.5; 2]);
        modulated.process_modulated([0.5; 2], offsets);
    }
    assert!(modulated.gain_reduction_db() < base.gain_reduction_db());
    for _ in 0..48000 {
        base.process_sample([0.5; 2]);
        modulated.process_modulated([0.5; 2], offsets);
    }
    for _ in 0..2400 {
        base.process_sample([0.0; 2]);
        modulated.process_modulated([0.0; 2], offsets);
    }
    assert!(modulated.gain_reduction_db() < base.gain_reduction_db());
}
