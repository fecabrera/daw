use super::*;
use daw_unit_core::{
    DriveV2ColorShape, Modulation, ModulationBehavior, ModulationRange, ModulationRoute,
    ModulationSourceId, ModulationValue, process_modulation,
};
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
#[test]
fn soft_matches_original_drive_with_color_tone_gain_and_output_level_at_every_rate() {
    for rate in RATES {
        for enabled in [false, true] {
            for shape in DriveV2ColorShape::ALL {
                for level in [SILENT_GAIN_DB, -12.0, 0.0, 12.0] {
                    let s = DriveV2Settings {
                        color_enabled: enabled,
                        color_shape: shape,
                        gain: 0.7,
                        tone: 0.8,
                        level_db: level,
                        ..Default::default()
                    };
                    let mut old_settings = kernel(&s);
                    old_settings.volume_db = level;
                    let mut old = Drive::new(context(rate), &old_settings);
                    let mut new = DriveV2::new(context(rate), &s);
                    for frame in 0..4096 {
                        let x = (frame as f32 * 0.17).sin() * 0.7;
                        let input = [x, -x * 0.3];
                        assert_eq!(old.process_sample(input), new.process_sample(input));
                    }
                }
            }
        }
    }
}

#[test]
fn dry_retains_input_gain_output_level_follows_mix_and_bypass_is_original_audio() {
    for rate in RATES {
        for clip in [DriveV2Clip::Soft, DriveV2Clip::Hard] {
            let s = DriveV2Settings {
                input_gain_db: 6.0,
                level_db: -3.0,
                dry_wet: 0.0,
                gain: 1.0,
                color_enabled: true,
                clip,
                ..Default::default()
            };
            let mut new = DriveV2::new(context(rate), &s);
            for _ in 0..512 {
                let input = [0.25, -0.12];
                assert_eq!(
                    new.process_sample(input),
                    input.map(|v| v * amplitude(6.0) * amplitude(-3.0))
                );
            }
            let off = DriveV2Settings {
                enabled: false,
                ..s.clone()
            };
            new.retarget(&off);
            for _ in 0..rate / 200 + 1 {
                new.process_sample([0.25, -0.12]);
            }
            assert_eq!(new.process_sample([0.25, -0.12]), [0.25, -0.12]);
            let silent = DriveV2Settings {
                input_gain_db: SILENT_GAIN_DB,
                ..s
            };
            let mut new = DriveV2::new(context(rate), &silent);
            assert_eq!(new.process_sample([1.0, -1.0]), [0.0; 2]);
        }
    }
}
#[test]
fn hard_clip_differs_from_soft_and_mode_power_retargets_and_reset_remain_repeatable() {
    for rate in RATES {
        let soft = DriveV2Settings {
            gain: 0.5,
            ..Default::default()
        };
        let hard = DriveV2Settings {
            clip: DriveV2Clip::Hard,
            ..soft.clone()
        };
        let mut a = DriveV2::new(context(rate), &soft);
        let mut b = DriveV2::new(context(rate), &hard);
        assert!((a.process_sample([0.1; 2])[0] - b.process_sample([0.1; 2])[0]).abs() > 0.01);
        b.retarget(&soft);
        for _ in 0..rate / 200 + 1 {
            b.process_sample([0.2; 2]);
        }
        b.reset();
        a.reset();
        for _ in 0..1024 {
            assert_eq!(a.process_sample([0.2; 2]), b.process_sample([0.2; 2]));
        }
        let clean = DriveV2Settings {
            gain: 0.0,
            clip: DriveV2Clip::Hard,
            ..Default::default()
        };
        let mut c = DriveV2::new(context(rate), &clean);
        assert_eq!(c.process_sample([0.2, -0.4]), [0.2, -0.4]);
    }
}
#[test]
fn all_destinations_use_shared_scope_smoothing_bounds_and_exact_base_silence() {
    for rate in RATES {
        let s = DriveV2Settings::default();
        let id = ModulationSourceId {
            layer: daw_unit_core::ModulationTargetId::new_v4(),
            unit: daw_unit_core::ModulationTargetId::new_v4(),
        };
        let value = ModulationValue {
            source: id,
            value: 1.0,
            weight: 1.0,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
            release_age: None,
        };
        let settings: Vec<_> = ModulationParameter::DRIVE_V2
            .iter()
            .map(|&p| {
                let mut r = Modulation::new(p);
                r.source = Some(id);
                r.amount = p.limit();
                r
            })
            .collect();
        let routes = || {
            settings
                .iter()
                .map(|r| ModulationRoute::new(*r, rate))
                .collect::<Vec<_>>()
        };
        let mut unit = DriveV2::new(context(rate), &s);
        let mut reference = unit.clone();
        let mut local = routes();
        for _ in 0..512 {
            assert_eq!(
                process_modulation(
                    &mut unit,
                    &mut local,
                    [0.01; 2],
                    &[value],
                    &[],
                    Default::default()
                ),
                reference.process_sample([0.01; 2])
            );
        }
        let mut shared = routes();
        for _ in 0..rate / 20 {
            assert!(
                process_modulation(
                    &mut unit,
                    &mut shared,
                    [0.01; 2],
                    &[],
                    &[value],
                    Default::default()
                )
                .iter()
                .all(|v| v.is_finite())
            );
        }
        let p = unit.effective_parameters();
        for (v, expected) in p.iter().zip([12.0, 1.0, 1.0, 1.0, 2000.0, 12.0, 1.0]) {
            assert!((v - expected).abs() < 0.01, "{p:?}");
        }
        let silent = DriveV2Settings {
            input_gain_db: SILENT_GAIN_DB,
            level_db: SILENT_GAIN_DB,
            ..s
        };
        let mut unit = DriveV2::new(context(rate), &silent);
        for _ in 0..512 {
            assert_eq!(
                process_modulation(
                    &mut unit,
                    &mut shared,
                    [0.5; 2],
                    &[],
                    &[value],
                    Default::default()
                ),
                [0.0; 2]
            );
        }
    }
}

#[test]
fn tube_is_asymmetric_finite_silent_and_preserves_mode_transition_history() {
    for rate in RATES {
        let settings = DriveV2Settings {
            clip: DriveV2Clip::Tube,
            gain: 0.7,
            ..Default::default()
        };
        let mut silence = DriveV2::new(context(rate), &settings);
        for _ in 0..1000 {
            assert_eq!(silence.process_sample([0.0; 2]), [0.0; 2]);
        }
        let mut positive = DriveV2::new(context(rate), &settings);
        let mut negative = positive.clone();
        let mut a = [0.0; 2];
        let mut b = a;
        for _ in 0..2000 {
            a = positive.process_sample([0.4; 2]);
            b = negative.process_sample([-0.4; 2]);
        }
        assert!((a[0] + b[0]).abs() > 0.05);
        assert!(a.into_iter().chain(b).all(f32::is_finite));
        for clip in DriveV2Clip::ALL {
            positive.retarget(&DriveV2Settings {
                clip,
                ..settings.clone()
            });
            let mut same = positive.clone();
            for i in 0..1000 {
                let sample = [(i as f32 * 0.17).sin() * 2.0, (i as f32 * 0.19).cos()];
                let out = positive.process_sample(sample);
                assert_eq!(out, same.process_sample(sample));
                assert!(out.into_iter().all(f32::is_finite));
            }
        }
        positive.retarget(&settings);
        positive.reset();
        let mut fresh = DriveV2::new(context(rate), &settings);
        for _ in 0..1000 {
            assert_eq!(
                positive.process_sample([0.2, -0.2]),
                fresh.process_sample([0.2, -0.2])
            );
        }
    }
}

#[test]
fn tube_does_not_add_peak_headroom_for_full_scale_input() {
    for rate in RATES {
        for gain in [0.0, 0.001, 0.2, 0.7, 1.0] {
            for input in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                let settings = DriveV2Settings {
                    clip: DriveV2Clip::Tube,
                    gain,
                    ..Default::default()
                };
                let mut unit = DriveV2::new(context(rate), &settings);
                for _ in 0..1000 {
                    let output = unit.process_sample([input; 2]);
                    assert!(
                        output.iter().all(|v| v.is_finite() && v.abs() <= 1.00001),
                        "rate={rate}, gain={gain}, input={input}: {output:?}"
                    );
                }
            }
        }
    }
}
