use super::*;
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn tick(
    unit: &mut Macro,
    routes: &mut [ModulationRoute],
    values: &[ModulationValue],
    frame: u64,
) -> f32 {
    process_modulation(
        unit,
        routes,
        (),
        &[],
        values,
        ModulationContext {
            frame,
            ..Default::default()
        },
    );
    unit.next_modulation(ModulationSourceContext {
        frame,
        ..Default::default()
    })
}
#[test]
fn value_bounds_base_ramps_and_reset_work_at_every_processing_rate() {
    for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
        let mut unit = Macro::new(MacroSettings::default(), context(rate));
        assert!(unit.descriptor().provides_modulation() && unit.descriptor().consumes_modulation());
        unit.set_settings(MacroSettings {
            value: 1.0,
            ..Default::default()
        });
        let frames = rate / 200;
        let mut last = 0.0;
        for frame in 0..frames {
            let value = tick(&mut unit, &mut [], &[], u64::from(frame));
            assert!((last..=1.0).contains(&value));
            if frame + 1 < frames {
                assert!(value < 1.0);
            }
            last = value;
        }
        assert_eq!(last, 1.0);
        unit.set_settings(MacroSettings {
            value: 0.25,
            ..Default::default()
        });
        unit.reset();
        assert_eq!(tick(&mut unit, &mut [], &[], 900_000), 0.25);
    }
}
#[test]
fn targeted_offsets_reuse_scope_smoothing_signed_cancellation_and_finite_bounds() {
    let a = MacroInstance::default();
    let b = MacroInstance::default();
    let source = ModulationSourceId {
        layer: a.id,
        unit: b.id,
    };
    let value = ModulationValue {
        source,
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Bipolar,
        behavior: ModulationBehavior::Continuous,
        release_age: None,
    };
    let mut route = Modulation::new(ModulationParameter::MacroValue);
    route.target = Some(a.id);
    route.source = Some(source);
    route.amount = 1.0;
    let mut unit = Macro::from_settings(&a, context(48_000));
    let mut other = Macro::from_settings(&b, context(48_000));
    let mut routes = [ModulationRoute::new(route, 48_000)];
    assert!(tick(&mut unit, &mut routes, &[value], 0) < 0.03);
    for frame in 1..1000 {
        tick(&mut unit, &mut routes, &[value], frame);
    }
    assert!((unit.value() - 1.0).abs() < 1e-6);
    assert_eq!(tick(&mut other, &mut routes, &[value], 1000), 0.0);
    let opposite = Modulation {
        amount: -1.0,
        id: b.id,
        ..route
    };
    let mut cancellation = [
        ModulationRoute::new(route, 48_000),
        ModulationRoute::new(opposite, 48_000),
    ];
    unit.reset();
    assert_eq!(tick(&mut unit, &mut cancellation, &[value], 0), 0.0);
    process_modulation(
        &mut unit,
        &mut routes,
        (),
        &[value],
        &[],
        Default::default(),
    );
    assert_eq!(
        unit.value(),
        0.0,
        "Shared inputs must not consume local values"
    );
    for input in [f32::NAN, f32::INFINITY, -1.0] {
        unit.reset();
        for frame in 0..1000 {
            assert!((0.0..=1.0).contains(&tick(
                &mut unit,
                &mut routes,
                &[ModulationValue {
                    value: input,
                    ..value
                }],
                frame
            )));
        }
    }
}
