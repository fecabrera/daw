use super::*;
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
#[test]
fn dry_bypass_silence_and_output_level_are_independent_at_all_rates() {
    for rate in RATES {
        for s in [
            ChorusSettings {
                enabled: false,
                ..Default::default()
            },
            ChorusSettings {
                dry_wet: 0.0,
                ..Default::default()
            },
        ] {
            let mut unit = Chorus::new(context(rate), &s);
            for _ in 0..rate / 10 {
                assert_eq!(unit.process_sample([0.4, -0.2]), [0.4, -0.2]);
            }
        }
        let mut unit = Chorus::new(
            context(rate),
            &ChorusSettings {
                dry_wet: 0.0,
                level_db: -6.0,
                ..Default::default()
            },
        );
        assert!((unit.process_sample([1.0; 2])[0] - gain(-6.0)).abs() < 1e-7);
        let mut silent = Chorus::new(
            context(rate),
            &ChorusSettings {
                level_db: SILENT_GAIN_DB,
                ..Default::default()
            },
        );
        for _ in 0..rate / 10 {
            assert_eq!(silent.process_sample([1.0; 2]), [0.0; 2]);
        }
    }
}
#[test]
fn static_delay_is_fractional_and_stereo_feedback_has_the_correct_sign() {
    for rate in RATES {
        for feedback in [-0.8, 0.0, 0.8] {
            let s = ChorusSettings {
                depth: 0.0,
                delay_ms: 20.0,
                feedback,
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut unit = Chorus::new(context(rate), &s);
            let first = rate as usize / 50;
            for frame in 0..first * 3 {
                let v = unit.process_sample(if frame == 0 { [1.0, 0.0] } else { [0.0; 2] });
                assert_eq!(v[1], 0.0);
                if frame == first {
                    assert!((v[0] - 1.0).abs() < 1e-6);
                }
                if frame == first * 2 {
                    assert!((v[0] - feedback).abs() < 1e-6);
                }
            }
        }
        // 15 ms is half a sample at 44.1 kHz: adjacent taps must retain total amplitude.
        let s = ChorusSettings {
            depth: 0.0,
            dry_wet: 1.0,
            ..Default::default()
        };
        let mut unit = Chorus::new(context(rate), &s);
        let mut sum = 0.0;
        for frame in 0..rate / 20 {
            sum += unit.process_sample(if frame == 0 { [1.0; 2] } else { [0.0; 2] })[0];
        }
        assert!((sum - 1.0).abs() < 1e-6);
        assert_eq!(unit.latency_frames(), 0);
    }
}
#[test]
fn mono_spreads_width_zero_collapses_wet_and_dry_is_unmodified() {
    for rate in RATES {
        let mut stereo = Chorus::new(
            context(rate),
            &ChorusSettings {
                dry_wet: 1.0,
                ..Default::default()
            },
        );
        let mut mono = Chorus::new(
            context(rate),
            &ChorusSettings {
                width: 0.0,
                dry_wet: 1.0,
                ..Default::default()
            },
        );
        let mut difference = 0.0;
        for frame in 0..rate / 4 {
            let input = [(frame as f32 * 0.1).sin(); 2];
            let a = stereo.process_sample(input);
            let b = mono.process_sample(input);
            difference += (a[0] - a[1]).abs();
            assert_eq!(b[0], b[1]);
        }
        assert!(difference > 1.0);
    }
}
#[test]
fn retarget_preserves_delay_clock_and_meter_bypass_detaches_clones() {
    for rate in RATES {
        let mut s = ChorusSettings::default();
        let mut unit = Chorus::new(context(rate), &s);
        let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        unit.set_meter(meter.clone());
        for _ in 0..1000 {
            unit.process_sample([0.4; 2]);
        }
        assert!(meter[0].load(std::sync::atomic::Ordering::Relaxed) > 0);
        let mut reference = unit.clone();
        let (cursor, phase) = (unit.cursor, unit.phase);
        s.enabled = false;
        unit.retarget(&s);
        assert_eq!((unit.cursor, unit.phase), (cursor, phase));
        for _ in 0..rate / 100 {
            unit.process_sample([0.4; 2]);
            reference.process_sample([0.4; 2]);
        }
        assert!(
            meter
                .iter()
                .all(|v| v.load(std::sync::atomic::Ordering::Relaxed) == 0)
        );
        s.enabled = true;
        unit.retarget(&s);
        for _ in 0..rate / 100 {
            unit.process_sample([0.4; 2]);
            reference.process_sample([0.4; 2]);
        }
        assert_eq!(
            unit.process_sample([0.4; 2]),
            reference.process_sample([0.4; 2])
        );
        s.voices = 4;
        s.delay_ms = 30.0;
        unit.retarget(&s);
        assert_ne!(unit.phase, 0.0);
        for _ in 0..rate / 100 {
            assert!(
                unit.process_sample([f32::MAX, f32::NAN])
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
        unit.reset();
        assert_eq!(unit.phase, 0.0);
        assert!(unit.delay.iter().all(|v| *v == [0.0; 2]));
    }
}
#[test]
fn external_offsets_are_smoothed_bounded_and_all_controls_are_discovered() {
    for rate in RATES {
        let mut unit = Chorus::new(context(rate), &Default::default());
        assert_eq!(Chorus::MODULATION_PARAMETERS, &ModulationParameter::CHORUS);
        for sign in [-1.0, 1.0] {
            for _ in 0..rate / 100 {
                assert!(
                    unit.process_modulated([f32::INFINITY, -0.3], [sign * 1e9; 8])
                        .iter()
                        .all(|v| v.is_finite())
                );
            }
            let p = unit.effective_parameters();
            assert!((5.0..=30.0).contains(&p[0]));
            assert!((0.0..=1.0).contains(&p[1]));
            assert!((-0.8..=0.8).contains(&p[2]));
            assert!((0.05..=10.0).contains(&p[3]));
            assert!((-180.0..=180.0).contains(&p[4]));
            assert!((0.0..=2.0).contains(&p[5]));
            assert!(p[6] <= 12.001);
            assert!((0.0..=1.0).contains(&p[7]));
        }
    }
}

#[test]
fn note_rates_follow_tempo_transport_wraps_and_modulation_at_every_project_rate() {
    use daw_unit_core::{LfoRateMode, NoteRate};
    for sample_rate in RATES {
        for note in 0..NoteRate::COUNT {
            for tempo in [30.0, 120.0, 300.0] {
                let settings = ChorusSettings {
                    rate_mode: LfoRateMode::Note,
                    note_rate: NoteRate(note),
                    ..Default::default()
                };
                let mut unit = Chorus::new(context(sample_rate), &settings);
                let hz = settings.base_rate_hz(tempo);
                for frame in [0, 12345, 12346, 0] {
                    unit.process_modulated_at([0.0; 2], frame, tempo, [0.0; 8]);
                    assert!((f64::from(unit.effective_parameters()[3]) - hz).abs() < hz * 1e-6);
                    let expected =
                        ((frame + 1) as f64 * hz / f64::from(sample_rate)).rem_euclid(1.0);
                    assert!(
                        (unit.phase - expected).abs() < 1e-10,
                        "note {note}, tempo {tempo}, frame {frame}"
                    );
                }
                // Rate offsets retain the shared octave mapping in Note mode, including
                // note rates outside the free-running 0.05–10 Hz control range.
                let mut offsets = [0.0; 8];
                offsets[3] = 1.0;
                for frame in 1..sample_rate / 50 {
                    unit.process_modulated_at([0.0; 2], u64::from(frame), tempo, offsets);
                }
                let [lo, hi] = settings.rate_bounds(tempo);
                assert!(
                    (f64::from(unit.effective_parameters()[3]) - (hz * 2.0).clamp(lo, hi)).abs()
                        < hz * 1e-5
                );
            }
        }
        let settings = ChorusSettings {
            rate_mode: LfoRateMode::Note,
            ..Default::default()
        };
        let mut unit = Chorus::new(context(sample_rate), &settings);
        let frame = u64::from(sample_rate / 3);
        unit.process_modulated_at([0.0; 2], frame, 120.0, [0.0; 8]);
        let before = unit.phase;
        unit.process_modulated_at([0.0; 2], frame + 1, 180.0, [0.0; 8]);
        let distance = (unit.phase - before + 0.5).rem_euclid(1.0) - 0.5;
        assert!(distance.abs() < 10.0 / f64::from(sample_rate));
        for i in 2..sample_rate / 100 {
            unit.process_modulated_at([0.0; 2], frame + u64::from(i), 180.0, [0.0; 8]);
        }
        let expected = ((frame + u64::from(sample_rate / 100)) as f64 * 3.0
            / f64::from(sample_rate))
        .rem_euclid(1.0);
        assert!((unit.phase - expected).abs() < 1e-9);
    }
}

#[test]
fn rendered_pitch_deviation_stays_consistent_for_fast_hz_note_tempo_and_rate_cv() {
    use daw_unit_core::{LfoRateMode, NoteRate};
    // Estimate frequency from interpolated positive zero crossings of a fully wet
    // 1 kHz tone. This measures the rendered audio, independent of the delay formula.
    for sample_rate in RATES {
        let mut reference: Option<f64> = None;
        for (mode, hz, note, tempo, rate_offset) in [
            (LfoRateMode::Hz, 0.8, NoteRate(12), 120.0, 0.0),
            (LfoRateMode::Hz, 1.6, NoteRate(12), 120.0, 0.0),
            (LfoRateMode::Hz, 10.0, NoteRate(12), 120.0, 0.0),
            (LfoRateMode::Hz, 0.8, NoteRate(12), 120.0, 3.0),
            (LfoRateMode::Note, 0.8, NoteRate(12), 100.0, 0.0),
            (LfoRateMode::Note, 0.8, NoteRate(18), 100.0, 0.0),
            (LfoRateMode::Note, 0.8, NoteRate(26), 300.0, 0.0),
            (LfoRateMode::Note, 0.8, NoteRate(18), 100.0, 2.0),
        ] {
            let settings = ChorusSettings {
                voices: 1,
                stereo_phase_degrees: 0.0,
                dry_wet: 1.0,
                rate_mode: mode,
                rate_hz: hz,
                note_rate: note,
                ..Default::default()
            };
            let mut unit = Chorus::new(context(sample_rate), &settings);
            let mut offsets = [0.0; 8];
            offsets[3] = rate_offset;
            let mut previous = 0.0;
            let mut last_crossing: Option<f64> = None;
            let mut peak_deviation = 0.0_f64;
            for frame in 0..sample_rate * 3 / 2 {
                let input = (std::f64::consts::TAU * 1000.0 * f64::from(frame)
                    / f64::from(sample_rate))
                .sin() as f32;
                let sample = f64::from(
                    unit.process_modulated_at([input; 2], u64::from(frame), tempo, offsets)[0],
                );
                if frame > sample_rate / 10 && previous < 0.0 && sample >= 0.0 {
                    let crossing = f64::from(frame) - sample / (sample - previous);
                    if let Some(last) = last_crossing {
                        let relative = f64::from(sample_rate) / (crossing - last) / 1000.0 - 1.0;
                        peak_deviation = peak_deviation.max(relative.abs());
                    }
                    last_crossing = Some(crossing);
                }
                previous = sample;
            }
            let reference = *reference.get_or_insert(peak_deviation);
            // Default settings should remain around +/-15 cents, and faster rates
            // should stay within 5% of that measured pitch excursion, including CV.
            assert!(
                (0.0075..0.0095).contains(&peak_deviation),
                "{sample_rate} {mode:?} {hz} {note:?} {tempo} {rate_offset}: {peak_deviation}"
            );
            assert!(
                (peak_deviation - reference).abs() < reference * 0.05,
                "{sample_rate} {mode:?} {hz} {note:?} {tempo} {rate_offset}: {peak_deviation} vs {reference}"
            );
        }
    }
}

#[test]
fn compensation_starts_at_host_tempo_is_smoothed_and_reset_reinitializes_it() {
    use daw_unit_core::{LfoRateMode, NoteRate};
    for sample_rate in RATES {
        let mut settings = ChorusSettings {
            rate_mode: LfoRateMode::Note,
            note_rate: NoteRate(18),
            ..Default::default()
        };
        let mut unit = Chorus::new(context(sample_rate), &settings);
        unit.process_modulated_at([0.1; 2], 0, 100.0, [0.0; 8]);
        let first = unit.depth_compensation.as_ref().unwrap().target();
        assert!((first - 0.12).abs() < 1e-6); // 1/16 at 100 BPM is 6 2/3 Hz.
        settings.note_rate = NoteRate(21);
        unit.retarget(&settings);
        unit.process_modulated_at([0.1; 2], 1, 100.0, [0.0; 8]);
        let ramp = unit.depth_compensation.as_ref().unwrap();
        assert!((ramp.target() - 0.06).abs() < 1e-6);
        // A clone retains the in-progress ramp rather than jumping to its target.
        let mut ramp = *ramp;
        assert!(ramp.next_value() > ramp.target());
        for frame in 2..sample_rate / 50 {
            assert!(
                unit.process_modulated_at([0.1; 2], u64::from(frame), 100.0, [0.0; 8])
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
        unit.reset();
        assert!(unit.depth_compensation.is_none());
        // Slow rates keep the original excursion instead of exceeding ring bounds.
        settings.note_rate = NoteRate(1);
        unit.retarget(&settings);
        unit.process_modulated_at([0.1; 2], 0, 30.0, [0.0; 8]);
        assert_eq!(unit.depth_compensation.as_ref().unwrap().target(), 1.0);
    }
}
