use super::tests::{pointer, spec_frame};
use super::*;
use crate::parameter_knobs;
use daw_core::ModulationParameter as P;

const PARAMETERS: [P; 57] = [
    P::OttLowFrequency,
    P::OttHighFrequency,
    P::OttLowDepth,
    P::OttLowInput,
    P::OttLowOutput,
    P::OttLowAttack,
    P::OttLowRelease,
    P::OttMidDepth,
    P::OttMidInput,
    P::OttMidOutput,
    P::OttMidAttack,
    P::OttMidRelease,
    P::OttHighDepth,
    P::OttHighInput,
    P::OttHighOutput,
    P::OttHighAttack,
    P::OttHighRelease,
    P::DirectionMixerDirection,
    P::DirectionMixerSpread,
    P::BitcrusherDrive,
    P::BitcrusherResolution,
    P::BitcrusherDownsampling,
    P::StereoSpreadLowerIntensity,
    P::StereoSpreadUpperIntensity,
    P::StereoSpreadLowerFrequency,
    P::StereoSpreadUpperFrequency,
    P::LimiterInput,
    P::LimiterRelease,
    P::LimiterLookahead,
    P::LimiterCeiling,
    P::SamplerGain,
    P::Pitch,
    P::Amplitude,
    P::Frequency,
    P::Resonance,
    P::Level,
    P::DryWet,
    P::Volume,
    P::DriveGain,
    P::Tone,
    P::ColorStrength,
    P::ColorFrequency,
    P::LfoDepth,
    P::LfoRate,
    P::MacroValue,
    P::EqLowGain,
    P::EqLowFrequency,
    P::EqLowQ,
    P::EqMidGain,
    P::EqHighGain,
    P::EqHighFrequency,
    P::EqHighQ,
    P::CompressorThreshold,
    P::CompressorRatio,
    P::CompressorAttack,
    P::CompressorRelease,
    P::CompressorKnee,
];

fn native_value(parameter: P, amount: f32) -> f32 {
    match parameter {
        P::DirectionMixerDirection
        | P::OttLowInput
        | P::OttLowOutput
        | P::OttMidInput
        | P::OttMidOutput
        | P::OttHighInput
        | P::OttHighOutput
        | P::LimiterInput
        | P::Pitch
        | P::EqLowGain
        | P::EqMidGain
        | P::EqHighGain => amount,
        P::Amplitude => 20.0 * (1.0 - amount.abs()).log10(),
        P::LimiterCeiling | P::SamplerGain | P::Level | P::Volume | P::CompressorThreshold => {
            -amount.abs()
        }
        P::StereoSpreadLowerFrequency
        | P::StereoSpreadUpperFrequency
        | P::OttLowFrequency
        | P::OttHighFrequency
        | P::OttLowAttack
        | P::OttLowRelease
        | P::OttMidAttack
        | P::OttMidRelease
        | P::OttHighAttack
        | P::OttHighRelease
        | P::LimiterRelease
        | P::Frequency
        | P::ColorFrequency
        | P::LfoRate
        | P::EqLowFrequency
        | P::EqHighFrequency
        | P::CompressorAttack
        | P::CompressorRelease => {
            parameter_knobs::destination(parameter).reset * 2.0_f32.powf(amount)
        }
        P::BitcrusherResolution
        | P::BitcrusherDownsampling
        | P::CompressorRatio
        | P::DirectionMixerSpread => 1.0 + amount.abs(),
        P::Resonance | P::EqLowQ | P::EqHighQ => 0.1 + amount.abs(),
        _ => amount.abs(),
    }
}
fn direction(parameter: P, amount: f32) -> f32 {
    match parameter {
        P::DirectionMixerDirection
        | P::OttLowInput
        | P::OttLowOutput
        | P::OttMidInput
        | P::OttMidOutput
        | P::OttHighInput
        | P::OttHighOutput
        | P::OttLowFrequency
        | P::OttHighFrequency
        | P::OttLowAttack
        | P::OttLowRelease
        | P::OttMidAttack
        | P::OttMidRelease
        | P::OttHighAttack
        | P::OttHighRelease => 1.0,
        P::StereoSpreadLowerFrequency
        | P::StereoSpreadUpperFrequency
        | P::LimiterInput
        | P::LimiterRelease
        | P::Pitch
        | P::Frequency
        | P::ColorFrequency
        | P::LfoRate
        | P::EqLowGain
        | P::EqMidGain
        | P::EqHighGain
        | P::EqLowFrequency
        | P::EqHighFrequency
        | P::CompressorAttack
        | P::CompressorRelease => 1.0,
        P::Amplitude
        | P::LimiterCeiling
        | P::SamplerGain
        | P::Level
        | P::Volume
        | P::CompressorThreshold => -amount.signum(),
        _ => amount.signum(),
    }
}
#[test]
fn signed_amount_scales_cover_both_bounds_and_roundtrip_without_changing_saved_units() {
    for parameter in PARAMETERS {
        let spec = parameter_knobs::amount(parameter);
        let limit = parameter.limit();
        assert_eq!((spec.min, spec.max, spec.reset), (-limit, limit, 0.0));
        assert_eq!(spec.normalized(0.0), 0.5);
        assert_eq!(spec.value(0.5), 0.0);
        assert_eq!(spec.value(-1.0), -limit);
        assert_eq!(spec.value(2.0), limit);
        let mut previous = -f32::INFINITY;
        for i in 0..=1000 {
            let value = spec.value(i as f32 / 1000.0);
            assert!(
                value.is_finite() && value >= previous,
                "{parameter:?}: {i} {value} {previous}"
            );
            assert!((-limit..=limit).contains(&value));
            previous = value;
            let roundtrip = spec.value(spec.normalized(value));
            assert!(
                (roundtrip - value).abs() < 1e-4 * limit,
                "{parameter:?}: {value} {roundtrip}"
            );
        }
        assert!(spec.bipolar);
        assert!(spec.value_label.is_some() && spec.endpoint_labels.is_some());
        // Legacy fractional amounts remain valid even for a now-stepped pitch control.
        assert!(
            daw_core::Modulation {
                amount: 0.25,
                ..daw_core::Modulation::new(parameter)
            }
            .valid()
        );
    }
}
#[test]
fn modulation_keyboard_steps_and_mouse_sensitivity_match_the_native_destination_domain() {
    for parameter in PARAMETERS {
        let native = parameter_knobs::destination(parameter);
        let amount = parameter_knobs::amount(parameter);
        assert_eq!(amount.keyboard_step, native.keyboard_step);
        assert_eq!(amount.discrete_step, native.discrete_step);
        assert_eq!(amount.reference_step, native.reference_step);
        assert_eq!(amount.reference_values, native.reference_values);
        // Upper Frequency resets at its upper bound. Keep native comparison values
        // inside the base knob range; signed amount bounds are tested separately.
        let values = if parameter == P::StereoSpreadLowerFrequency {
            [0.5, 1.5]
        } else if parameter == P::StereoSpreadUpperFrequency {
            [-1.5, -0.5]
        } else {
            [-0.25 * parameter.limit(), 0.25 * parameter.limit()]
        };
        for value in values {
            let native_start = native_value(parameter, value);
            let signed_direction = direction(parameter, value);
            let expected = native
                .value(native.normalized(native.keyboard_value(native_start, signed_direction)));
            let stepped = amount.value(amount.normalized(amount.keyboard_value(value, 1.0)));
            let actual = native_value(parameter, stepped);
            assert!(
                (actual - expected).abs() < expected.abs().max(1.0) * 2e-5,
                "key {parameter:?}: {value} actual={actual} expected={expected}"
            );
            // Equal pointer travel must cover equal native-scale travel, including Shift's factor.
            for delta in [0.01, 0.001] {
                let expected =
                    native.value(native.normalized(native_start) + signed_direction * delta);
                let dragged = amount.value(amount.normalized(value) + delta * amount.drag_factor());
                let actual = native_value(parameter, dragged);
                assert!(
                    (actual - expected).abs() < expected.abs().max(1.0) * 2e-5,
                    "drag {parameter:?}: {value} actual={actual} expected={expected}"
                );
            }
        }
        if let Some(scale) = amount.amount_scale {
            assert!(scale.drag_factor().is_finite() && scale.drag_factor() > 0.0);
            if let Some(marks) = amount.reference_values {
                for mark in marks.iter().filter_map(|v| scale.reference_amount(*v)) {
                    assert!(mark.is_finite() && (0.0..=parameter.limit()).contains(&mark));
                }
            }
        }
    }
    assert!(
        (parameter_knobs::amount(P::Frequency).keyboard_value(0.0, 1.0) - 1.0 / 24.0).abs() < 1e-7
    );
}
#[test]
fn all_amount_controls_retain_idle_values_and_share_keyboard_disabled_reset_and_fine_drag_behavior()
{
    for parameter in PARAMETERS {
        let spec = parameter_knobs::amount(parameter);
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let mut value = 0.25;
        let rect = spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true);
        assert_eq!(value, 0.25, "idle {parameter:?}");
        let point = rect.center();
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
            Default::default(),
            spec,
            true,
        );
        spec_frame(
            &ctx,
            &mut value,
            vec![pointer(point, false)],
            Default::default(),
            spec,
            true,
        );
        let key = |pressed| egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        spec_frame(
            &ctx,
            &mut value,
            vec![key(true)],
            Default::default(),
            spec,
            true,
        );
        assert!(value > 0.25, "key {parameter:?}: {value}");
        spec_frame(
            &ctx,
            &mut value,
            vec![key(false)],
            Default::default(),
            spec,
            true,
        );
        let held = value;
        spec_frame(
            &ctx,
            &mut value,
            vec![key(true)],
            Default::default(),
            spec,
            false,
        );
        assert_eq!(value, held, "disabled {parameter:?}");
        spec_frame(
            &ctx,
            &mut value,
            vec![key(false)],
            Default::default(),
            spec,
            true,
        );
        for _ in 0..2 {
            spec_frame(
                &ctx,
                &mut value,
                vec![pointer(point, true)],
                Default::default(),
                spec,
                true,
            );
            spec_frame(
                &ctx,
                &mut value,
                vec![pointer(point, false)],
                Default::default(),
                spec,
                true,
            );
        }
        assert_eq!(value, 0.0, "reset {parameter:?}");
        spec_frame(
            &ctx,
            &mut value,
            vec![pointer(point, true)],
            Default::default(),
            spec,
            true,
        );
        let end = point - Vec2::new(0.0, 12.0);
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(end)],
            Default::default(),
            spec,
            true,
        );
        let coarse = value;
        spec_frame(&ctx, &mut value, vec![], egui::Modifiers::SHIFT, spec, true);
        assert_eq!(value, coarse, "Shift alone {parameter:?}");
        for distance in 1..=8 {
            let next = end - Vec2::new(0.0, distance as f32 * 3.0);
            let previous = value;
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(next)],
                egui::Modifiers::SHIFT,
                spec,
                true,
            );
            assert!(
                value >= previous && value <= parameter.limit(),
                "drag {parameter:?}"
            );
            if parameter == P::Pitch {
                assert_eq!(value.fract(), 0.0);
            }
        }
    }
}

#[test]
fn envelope_time_multiplier_display_preserves_saved_amounts_and_existing_interaction() {
    for parameter in [P::EnvelopeAttack, P::EnvelopeDecay, P::EnvelopeRelease] {
        let spec = parameter_knobs::amount(parameter);
        let previous = parameter_knobs::destination(parameter)
            .modulation_amount(parameter.limit(), AmountMapping::Octaves);
        assert_eq!((spec.min, spec.max, spec.reset), (-6.0, 6.0, 0.0));
        for (saved, display) in [
            (-6.0, "×0.015625"),
            (-1.0, "×0.5"),
            (0.0, "×1"),
            (2.0, "×4"),
            (6.0, "×64"),
        ] {
            assert_eq!((spec.format)(saved), display);
            assert_eq!(spec.value_label.unwrap()(saved), display);
            assert_eq!(parameter_knobs::format_amount(parameter, saved), display);
            assert_eq!(spec.normalized(saved), previous.normalized(saved));
            assert_eq!(
                spec.keyboard_value(saved, 1.0).to_bits(),
                previous.keyboard_value(saved, 1.0).to_bits()
            );
            assert_eq!(
                spec.keyboard_value(saved, -1.0).to_bits(),
                previous.keyboard_value(saved, -1.0).to_bits()
            );
        }
        assert_eq!(spec.endpoint_labels, Some(["×1/64", "×64"]));
        assert_eq!(spec.drag_factor(), previous.drag_factor());
        for step in 0..=1000 {
            let position = step as f32 / 1000.0;
            assert_eq!(spec.value(position), previous.value(position));
        }
    }
}
