use super::*;
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
    ModulationValue, process_modulation,
};
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn close(actual: [f32; 2], expected: [f32; 2]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() < 0.00001, "{actual:?} != {expected:?}");
    }
}
#[test]
fn neutral_decode_width_and_cardinal_directions_match_the_design_at_every_rate() {
    for rate in RATES {
        let mut settings = DirectionMixerSettings::default();
        let mut unit = DirectionMixer::new(context(rate), &settings);
        for input in [
            [0.0; 2],
            [0.25, -0.5],
            [f32::MAX, -f32::MAX],
            [f32::from_bits(1), 0.0],
        ] {
            assert_eq!(unit.process_sample(input), input);
        }
        for (direction, spread, expected) in [
            (0.0, 0.0, [0.375; 2]),
            (0.0, 2.0, [0.2375, -0.0125]),
            (180.0, 1.0, [0.25, 0.5]),
            (-180.0, 1.0, [0.25, 0.5]),
            (90.0, 1.0, [0.0, 0.375 * std::f32::consts::SQRT_2]),
            (-90.0, 1.0, [0.375 * std::f32::consts::SQRT_2, 0.0]),
        ] {
            settings.direction_degrees = direction;
            settings.spread = spread;
            unit.retarget(&settings);
            unit.reset();
            close(unit.process_sample([0.5, 0.25]), expected);
        }
        settings.input = DirectionMixerInput::MidSide;
        settings.direction_degrees = 0.0;
        settings.spread = 1.0;
        unit.retarget(&settings);
        unit.reset();
        close(unit.process_sample([0.375, 0.125]), [0.5, 0.25]);
        settings.enabled = false;
        unit.retarget(&settings);
        for _ in 0..rate / 200 {
            unit.process_sample([0.375, 0.125]);
        }
        assert_eq!(unit.process_sample([0.375, 0.125]), [0.375, 0.125]);
    }
}
#[test]
fn typed_routes_enforce_scope_sum_offsets_clamp_and_preserve_neutral_audio() {
    let source = ModulationValue {
        source: ModulationSourceId {
            layer: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            unit: "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        },
        value: 1.0,
        weight: 1.0,
        release_age: None,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    };
    for rate in RATES {
        let mut unit = DirectionMixer::new(context(rate), &DirectionMixerSettings::default());
        let mut routes: Vec<_> = ModulationParameter::DIRECTION_MIXER
            .into_iter()
            .map(|parameter| {
                ModulationRoute::new(
                    Modulation {
                        source: Some(source.source),
                        amount: parameter.limit(),
                        ..Modulation::new(parameter)
                    },
                    rate,
                )
            })
            .collect();
        for _ in 0..rate / 50 {
            assert_eq!(
                process_modulation(
                    &mut unit,
                    &mut routes,
                    [0.5, 0.25],
                    &[source],
                    &[],
                    ModulationContext::default()
                ),
                [0.5, 0.25]
            );
        }
        for _ in 0..rate / 50 {
            process_modulation(
                &mut unit,
                &mut routes,
                [0.5, 0.25],
                &[],
                &[source],
                ModulationContext::default(),
            );
        }
        let effective = unit.effective_parameters();
        assert!((effective[0] - 180.0).abs() < 0.001);
        assert_eq!(effective[1], 2.0);
        unit.reset();
        assert_eq!(unit.effective_parameters(), [0.0, 1.0]);
        for offsets in [[f32::MAX; 2], [-f32::MAX; 2], [f32::NAN, f32::INFINITY]] {
            for _ in 0..1000 {
                assert!(
                    unit.process_modulated([f32::MAX, -f32::MAX], offsets)
                        .iter()
                        .all(|v| v.is_finite())
                );
            }
            let [d, s] = unit.effective_parameters();
            assert!((-180.0..=180.0).contains(&d));
            assert!((0.0..=2.0).contains(&s));
        }
    }
}
#[test]
fn compatible_updates_preserve_unrelated_ramps_and_reset_is_silent() {
    let mut settings = DirectionMixerSettings {
        direction_degrees: 0.0,
        ..Default::default()
    };
    let mut unit = DirectionMixer::new(context(48000), &settings);
    settings.direction_degrees = 90.0;
    unit.retarget(&settings);
    for _ in 0..120 {
        unit.process_sample([0.0; 2]);
    }
    let remaining = unit.parameters[0].remaining;
    settings.collapsed = true;
    settings.modulation_open = true;
    settings.spread = 2.0;
    unit.retarget(&settings);
    assert_eq!(unit.parameters[0].remaining, remaining);
    for _ in 0..120 {
        assert_eq!(unit.process_sample([0.0; 2]), [0.0; 2]);
    }
    assert_eq!(unit.effective_parameters()[0], 90.0);
    unit.reset();
    assert_eq!(unit.process_sample([f32::NAN, f32::INFINITY]), [0.0; 2]);
}

#[test]
fn direction_and_modulation_follow_left_and_right_knob_positions() {
    for rate in RATES {
        for input_mode in [DirectionMixerInput::LeftRight, DirectionMixerInput::MidSide] {
            let input = if input_mode == DirectionMixerInput::LeftRight {
                [0.5; 2]
            } else {
                [0.5, 0.0]
            };
            for degrees in [-98.0, -90.0, -45.0, 45.0, 90.0, 98.0] {
                for modulated in [false, true] {
                    let settings = DirectionMixerSettings {
                        input: input_mode,
                        direction_degrees: if modulated { 0.0 } else { degrees },
                        ..Default::default()
                    };
                    let mut unit = DirectionMixer::new(context(rate), &settings);
                    let mut output = [0.0; 2];
                    for _ in 0..rate / 50 {
                        output = unit
                            .process_modulated(input, [if modulated { degrees } else { 0.0 }, 0.0]);
                    }
                    assert_eq!(
                        output[0] > output[1],
                        degrees < 0.0,
                        "rate={rate} input={input_mode:?} degrees={degrees} modulation={modulated}: {output:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn lr_spread_matches_measured_reference_gains_for_base_and_modulation() {
    // User-supplied L/R bounces establish these gain pairs. Half-step checks
    // exercise interpolation. No private audio is a fixture.
    for rate in RATES {
        for (spread, mid_gain, side_gain) in [
            (0.0, 1.0, 0.0),
            (0.5, 1.0, 0.5),
            (1.0, 1.0, 1.0),
            (1.125, 0.909, 1.0),
            (1.25, 0.818, 1.0),
            (1.5, 0.65, 1.0),
            (1.75, 0.482, 1.0),
            (1.875, 0.391, 1.0),
            (2.0, 0.3, 1.0),
        ] {
            for (input, mid, side) in [
                ([0.5, 0.25], 0.375, 0.125),
                ([0.5; 2], 0.5, 0.0),
                ([0.25, -0.25], 0.0, 0.25),
                ([0.501, 0.499], 0.5, 0.001),
            ] {
                for modulated in [false, true] {
                    let mut unit = DirectionMixer::new(
                        context(rate),
                        &DirectionMixerSettings {
                            spread: if modulated { 1.0 } else { spread },
                            ..Default::default()
                        },
                    );
                    let mut output = [0.0; 2];
                    for _ in 0..rate / 50 {
                        output = unit.process_modulated(
                            input,
                            [0.0, if modulated { spread - 1.0 } else { 0.0 }],
                        );
                    }
                    close(
                        output,
                        [
                            mid * mid_gain + side * side_gain,
                            mid * mid_gain - side * side_gain,
                        ],
                    );
                    assert!((unit.effective_parameters()[1] - spread).abs() < 0.0001);
                }
            }
        }
        // Encoded M/S keeps its direct Side-level behavior, independent of L/R widening.
        for (spread, expected) in [
            (0.0, [0.375; 2]),
            (1.5, [0.5625, 0.1875]),
            (2.0, [0.625, 0.125]),
        ] {
            let mut unit = DirectionMixer::new(
                context(rate),
                &DirectionMixerSettings {
                    input: DirectionMixerInput::MidSide,
                    spread,
                    ..Default::default()
                },
            );
            close(unit.process_sample([0.375, 0.125]), expected);
        }
    }
}

#[test]
fn lr_spread_retargets_smoothly_across_unity_and_input_mode() {
    for rate in RATES {
        let mut settings = DirectionMixerSettings {
            spread: 0.0,
            ..Default::default()
        };
        let mut unit = DirectionMixer::new(context(rate), &settings);
        let mut previous = unit.process_sample([0.5, 0.25]);
        for (spread, mode, expected) in [
            (2.0, DirectionMixerInput::LeftRight, [0.2375, -0.0125]),
            (2.0, DirectionMixerInput::MidSide, [1.0, 0.0]),
            (0.0, DirectionMixerInput::LeftRight, [0.375; 2]),
        ] {
            settings.spread = spread;
            settings.input = mode;
            unit.retarget(&settings);
            for _ in 0..rate / 200 {
                let current = unit.process_sample([0.5, 0.25]);
                for ch in 0..2 {
                    assert!((current[ch] - previous[ch]).abs() < 0.02);
                }
                previous = current;
            }
            close(previous, expected);
        }
    }
}
