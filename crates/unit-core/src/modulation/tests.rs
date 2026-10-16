use super::*;
use crate::{InstrumentSettings, ProcessingContext};
fn id(n: u128) -> ModulationSourceId {
    ModulationSourceId {
        layer: Uuid::from_u128(1),
        unit: Uuid::from_u128(n),
    }
}
fn context() -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: 1000,
        max_block_frames: 32,
    }
}
fn route(
    source: ModulationSourceId,
    parameter: ModulationParameter,
    amount: f32,
) -> ModulationRoute {
    let mut settings = Modulation::new(parameter);
    settings.source = Some(source);
    settings.amount = amount;
    ModulationRoute::new(settings, 1000)
}
struct TestSource {
    value: f32,
    ticks: usize,
}
impl ModulationSource for TestSource {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = ModulationOutputDescriptor {
        name: "Test control",
        range: ModulationRange::Bipolar,
        behavior: ModulationBehavior::Contour,
    };
    fn next_modulation(&mut self, _context: ModulationSourceContext) -> f32 {
        self.ticks += 1;
        self.value
    }
}
struct TestDestination;
impl ModulationDestination for TestDestination {
    type Audio = [f32; 2];
    // Deliberately reversed: inputs use parameter identity, never declaration order.
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &[ModulationParameter::Tone, ModulationParameter::Frequency];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        _: Self::Audio,
        inputs: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        [
            inputs.offset(ModulationParameter::Frequency),
            inputs.offset(ModulationParameter::Tone),
        ]
    }
}
#[test]
fn independent_provider_fans_out_to_typed_destinations_without_advancing_twice() {
    let mut source = TestSource {
        value: -0.5,
        ticks: 0,
    };
    let mut output = ModulationOutput::new(id(2), true, context());
    let values = [output.sample(&mut source, Default::default())];
    let mut a = [
        route(id(2), ModulationParameter::Frequency, 2.0),
        route(id(2), ModulationParameter::Tone, -1.0),
    ];
    let mut b = [route(id(2), ModulationParameter::Frequency, 4.0)];
    let c = ModulationContext::default();
    assert_eq!(
        process_modulation(&mut TestDestination, &mut a, [0.0; 2], &[], &values, c),
        [-1.0, 0.5]
    );
    assert_eq!(
        process_modulation(&mut TestDestination, &mut b, [0.0; 2], &[], &values, c),
        [-2.0, 0.0]
    );
    assert_eq!(source.ticks, 1);
    // Bus destinations cannot consume per-note values, even outside the DAW's validator.
    assert_eq!(
        process_modulation(&mut TestDestination, &mut a, [0.0; 2], &values, &[], c),
        [0.0; 2]
    );
}
#[test]
fn common_output_clamps_range_neutralizes_nonfinite_and_keeps_disabled_sources_advancing() {
    let mut source = TestSource {
        value: -2.0,
        ticks: 0,
    };
    let mut output = ModulationOutput::new(id(2), true, context());
    assert_eq!(output.sample(&mut source, Default::default()).value, -1.0);
    source.value = f32::NAN;
    assert_eq!(output.sample(&mut source, Default::default()).value, 0.0);
    source.value = 2.0;
    assert_eq!(output.sample(&mut source, Default::default()).value, 1.0);
    output.set_enabled(false);
    let weights: Vec<_> = (0..5)
        .map(|_| output.sample(&mut source, Default::default()).weight)
        .collect();
    assert!(weights.windows(2).all(|pair| pair[0] > pair[1]));
    assert_eq!(weights[4], 0.0);
    output.sample(&mut source, Default::default());
    assert_eq!(source.ticks, 9);
    assert_eq!(ModulationRange::Unipolar.sanitize(-0.5), 0.0);
    assert_eq!(ModulationRange::Unipolar.sanitize(f32::INFINITY), 0.0);
    struct Weighted(f32);
    impl ModulationSource for Weighted {
        const MODULATION_OUTPUT: ModulationOutputDescriptor = TestSource::MODULATION_OUTPUT;
        fn next_modulation(&mut self, _: ModulationSourceContext) -> f32 {
            1.0
        }
        fn modulation_weight(&self) -> f32 {
            self.0
        }
    }
    let mut weighted = ModulationOutput::new(id(3), true, context());
    for (depth, expected) in [
        (0.0, 0.0),
        (0.5, 0.5),
        (2.0, 1.0),
        (-1.0, 0.0),
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
    ] {
        assert_eq!(
            weighted
                .sample(&mut Weighted(depth), Default::default())
                .weight,
            expected
        );
    }
}
#[test]
fn routes_share_signed_summing_neutral_failures_and_five_millisecond_retargeting() {
    let value = ModulationValue {
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Contour,
        source: id(2),
        value: 0.5,
        weight: 1.0,
        release_age: None,
    };
    let c = ModulationContext::default();
    let mut routes = [
        route(id(2), ModulationParameter::Frequency, 2.0),
        route(id(2), ModulationParameter::Frequency, -2.0),
        route(id(3), ModulationParameter::Frequency, 3.0),
        route(id(2), ModulationParameter::Volume, 12.0),
    ];
    assert_eq!(
        process_modulation(
            &mut TestDestination,
            &mut routes,
            [0.0; 2],
            &[],
            &[value],
            c
        ),
        [0.0; 2]
    );
    routes[1].retarget(&route(id(2), ModulationParameter::Frequency, 0.0));
    let mut previous = 0.0;
    for _ in 0..5 {
        let current = process_modulation(
            &mut TestDestination,
            &mut routes,
            [0.0; 2],
            &[],
            &[value],
            c,
        )[0];
        assert!(current > previous);
        previous = current;
    }
    assert_eq!(previous, 1.0);
    for bad in [
        ModulationValue {
            value: f32::NAN,
            ..value
        },
        ModulationValue {
            weight: f32::INFINITY,
            ..value
        },
        ModulationValue {
            weight: 0.0,
            ..value
        },
    ] {
        assert_eq!(
            process_modulation(&mut TestDestination, &mut routes, [0.0; 2], &[], &[bad], c),
            [0.0; 2]
        );
    }
    let mut invalid = Modulation::new(ModulationParameter::Frequency);
    invalid.source = Some(id(2));
    invalid.amount = f32::NAN;
    assert!(
        ModulationRoute::new(invalid, 1000)
            .sample(&[], &[value])
            .is_none()
    );
}
#[test]
fn discovery_uses_generic_identity_scope_and_retained_presence_with_saved_id_compatibility() {
    let mut instrument: InstrumentSettings = crate::OscillatorSettings::default().into();
    let layer = &mut instrument.layers[0];
    let envelope = &mut layer.envelopes[0];
    envelope.mode = EnvelopeMode::Legato;
    envelope.present = false;
    let source_id = ModulationSourceId {
        layer: layer.id,
        unit: envelope.id,
    };
    let info = instrument.modulation_source(source_id).unwrap();
    assert_eq!(info.scope, ModulationScope::Shared);
    assert!(!info.present);
    assert_eq!(info.output, ENVELOPE_MODULATION_OUTPUT);
    assert!(ModulationScope::PerNote.accepts(info.scope));
    assert!(!ModulationScope::Shared.accepts(ModulationScope::PerNote));
    assert!(instrument.modulation_source(id(999)).is_none());
    let saved = serde_json::json!({"layer": source_id.layer, "envelope": source_id.unit});
    assert_eq!(
        serde_json::from_value::<ModulationSourceId>(saved.clone()).unwrap(),
        source_id
    );
    assert_eq!(serde_json::to_value(source_id).unwrap(), saved);
    let generic = serde_json::json!({"layer": source_id.layer, "unit": source_id.unit});
    assert_eq!(
        serde_json::from_value::<ModulationSourceId>(generic).unwrap(),
        source_id
    );
}

#[test]
fn bipolar_sources_cannot_invert_the_amplitude_gate() {
    let mut routes = [route(id(2), ModulationParameter::Amplitude, 1.0)];
    let value = ModulationValue {
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Contour,
        source: id(2),
        value: -0.5,
        weight: 1.0,
        release_age: None,
    };
    let inputs = ModulationInputs::resolve(
        &mut routes,
        &[ModulationParameter::Amplitude],
        &[value],
        &[],
        ModulationContext::default(),
    );
    assert_eq!(inputs.amplitude(), 0.0);
    let absent = ModulationInputs::resolve(
        &mut routes,
        &[ModulationParameter::Amplitude],
        &[],
        &[],
        ModulationContext {
            note_gate: 0.25,
            ..Default::default()
        },
    );
    assert_eq!(absent.amplitude(), 0.25);
}

#[test]
fn continuous_amplitude_multiplies_the_contour_and_never_prolongs_its_release() {
    let contour = ModulationValue {
        source: id(2),
        value: 0.4,
        weight: 1.0,
        release_age: Some(10),
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Contour,
    };
    let continuous = ModulationValue {
        source: id(3),
        value: 0.25,
        weight: 1.0,
        release_age: None,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    };
    for (value, range) in [
        (0.25, ModulationRange::Unipolar),
        (-0.5, ModulationRange::Bipolar),
    ] {
        let mut routes = [
            route(id(2), ModulationParameter::Amplitude, 1.0),
            route(id(3), ModulationParameter::Amplitude, 0.8),
        ];
        let inputs = ModulationInputs::resolve(
            &mut routes,
            &[ModulationParameter::Amplitude],
            &[],
            &[
                contour,
                ModulationValue {
                    value,
                    range,
                    ..continuous
                },
            ],
            ModulationContext {
                note_gate: 0.0,
                release_age: Some(10),
                ..Default::default()
            },
        );
        assert!((inputs.amplitude() - 0.16).abs() < 1e-6);
        let only_lfo = ModulationInputs::resolve(
            &mut routes,
            &[ModulationParameter::Amplitude],
            &[],
            &[continuous],
            ModulationContext {
                note_gate: 0.0,
                release_age: Some(10),
                ..Default::default()
            },
        );
        assert_eq!(only_lfo.amplitude(), 0.0);
    }
}
#[test]
fn lfo_saved_settings_default_empty_and_validate_identity_and_selected_output_metadata() {
    let mut instrument: InstrumentSettings = crate::OscillatorSettings::default().into();
    let old = serde_json::to_value(&instrument).unwrap();
    let mut legacy = old.clone();
    legacy["layers"][0].as_object_mut().unwrap().remove("lfos");
    assert!(
        serde_json::from_value::<InstrumentSettings>(legacy)
            .unwrap()
            .layers[0]
            .lfos
            .is_empty()
    );
    instrument.layers[0]
        .lfos
        .push(crate::LfoInstance::default());
    let lfo = &mut instrument.layers[0].lfos[0];
    lfo.settings.polarity = crate::LfoPolarity::Bipolar;
    lfo.collapsed = true;
    let source = ModulationSourceId {
        layer: instrument.layers[0].id,
        unit: instrument.layers[0].lfos[0].id,
    };
    assert_eq!(
        instrument.modulation_source(source).unwrap().output.range,
        ModulationRange::Bipolar
    );
    assert_eq!(
        instrument
            .modulation_source(source)
            .unwrap()
            .output
            .behavior,
        ModulationBehavior::Continuous
    );
    assert_eq!(
        serde_json::from_value::<InstrumentSettings>(serde_json::to_value(&instrument).unwrap())
            .unwrap(),
        instrument
    );
    assert!(instrument.valid());
    for invalid in [f32::NAN, -1.0, 101.0] {
        let mut bad = instrument.clone();
        bad.layers[0].lfos[0].settings.smooth_ms = invalid;
        assert!(!bad.valid());
    }
    let duplicate = instrument.layers[0].envelopes[0].id;
    instrument.layers[0].lfos[0].id = duplicate;
    assert!(!instrument.valid());
    let mut malformed = old;
    malformed["layers"][0]["lfos"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<InstrumentSettings>(malformed).is_err());
}

#[test]
fn lfo_destinations_validate_shared_references_cycles_and_legacy_defaults() {
    let mut i = InstrumentSettings::default();
    let owner = i.layers[0].id;
    let mut a = crate::LfoInstance::default();
    let a_id = ModulationSourceId {
        layer: owner,
        unit: a.id,
    };
    let mut b = crate::LfoInstance::default();
    let b_id = ModulationSourceId {
        layer: i.main.id,
        unit: b.id,
    };
    a.modulations.push(Modulation {
        source: Some(b_id),
        amount: 1.0,
        ..Modulation::new(ModulationParameter::LfoDepth)
    });
    b.modulation_open = true;
    i.layers[0].lfos.push(a);
    i.main.lfos.push(b);
    assert!(i.valid());
    let nodes: Vec<_> = i
        .lfo_destinations()
        .map(|(id, l)| (id, l.modulations.as_slice()))
        .collect();
    assert_eq!(modulation_dependency_order(&nodes), Some(vec![1, 0]));
    assert!(modulation_would_cycle(&nodes, b_id, a_id));
    assert!(modulation_would_cycle(&nodes, a_id, a_id));
    assert!(!modulation_would_cycle(&nodes, a_id, b_id));
    let mut legacy = serde_json::to_value(&i).unwrap();
    let a = &mut legacy["layers"][0]["lfos"][0];
    a["settings"].as_object_mut().unwrap().remove("depth");
    a.as_object_mut().unwrap().remove("modulations");
    a.as_object_mut().unwrap().remove("modulation_open");
    let loaded: InstrumentSettings = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(loaded.layers[0].lfos[0].settings.depth, 1.0);
    assert!(
        loaded.layers[0].lfos[0].modulations.is_empty()
            && !loaded.layers[0].lfos[0].modulation_open
    );
    assert!(loaded.valid());
    legacy["layers"][0]["lfos"][0]["modulations"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<InstrumentSettings>(legacy).is_err());
    let original = i.clone();
    for source in [
        a_id,
        b_id,
        ModulationSourceId {
            layer: owner,
            unit: i.layers[0].envelopes[0].id,
        },
        ModulationSourceId {
            layer: owner,
            unit: id(999).unit,
        },
    ] {
        let mut invalid = original.clone();
        invalid.main.lfos[0].modulations.push(Modulation {
            source: Some(source),
            enabled: false,
            ..Modulation::new(ModulationParameter::LfoRate)
        });
        assert!(!invalid.valid(), "{source:?}");
    }
    for depth in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
        let mut invalid = original.clone();
        invalid.main.lfos[0].settings.depth = depth;
        assert!(!invalid.valid());
    }
    let mut wrong_target = original;
    wrong_target.main.lfos[0]
        .modulations
        .push(Modulation::new(ModulationParameter::Pitch));
    assert!(!wrong_target.valid());
}
