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
    };
    fn next_modulation(&mut self) -> f32 {
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
    let values = [output.sample(&mut source)];
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
    assert_eq!(output.sample(&mut source).value, -1.0);
    source.value = f32::NAN;
    assert_eq!(output.sample(&mut source).value, 0.0);
    source.value = 2.0;
    assert_eq!(output.sample(&mut source).value, 1.0);
    output.set_enabled(false);
    let weights: Vec<_> = (0..5).map(|_| output.sample(&mut source).weight).collect();
    assert!(weights.windows(2).all(|pair| pair[0] > pair[1]));
    assert_eq!(weights[4], 0.0);
    output.sample(&mut source);
    assert_eq!(source.ticks, 9);
    assert_eq!(ModulationRange::Unipolar.sanitize(-0.5), 0.0);
    assert_eq!(ModulationRange::Unipolar.sanitize(f32::INFINITY), 0.0);
}
#[test]
fn routes_share_signed_summing_neutral_failures_and_five_millisecond_retargeting() {
    let value = ModulationValue {
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
