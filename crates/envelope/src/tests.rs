use super::*;
use daw_unit_core::{CurveShape, EnvelopeStageSettings, EnvelopeTiming};

#[test]
fn note_timing_scales_attack_and_decay_but_keeps_release_in_milliseconds() {
    for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
        for length in [rate / 4, rate, rate * 4] {
            let settings = EnvelopeSettings {
                timing: EnvelopeTiming::Note,
                attack_note: 0.25,
                decay_note: 0.5,
                ..settings()
            };
            let mut e = envelope(settings, rate);
            e.note_on_with_duration(Some(u64::from(length)));
            let attack = (f64::from(length) * 0.25).round() as u64;
            let decay = (f64::from(length) * 0.5).round() as u64;
            e.advance(attack - 1);
            assert_eq!(e.stage(), Stage::Attack);
            e.next_sample();
            assert_eq!(e.level(), 1.0);
            e.advance(decay - 1);
            assert_eq!(e.stage(), Stage::Decay);
            e.next_sample();
            assert_eq!(e.level(), settings.sustain);
            e.note_off();
            e.advance(settings.release.frames(rate) - 1);
            assert_eq!(e.stage(), Stage::Release);
            e.next_sample();
            assert!(e.is_idle());
        }
    }
}

#[test]
fn unknown_note_length_uses_retained_time_values_and_note_off_interrupts_long_stages() {
    let mut settings = settings();
    settings.timing = EnvelopeTiming::Note;
    settings.attack_note = 1.0;
    settings.decay_note = 1.0;
    let mut e = envelope(settings, 48_000);
    e.note_on();
    e.advance(settings.attack.frames(48_000));
    assert_eq!(e.stage(), Stage::Decay);
    e.note_on_with_duration(Some(48_000));
    e.advance(24_000);
    assert_eq!(e.level(), 0.5);
    e.note_off();
    assert_eq!(e.level(), 0.5);
    e.advance(settings.release.frames(48_000));
    assert!(e.is_idle());
    settings.attack_note = 0.0;
    settings.decay_note = 0.0;
    e.set_settings(settings);
    e.note_on_with_duration(Some(1));
    assert_eq!(e.stage(), Stage::Sustain);
    assert_eq!(e.level(), settings.sustain);
}

#[test]
fn note_timing_live_edits_anchor_level_and_reconstruction_matches_sample_processing() {
    let mut settings = EnvelopeSettings {
        timing: EnvelopeTiming::Note,
        attack_note: 1.0,
        decay_note: 0.4,
        ..settings()
    };
    let mut e = envelope(settings, 48_000);
    e.note_on_with_duration(Some(1000));
    e.advance(250);
    assert_eq!(e.level(), 0.25);
    settings.attack_note = 0.5;
    e.set_settings(settings);
    assert_eq!(e.level(), 0.25);
    e.advance(374);
    assert_eq!(e.stage(), Stage::Attack);
    e.next_sample();
    assert_eq!(e.stage(), Stage::Decay);
    for shape in CurveShape::ALL {
        settings.attack.shape = shape;
        settings.decay.shape = shape;
        settings.decay_note = 0.4;
        for age in [0, 1, 249, 500, 700, 1000] {
            let mut played = envelope(settings, 48_000);
            let mut sought = envelope(settings, 48_000);
            played.note_on_with_duration(Some(1000));
            sought.note_on_with_duration(Some(1000));
            for _ in 0..age {
                played.next_sample();
            }
            sought.advance(age);
            assert_eq!(played.stage(), sought.stage());
            assert!((played.level() - sought.level()).abs() < 1e-6);
        }
    }
}

fn settings() -> EnvelopeSettings {
    let linear = |time_ms| EnvelopeStageSettings {
        time_ms,
        shape: CurveShape::Linear,
        amount: 0.5,
    };
    EnvelopeSettings {
        attack: linear(20.0),
        decay: linear(40.0),
        sustain: 0.35,
        release: linear(60.0),
        ..Default::default()
    }
}
fn envelope(settings: EnvelopeSettings, rate: u32) -> Envelope {
    Envelope::new(
        settings,
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
    )
}

#[test]
fn adsr_reaches_endpoints_at_the_configured_times_at_all_project_rates() {
    for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
        let settings = settings();
        let mut e = envelope(settings, rate);
        e.note_on();
        for _ in 0..settings.attack.frames(rate) {
            e.next_sample();
        }
        assert_eq!(e.level(), 1.0);
        assert_eq!(e.stage(), Stage::Decay);
        for _ in 0..settings.decay.frames(rate) {
            e.next_sample();
        }
        assert_eq!(e.level(), 0.35);
        assert_eq!(e.stage(), Stage::Sustain);
        e.advance(u64::from(rate));
        assert_eq!(e.level(), 0.35);
        e.note_off();
        for _ in 0..settings.release.frames(rate) {
            e.next_sample();
        }
        assert!(e.is_idle());
        assert_eq!(e.level(), 0.0);
    }
}

#[test]
fn early_note_off_releases_from_the_current_attack_or_decay_level() {
    for frames in [200, 1200] {
        let mut e = envelope(settings(), 48_000);
        e.note_on();
        e.advance(frames);
        let held = e.level();
        e.note_off();
        assert_eq!(e.level(), held);
        assert_eq!(e.stage(), Stage::Release);
        let first = e.next_sample();
        assert!(first < held && first > held - 0.001);
        let mut last = first;
        for _ in 0..2880 {
            let value = e.next_sample();
            assert!((0.0..=last).contains(&value));
            last = value;
        }
        assert_eq!(last, 0.0);
    }
}

#[test]
fn zero_length_stages_and_zero_sustain_do_not_stick_or_produce_invalid_samples() {
    for sustain in [0.0, 1.0] {
        let mut settings = settings();
        settings.attack.time_ms = 0.0;
        settings.decay.time_ms = 0.0;
        settings.release.time_ms = 0.0;
        settings.sustain = sustain;
        let mut e = envelope(settings, 48_000);
        e.note_on();
        assert_eq!(e.next_sample(), sustain);
        e.note_off();
        assert_eq!(e.next_sample(), 0.0);
        assert!(e.is_idle());
        e.note_on();
        e.reset();
        assert_eq!(e.next_sample(), 0.0);
    }
}

#[test]
fn curve_and_time_edits_retain_stage_and_level_and_sustain_edits_are_smoothed() {
    let mut settings = settings();
    settings.attack.time_ms = 100.0;
    let mut e = envelope(settings, 48_000);
    e.note_on();
    e.advance(1200);
    let level = e.level();
    settings.attack.time_ms = 200.0;
    settings.attack.shape = CurveShape::Exponential;
    settings.attack.amount = 1.0;
    e.set_settings(settings);
    assert_eq!(e.stage(), Stage::Attack);
    assert_eq!(e.level(), level);
    e.advance(7199);
    assert_eq!(e.stage(), Stage::Attack);
    e.next_sample();
    assert_eq!(e.stage(), Stage::Decay);
    e.advance(1920);
    assert_eq!(e.stage(), Stage::Sustain);
    let held = e.level();
    settings.sustain = 0.9;
    e.set_settings(settings);
    assert_eq!(e.level(), held);
    assert!(e.next_sample() < held + 0.003);
    e.advance(239);
    assert_eq!(e.level(), 0.9);
    e.note_off();
    e.advance(100);
    let level = e.level();
    settings.release.shape = CurveShape::SCurve;
    settings.release.time_ms = 100.0;
    e.set_settings(settings);
    assert_eq!(e.stage(), Stage::Release);
    assert_eq!(e.level(), level);
    e.advance(4800);
    assert!(e.is_idle());
}

#[test]
fn fast_seek_reconstruction_matches_sample_processing_for_every_curve() {
    for shape in CurveShape::ALL {
        let mut settings = settings();
        settings.attack.shape = shape;
        settings.decay.shape = shape;
        settings.release.shape = shape;
        for frames in [0, 10, 900, 1000, 2800, 5000] {
            let mut expected = envelope(settings, 48_000);
            let mut actual = envelope(settings, 48_000);
            expected.note_on();
            actual.note_on();
            for _ in 0..frames {
                expected.next_sample();
            }
            actual.advance(frames);
            assert_eq!(actual.stage(), expected.stage());
            assert!((actual.level() - expected.level()).abs() < 1e-6);
            expected.note_off();
            actual.note_off();
            for _ in 0..1234 {
                expected.next_sample();
            }
            actual.advance(1234);
            assert!((actual.level() - expected.level()).abs() < 1e-6);
        }
    }
}

#[test]
fn changing_a_later_stage_does_not_reshape_the_current_stage() {
    let settings = settings();
    let mut actual = envelope(settings, 48_000);
    let mut expected = envelope(settings, 48_000);
    actual.note_on();
    expected.note_on();
    actual.advance(100);
    expected.advance(100);
    actual.set_settings(EnvelopeSettings {
        release: EnvelopeStageSettings {
            shape: CurveShape::SCurve,
            ..settings.release
        },
        ..settings
    });
    for _ in 0..500 {
        assert_eq!(actual.next_sample(), expected.next_sample());
    }
}
