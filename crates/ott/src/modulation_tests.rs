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
fn routes(rate: u32, amounts: [f32; 19]) -> Vec<ModulationRoute> {
    P::OTT
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
    unit: &mut Ott,
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
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn amounts() -> [f32; 19] {
    [
        1.0, -1.0, -0.5, 6.0, -3.0, 1.0, -1.0, -0.25, -6.0, 3.0, -1.0, 1.0, -0.75, 3.0, 6.0, 2.0,
        -2.0, 3.0, -0.25,
    ]
}
#[test]
fn typed_inputs_preserve_neutral_audio_reject_local_sources_and_match_fixed_controls() {
    for rate in RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let settings = OttSettings::default();
        let mut unit = Ott::new(context, &settings);
        let mut reference = Ott::new(context, &settings);
        assert!(unit.descriptor().consumes_modulation());
        assert!(!unit.descriptor().provides_modulation());
        assert_eq!(
            Ott::MODULATION_PARAMETERS,
            OttSettings::MODULATION_PARAMETERS
        );
        assert_eq!(Ott::MODULATION_SCOPE, ModulationScope::Shared);
        let mut cancelled = routes(rate, amounts());
        cancelled.extend(routes(rate, amounts().map(|a| -a)));
        for frame in 0..2000 {
            let input = [
                (frame as f32 * 0.13).sin() * 0.2,
                (frame as f32 * 0.17).cos() * 0.1,
            ];
            assert_eq!(
                process(&mut unit, &mut cancelled, input, false),
                reference.process_sample(input)
            );
        }
        let mut incoming = routes(rate, amounts());
        for _ in 0..100 {
            assert_eq!(
                process(&mut unit, &mut incoming, [0.3, -0.2], true),
                reference.process_sample([0.3, -0.2])
            );
        }
        unit.reset();
        process(&mut unit, &mut incoming, [0.0; 2], false);
        let first = unit.effective_parameters();
        assert!(first[0] > 250.0 && first[0] < 500.0);
        assert!(first[2] > 0.5 && first[2] < 1.0);
        for _ in 0..rate / 20 {
            process(&mut unit, &mut incoming, [0.0; 2], false);
        }
        let mut fixed_settings = OttSettings {
            low_frequency_hz: 500.0,
            high_frequency_hz: 1250.0,
            level_db: 3.0,
            dry_wet: 0.75,
            ..settings.clone()
        };
        for (b, values) in fixed_settings.bands.iter_mut().zip([
            [0.5, 6.0, -3.0, 20.0, 50.0],
            [0.75, -6.0, 3.0, 5.0, 200.0],
            [0.25, 3.0, 6.0, 40.0, 25.0],
        ]) {
            b.depth = values[0];
            b.input_db = values[1];
            b.output_db = values[2];
            b.attack_ms = values[3];
            b.release_ms = values[4];
        }
        let mut fixed = Ott::new(context, &fixed_settings);
        for (actual, expected) in unit
            .effective_parameters()
            .into_iter()
            .zip(fixed.effective_parameters())
        {
            assert!(
                (actual - expected).abs() < 0.002,
                "{rate}: {actual} vs {expected}"
            );
        }
        for frame in 0..1000 {
            let x = (frame as f32 * 0.12).sin() * 0.2;
            let actual = process(&mut unit, &mut incoming, [x, -0.25 * x], false);
            let expected = fixed.process_sample([x, -0.25 * x]);
            assert!((actual[0] - expected[0]).abs() < 0.0001);
            assert!((actual[0] + 4.0 * actual[1]).abs() < 0.00001);
        }
        let current = unit.effective_parameters();
        let mut presentation = settings.clone();
        presentation.modulation_open = true;
        unit.retarget(&presentation);
        assert_eq!(unit.effective_parameters(), current);
        unit.reset();
        reference.reset();
        for _ in 0..100 {
            assert_eq!(
                unit.process_sample([0.2, -0.1]),
                reference.process_sample([0.2, -0.1])
            );
        }
    }
}
#[test]
fn modulation_bounds_keep_crossovers_ordered_meters_honest_and_bypass_and_silence_exact() {
    for rate in RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let settings = OttSettings::default();
        let mut unit = Ott::new(context, &settings);
        let mut crossed = [0.0; 19];
        crossed[0] = 6.0;
        crossed[1] = -6.0;
        for _ in 0..rate / 50 {
            unit.process_modulated([0.1, -0.025], crossed);
        }
        let p = unit.effective_parameters();
        assert!((p[0] - 2000.0).abs() < 0.01 && (p[1] - 2001.0).abs() < 0.01);
        for offsets in [
            [f32::MAX; 19],
            [f32::MIN; 19],
            [f32::NAN; 19],
            [f32::INFINITY; 19],
        ] {
            for _ in 0..100 {
                assert!(
                    unit.process_modulated([f32::MAX, -f32::MAX], offsets)
                        .iter()
                        .all(|v| v.is_finite())
                );
                let p = unit.effective_parameters();
                assert!(p.iter().all(|v| v.is_finite()));
                assert!((19.99..=2000.01).contains(&p[0]) && (199.99..=20000.01).contains(&p[1]));
                assert!(p[0] < p[1]);
                for i in 0..3 {
                    let b = &p[2 + 5 * i..7 + 5 * i];
                    assert!((0.0..=1.0).contains(&b[0]));
                    assert!(b[1..3].iter().all(|v| (-24.0..=24.0).contains(v)));
                    assert!((0.099..=200.01).contains(&b[3]) && (9.99..=2000.01).contains(&b[4]));
                }
                assert!(p[17] <= 12.001 && (0.0..=1.0).contains(&p[18]));
                assert!(unit.gain_change_db().iter().all(|v| v.is_finite()));
            }
        }
        let off = OttSettings {
            enabled: false,
            ..settings.clone()
        };
        unit.retarget(&off);
        unit.reset();
        for _ in 0..100 {
            assert_eq!(unit.process_modulated([2.0, -1.0], amounts()), [2.0, -1.0]);
        }
        let silent = OttSettings {
            level_db: daw_unit_core::SILENT_GAIN_DB,
            ..settings
        };
        unit.retarget(&silent);
        unit.reset();
        for _ in 0..100 {
            assert_eq!(unit.process_modulated([0.5, -0.125], [24.0; 19]), [0.0; 2]);
        }
    }
}

#[test]
fn crossover_sweeps_preserve_order_during_base_edits_and_depth_meters_ignore_mix_gain() {
    for rate in RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut settings = OttSettings::default();
        let mut unit = Ott::new(context, &settings);
        for frame in 0..3000 {
            if frame % 73 == 0 {
                let upper = frame % 146 == 0;
                settings.low_frequency_hz = if upper { 1800.0 } else { 100.0 };
                settings.high_frequency_hz = if upper { 2000.0 } else { 250.0 };
                unit.retarget(&settings);
            }
            let mut offsets = [0.0; 19];
            offsets[0] = (frame as f32 * 0.013).cos() * 6.0;
            offsets[1] = -offsets[0];
            assert!(
                unit.process_modulated([0.1, -0.025], offsets)
                    .iter()
                    .all(|v| v.is_finite())
            );
            let p = unit.effective_parameters();
            assert!(p[0] < p[1]);
            assert!(p[1] - p[0] >= 0.99);
        }
        let mut full = Ott::new(context, &OttSettings::default());
        let mut half = Ott::new(context, &OttSettings::default());
        let mut offsets = [0.0; 19];
        for i in [2, 7, 12] {
            offsets[i] = -0.5;
        }
        offsets[17] = 6.0;
        offsets[18] = -1.0;
        for frame in 0..rate / 20 {
            let x = (frame as f32 * 0.08).sin() * 0.1;
            full.process_sample([x, -0.25 * x]);
            half.process_modulated([x, -0.25 * x], offsets);
        }
        assert!(full.gain_change_db().iter().any(|v| v.abs() > 0.1));
        for (actual, original) in half.gain_change_db().into_iter().zip(full.gain_change_db()) {
            assert!((actual - original * 0.5).abs() < 0.001);
        }
    }
}
