use super::*;
use daw_unit_core::*;
fn context() -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: 48000,
        max_block_frames: 512,
    }
}
fn source() -> ModulationValue {
    ModulationValue {
        source: ModulationSourceId {
            layer: EnvelopeInstance::default().id,
            unit: EnvelopeInstance::default().id,
        },
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
        release_age: None,
    }
}
fn routes(
    source: ModulationValue,
    parameters: &[ModulationParameter],
    amount: f32,
) -> Vec<ModulationRoute> {
    parameters
        .iter()
        .map(|&p| {
            let mut r = Modulation::new(p);
            r.source = Some(source.source);
            r.amount = amount;
            ModulationRoute::new(r, 48000)
        })
        .collect()
}
fn tick(e: &mut Envelope, routes: &mut [ModulationRoute], values: &[ModulationValue]) -> f32 {
    process_modulation(e, routes, (), &[], values, Default::default());
    e.next_sample()
}
#[test]
fn neutral_inputs_are_exact_and_time_modulation_preserves_base_and_stage_progress() {
    let settings = EnvelopeSettings::default();
    let mut original = Envelope::new(settings, context());
    let mut neutral = Envelope::new(settings, context());
    original.note_on();
    neutral.note_on();
    for frame in 0..20000 {
        if frame == 9000 {
            original.note_off();
            neutral.note_off();
        }
        assert_eq!(original.next_sample(), tick(&mut neutral, &mut [], &[]));
    }
    let settings = EnvelopeSettings {
        attack: EnvelopeStageSettings {
            time_ms: 100.0,
            ..settings.attack
        },
        ..settings
    };
    let mut e = Envelope::new(settings, context());
    e.note_on_modulated(None);
    let value = source();
    let mut connections = routes(value, &[ModulationParameter::EnvelopeAttack], 1.0);
    let mut previous = 0.0;
    for _ in 0..4800 {
        let current = tick(&mut e, &mut connections, &[value]);
        assert!(current >= previous && current - previous < 0.01);
        previous = current;
    }
    assert_eq!(e.stage(), Stage::Attack);
    assert!((0.49..0.53).contains(&e.level()));
    assert_eq!(e.settings(), settings);
    assert!((e.modulation_values()[0] - 200.0).abs() < 0.01);
    e.note_off();
    for _ in 0..10000 {
        tick(&mut e, &mut connections, &[value]);
    }
    assert!(e.is_idle());
}
#[test]
fn all_typed_targets_apply_bounds_and_note_percent_can_start_from_zero() {
    let value = source();
    let settings = EnvelopeSettings {
        timing: EnvelopeTiming::Note,
        attack_note: 0.0,
        decay_note: 0.0,
        ..Default::default()
    };
    let mut e = Envelope::new(settings, context());
    let mut connections = routes(value, &ModulationParameter::ENVELOPE, 1.0);
    e.note_on_modulated(Some(48000));
    tick(&mut e, &mut connections, &[value]);
    assert_eq!(e.stage(), Stage::Attack);
    for _ in 0..1000 {
        tick(&mut e, &mut connections, &[value]);
    }
    let v = e.modulation_values();
    assert!((v[0] - settings.attack.time_ms * 2.0).abs() < 0.01);
    assert!((v[1] - settings.decay.time_ms * 2.0).abs() < 0.01);
    assert!((v[3] - settings.release.time_ms * 2.0).abs() < 0.01);
    assert!(v[2] <= 1.0 && v[2] > 0.99);
    for &value in &v[4..] {
        assert!((0.99..=1.0).contains(&value));
    }
    assert_eq!(e.settings(), settings);
    let mut cancelling = routes(value, &ModulationParameter::ENVELOPE, -1.0);
    connections.append(&mut cancelling);
    for _ in 0..2000 {
        tick(&mut e, &mut connections, &[value]);
    }
    for (effective, base) in e
        .modulation_values()
        .into_iter()
        .zip(settings.parameter_values())
    {
        assert!((effective - base).abs() < 0.001);
    }
}

#[test]
fn changing_durations_keep_progress_and_release_bounded_at_every_processing_rate() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let settings = EnvelopeSettings {
            attack: EnvelopeStageSettings {
                time_ms: 100.0,
                shape: CurveShape::Linear,
                amount: 0.5,
            },
            ..Default::default()
        };
        let mut e = Envelope::new(
            settings,
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            },
        );
        let mut value = source();
        let mut route = Modulation::new(ModulationParameter::EnvelopeAttack);
        route.source = Some(value.source);
        route.amount = 2.0;
        let mut routes = [ModulationRoute::new(route, rate)];
        e.note_on_modulated(None);
        let mut previous = 0.0;
        for frame in 0..rate / 10 {
            value.value =
                0.5 + 0.5 * (frame as f32 / rate as f32 * std::f32::consts::TAU * 10.0).sin();
            let level = tick(&mut e, &mut routes, &[value]);
            assert!(level >= previous - 1e-6 && level - previous < 0.01);
            previous = level;
        }
        assert_eq!(e.stage(), Stage::Attack);
        e.note_off();
        for _ in 0..rate {
            tick(&mut e, &mut routes, &[value]);
        }
        assert!(e.is_idle());
    }
}

#[test]
fn curve_strength_inputs_change_the_actual_attack_decay_and_release_contours() {
    for parameter in [
        ModulationParameter::EnvelopeAttackCurve,
        ModulationParameter::EnvelopeDecayCurve,
        ModulationParameter::EnvelopeReleaseCurve,
    ] {
        let stage = EnvelopeStageSettings {
            time_ms: 100.0,
            shape: CurveShape::Exponential,
            amount: 0.5,
        };
        let settings = EnvelopeSettings {
            attack: stage,
            decay: stage,
            release: stage,
            sustain: 0.5,
            ..Default::default()
        };
        let value = source();
        let mut connections = routes(value, &[parameter], -0.5);
        let mut processed = Envelope::new(settings, context());
        let mut reference = Envelope::new(settings, context());
        processed.note_on_modulated(None);
        reference.note_on();
        let prefix = if parameter == ModulationParameter::EnvelopeDecayCurve {
            4800
        } else if parameter == ModulationParameter::EnvelopeReleaseCurve {
            9600
        } else {
            0
        };
        for _ in 0..prefix {
            tick(&mut processed, &mut connections, &[value]);
            reference.next_sample();
        }
        if parameter == ModulationParameter::EnvelopeReleaseCurve {
            processed.note_off();
            reference.note_off();
        }
        for _ in 0..1200 {
            tick(&mut processed, &mut connections, &[value]);
            reference.next_sample();
        }
        assert!(
            (processed.level() - reference.level()).abs() > 0.01,
            "{parameter:?}"
        );
    }
}

#[test]
fn shared_envelopes_reject_local_inputs_through_the_common_port_entry_point() {
    let value = source();
    let mut connections = routes(value, &[ModulationParameter::EnvelopeSustain], 1.0);
    let mut e = Envelope::new(EnvelopeSettings::default(), context())
        .with_modulation_scope(ModulationScope::Shared);
    assert_eq!(e.modulation_input_scope(), ModulationScope::Shared);
    for _ in 0..1000 {
        process_modulation(
            &mut e,
            &mut connections,
            (),
            &[value],
            &[],
            Default::default(),
        );
    }
    assert_eq!(
        e.modulation_values()[2],
        EnvelopeSettings::default().sustain
    );
    for _ in 0..1000 {
        process_modulation(
            &mut e,
            &mut connections,
            (),
            &[],
            &[value],
            Default::default(),
        );
    }
    assert!(e.modulation_values()[2] > 0.99);
}
