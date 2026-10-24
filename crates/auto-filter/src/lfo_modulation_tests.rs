use super::*;
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationContext, ModulationParameter as P, ModulationRange,
    ModulationRoute, ModulationSourceId, ModulationValue, process_modulation,
};

#[test]
fn shared_lfo_controls_match_static_values_at_every_rate_and_wave() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for &wave in daw_unit_core::LfoWave::ALL {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = AutoFilterSettings {
                wave,
                ..Default::default()
            };
            let mut actual = AutoFilter::new(context, &settings);
            let mut expected = AutoFilter::new(
                context,
                &AutoFilterSettings {
                    strength_octaves: 3.0,
                    sync_phase_degrees: 90.0,
                    stereo_phase_degrees: -90.0,
                    ..settings
                },
            );
            let id = ModulationSourceId {
                layer: daw_unit_core::OscillatorSettings::default().id,
                unit: daw_unit_core::OscillatorSettings::default().id,
            };
            let value = ModulationValue {
                source: id,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Continuous,
                release_age: None,
            };
            let mut routes: Vec<_> = [
                (P::AutoFilterStrength, 2.0),
                (P::AutoFilterSyncPhase, 90.0),
                (P::AutoFilterStereoPhase, -90.0),
            ]
            .into_iter()
            .map(|(parameter, amount)| {
                let mut r = Modulation::new(parameter);
                r.amount = amount;
                r.source = Some(id);
                ModulationRoute::new(r, rate)
            })
            .collect();
            for frame in 0..rate / 10 {
                let input = [
                    (frame as f32 * 0.03).sin() * 0.2,
                    (frame as f32 * 0.07).sin() * 0.2,
                ];
                let a = process_modulation(
                    &mut actual,
                    &mut routes,
                    input,
                    &[],
                    &[value],
                    ModulationContext {
                        frame: frame.into(),
                        tempo_bpm: 120.0,
                        ..Default::default()
                    },
                );
                let b = expected.process_at(input, frame.into(), 120.0);
                if frame > rate / 20 {
                    assert!(
                        (a[0] - b[0]).abs() < 0.00002 && (a[1] - b[1]).abs() < 0.00002,
                        "{rate} {wave:?} {frame}: {a:?} {b:?}"
                    );
                }
            }
            let readings = actual.modulation_parameters();
            assert!(
                (readings[5] - 3.0).abs() < 0.001
                    && (readings[6] - 90.0).abs() < 0.001
                    && (readings[7] + 90.0).abs() < 0.001
            );
            actual.reset();
            expected.reset();
            // A voice-local contour must not enter this shared bus destination.
            let mut baseline = AutoFilter::new(
                context,
                &AutoFilterSettings {
                    wave,
                    ..Default::default()
                },
            );
            for route in &mut routes {
                route.reset();
            }
            for frame in 0..512 {
                assert_eq!(
                    process_modulation(
                        &mut actual,
                        &mut routes,
                        [0.2; 2],
                        &[value],
                        &[],
                        ModulationContext {
                            frame,
                            tempo_bpm: 120.0,
                            ..Default::default()
                        }
                    ),
                    baseline.process_at([0.2; 2], frame, 120.0)
                );
            }
        }
    }
}

#[test]
fn lfo_offsets_bound_cancel_and_reset_without_affecting_the_static_filter() {
    let context = ProcessingContext {
        sample_rate_hz: 48000,
        max_block_frames: 512,
    };
    for offsets in [[1e6; 7], [-1e6; 7], [f32::NAN; 7], [f32::INFINITY; 7]] {
        let mut unit = AutoFilter::new(context, &AutoFilterSettings::default());
        for frame in 0..2400 {
            let output = unit.process_offsets_at([0.2; 2], frame, 120.0, offsets);
            assert!(output.iter().all(|v| v.is_finite()));
            let p = unit.modulation_parameters();
            assert!((0.0..=6.0).contains(&p[5]));
            assert!(p[6..].iter().all(|v| (-180.0..=180.0).contains(v)));
        }
        unit.reset();
        assert_eq!(&unit.modulation_parameters()[5..], &[1.0, 0.0, 0.0]);
    }
    let settings = AutoFilterSettings {
        lfo_enabled: false,
        ..Default::default()
    };
    let mut actual = AutoFilter::new(context, &settings);
    let mut expected = AutoFilter::new(context, &settings);
    for frame in 0..4800 {
        assert_eq!(
            actual.process_offsets_at(
                [0.2; 2],
                frame,
                120.0,
                [0.0, 0.0, 0.0, 0.0, 6.0, 180.0, -180.0]
            ),
            expected.process_at([0.2; 2], frame, 120.0)
        );
    }
}

#[test]
fn opposite_shared_routes_cancel_each_new_destination_exactly() {
    let context = ProcessingContext {
        sample_rate_hz: 48000,
        max_block_frames: 512,
    };
    let id = ModulationSourceId {
        layer: daw_unit_core::OscillatorSettings::default().id,
        unit: daw_unit_core::OscillatorSettings::default().id,
    };
    let value = ModulationValue {
        source: id,
        value: 0.8,
        weight: 1.0,
        range: ModulationRange::Bipolar,
        behavior: ModulationBehavior::Continuous,
        release_age: None,
    };
    for parameter in [
        P::AutoFilterStrength,
        P::AutoFilterSyncPhase,
        P::AutoFilterStereoPhase,
    ] {
        let mut routes: Vec<_> = [parameter.limit(), -parameter.limit()]
            .into_iter()
            .map(|amount| {
                let mut r = Modulation::new(parameter);
                r.source = Some(id);
                r.amount = amount;
                ModulationRoute::new(r, 48000)
            })
            .collect();
        let mut actual = AutoFilter::new(context, &AutoFilterSettings::default());
        let mut expected = actual.clone();
        for frame in 0..4800 {
            assert_eq!(
                process_modulation(
                    &mut actual,
                    &mut routes,
                    [0.25; 2],
                    &[],
                    &[value],
                    ModulationContext {
                        frame,
                        tempo_bpm: 120.0,
                        ..Default::default()
                    }
                ),
                expected.process_at([0.25; 2], frame, 120.0)
            );
        }
    }
}
