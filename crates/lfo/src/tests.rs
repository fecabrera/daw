use super::*;
use daw_unit_core::{LfoWave, NoteRate};
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn clock(frame: u64) -> ModulationSourceContext {
    ModulationSourceContext {
        frame,
        ..Default::default()
    }
}
fn settings() -> LfoSettings {
    LfoSettings {
        rate_mode: LfoRateMode::Hz,
        rate_hz: 1.0,
        smooth_ms: 0.0,
        polarity: LfoPolarity::Bipolar,
        ..Default::default()
    }
}
#[test]
fn sine_phase_polarity_and_note_rates_match_the_declared_clock_at_all_rates() {
    for rate in RATES {
        let mut lfo = Lfo::new(settings(), context(rate));
        assert!(lfo.descriptor().provides_modulation());
        assert!(!lfo.descriptor().is_source);
        for (fraction, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 0.0), (0.75, -1.0)] {
            assert!(
                (lfo.sample_at(clock((fraction * f64::from(rate)) as u64)) - expected).abs() < 1e-5
            );
        }
        let mut unipolar = Lfo::new(
            LfoSettings {
                polarity: LfoPolarity::Unipolar,
                ..settings()
            },
            context(rate),
        );
        assert_eq!(unipolar.sample_at(clock(0)), 0.5);
        assert_eq!(unipolar.sample_at(clock(u64::from(rate) / 4)), 1.0);
        let mut phase = Lfo::new(
            LfoSettings {
                phase_degrees: 180.0,
                ..settings()
            },
            context(rate),
        );
        assert!((phase.sample_at(clock(u64::from(rate) / 4)) + 1.0).abs() < 1e-5);
        for note in 0..NoteRate::COUNT {
            for tempo in [60.0, 120.0, 175.0] {
                let note_rate = NoteRate(note);
                let mut unit = Lfo::new(
                    LfoSettings {
                        rate_mode: LfoRateMode::Note,
                        note_rate,
                        ..settings()
                    },
                    context(rate),
                );
                let at = (f64::from(rate) * note_rate.beats() * 60.0 / f64::from(tempo) / 4.0)
                    .round() as u64;
                assert!(
                    (unit.sample_at(ModulationSourceContext {
                        tempo_bpm: tempo,
                        ..clock(at)
                    }) - 1.0)
                        .abs()
                        < 0.0001
                );
            }
        }
    }
}
#[test]
fn every_wave_and_smoothing_setting_has_deterministic_bounded_seek_and_periodic_output() {
    for rate in RATES {
        for wave in LfoWave::ALL {
            for smooth_ms in [0.0, 1.0, 100.0] {
                let settings = LfoSettings {
                    wave: *wave,
                    smooth_ms,
                    ..settings()
                };
                let mut played = Lfo::new(settings, context(rate));
                for frame in 0..1200 {
                    let value = played.sample_at(clock(frame));
                    assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
                    if frame % 79 == 0 {
                        let mut sought = Lfo::new(settings, context(rate));
                        assert_eq!(value, sought.sample_at(clock(frame)));
                        assert!(
                            (value - sought.sample_at(clock(frame + u64::from(rate)))).abs() < 1e-5
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn note_origins_restart_shared_cycles_without_owning_note_release() {
    for restart in LfoRestart::ALL {
        let mut lfo = Lfo::new(
            LfoSettings {
                restart,
                ..settings()
            },
            context(48000),
        );
        let c = ModulationSourceContext {
            frame: 24000,
            note_origin: Some(12000),
            legato_origin: Some(0),
            ..Default::default()
        };
        let expected = if restart == LfoRestart::Retrigger {
            1.0
        } else {
            0.0
        };
        assert!((lfo.sample_at(c) - expected).abs() < 1e-5);
        let mut output = daw_unit_core::ModulationOutput::new(
            daw_unit_core::ModulationSourceId {
                layer: daw_unit_core::EnvelopeInstance::default().id,
                unit: daw_unit_core::EnvelopeInstance::default().id,
            },
            true,
            context(48000),
        );
        let sample = output.sample(&mut lfo, c);
        assert_eq!(sample.behavior, ModulationBehavior::Continuous);
        assert!(sample.release_age.is_none());
    }
}
#[test]
fn smoothing_softens_edges_and_live_settings_settle_without_resetting_the_clock() {
    let c = context(48000);
    let mut sharp = Lfo::new(
        LfoSettings {
            wave: LfoWave::Square,
            ..settings()
        },
        c,
    );
    let mut smooth = Lfo::new(
        LfoSettings {
            wave: LfoWave::Square,
            smooth_ms: 20.0,
            ..settings()
        },
        c,
    );
    assert_eq!(sharp.sample_at(clock(24000)), -1.0);
    assert!(smooth.sample_at(clock(24000)) > 0.99);
    assert!(smooth.sample_at(clock(24480)).abs() < 0.99);
    let mut unit = Lfo::new(settings(), c);
    for frame in 0..12000 {
        unit.sample_at(clock(frame));
    }
    let mut changed = settings();
    changed.wave = LfoWave::Triangle;
    unit.set_settings(changed);
    assert!(unit.sample_at(clock(12000)) > 0.99);
    for frame in 12001..12300 {
        unit.sample_at(clock(frame));
    }
    let mut target = Lfo::new(changed, c);
    assert_eq!(unit.sample_at(clock(12300)), target.sample_at(clock(12300)));
    unit.reset();
    assert_eq!(unit.sample_at(clock(12300)), target.sample_at(clock(12300)));
}

#[test]
fn frequency_bounds_smoothing_and_live_tempo_changes_are_finite_at_all_project_rates() {
    for rate in RATES {
        for wave in LfoWave::ALL {
            for hz in [0.01, 20.0] {
                for ms in [0.0, 0.001, 100.0] {
                    let mut unit = Lfo::new(
                        LfoSettings {
                            wave: *wave,
                            rate_hz: hz,
                            smooth_ms: ms,
                            ..settings()
                        },
                        context(rate),
                    );
                    for frame in [0, 1, 10, u64::from(rate), u64::from(rate) * 100] {
                        let value = unit.sample_at(clock(frame));
                        assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
                    }
                }
            }
        }
        let mut clock = LfoClock::new(context(rate));
        let at = u64::from(rate) / 3;
        let before = clock.phase_at(at, 0, 2.0);
        let changed = clock.phase_at(at + 1, 0, 3.0);
        assert!((changed - (before + 2.0 / f64::from(rate))).abs() < 1e-12);
        for frame in at + 2..=at + u64::from(rate / 200) + 2 {
            clock.phase_at(frame, 0, 3.0);
        }
        let end = at + u64::from(rate / 200) + 3;
        assert!(
            (clock.phase_at(end, 0, 3.0) - (end as f64 * 3.0 / f64::from(rate)).rem_euclid(1.0))
                .abs()
                < 1e-12
        );
    }
}
