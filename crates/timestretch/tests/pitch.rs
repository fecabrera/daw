//! Generated signals and crossing/projection measurements independent of the DSP.
use std::{f64::consts::TAU, ops::ControlFlow};
use timestretch::{Error, Pitch, Ratio, Request, SAMPLE_RATES};

#[test]
fn pitch_memory_estimate_matches_admission_and_rejects_invalid_dimensions() {
    for offset in [-127.0, -24.0, 0.0, 24.0, 127.0] {
        let mut request = Request::new(48000, 2, Ratio::new(3, 2).unwrap())
            .with_pitch(Pitch::from_semitones(offset).unwrap());
        let input: Vec<_> = (0..2048).map(|i| (i as f32 * 0.07).sin()).collect();
        let bytes = request.working_memory_bytes(1024).unwrap();
        request.stretch.memory_limit_bytes = bytes - 1;
        assert_eq!(request.process(&input), Err(Error::ResourceLimit));
        request.stretch.memory_limit_bytes = bytes;
        assert_eq!(request.process(&input).unwrap().len(), 3072);
        assert!(request.working_memory_bytes(usize::MAX).is_err());
        request.stretch.channels = 3;
        assert_eq!(
            request.working_memory_bytes(1024),
            Err(Error::InvalidLayout)
        );
        request.stretch.sample_rate = 0;
        assert_eq!(request.working_memory_bytes(1024), Err(Error::InvalidRate));
    }
}

fn sine(rate: u32, hz: f64, seconds: f64) -> Vec<f32> {
    (0..(f64::from(rate) * seconds).ceil() as usize)
        .map(|i| (TAU * hz * i as f64 / f64::from(rate)).sin() as f32 * 0.4)
        .collect()
}
fn frequency(samples: &[f32], rate: u32) -> f64 {
    let crossings: Vec<_> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, s)| s[0] <= 0.0 && s[1] > 0.0)
        .map(|(i, s)| i as f64 - f64::from(s[0]) / f64::from(s[1] - s[0]))
        .collect();
    assert!(
        crossings.len() >= 21,
        "insufficient cycles: {}",
        crossings.len()
    );
    (crossings.len() - 1) as f64 * f64::from(rate) / (crossings.last().unwrap() - crossings[0])
}
fn rms(samples: &[f32]) -> f64 {
    (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
}
fn check_tone_at(rate: u32, hz: f64, ratio: Ratio, semitones: f64) {
    let pitch = Pitch::from_semitones(semitones).unwrap();
    let expected = hz * pitch.factor();
    let seconds = (80.0 / expected / ratio.as_f64()).max(0.5);
    let input = sine(rate, hz, seconds);
    let output = Request::new(rate, 1, ratio)
        .with_pitch(pitch)
        .process(&input)
        .unwrap();
    assert_eq!(output.len(), ratio.output_frames(input.len()).unwrap());
    let middle = &output[output.len() / 4..output.len() * 3 / 4];
    let cents = 1200.0 * (frequency(middle, rate) / expected).log2();
    let db = 20.0 * (rms(middle) / (0.4 / 2f64.sqrt())).log10();
    let ordinary = (0.5..=2.0).contains(&ratio.as_f64()) && (-12.0..=12.0).contains(&semitones);
    assert!(
        cents.abs() <= if ordinary { 5.0 } else { 15.0 },
        "rate={rate} duration={ratio} pitch={semitones}: {cents} cents"
    );
    assert!(
        db.abs() <= if ordinary { 0.5 } else { 1.5 },
        "rate={rate} duration={ratio} pitch={semitones}: {db} dB"
    );
}
fn check_tone(rate: u32, ratio: Ratio, semitones: f64) {
    check_tone_at(rate, 440.0, ratio, semitones);
}

#[test]
fn full_range_pitch_tones_preserve_frequency_and_duration() {
    for (hz, semitones) in [
        (440.0, -36.0),
        (440.0, 36.0),
        (8000.0, -96.0),
        (32.0, 96.0),
        (8.1757989156, 127.0),
        (12543.853951, -127.0),
    ] {
        let pitch = Pitch::from_semitones(semitones).unwrap();
        let expected = hz * pitch.factor();
        let input = sine(48000, hz, (48.0 / hz).max(48.0 / expected).max(0.5));
        let output = Request::new(48000, 1, Ratio::new(1, 1).unwrap())
            .with_pitch(pitch)
            .process(&input)
            .unwrap();
        assert_eq!(output.len(), input.len());
        let middle = &output[output.len() / 4..output.len() * 3 / 4];
        let cents = (frequency(middle, 48000) / expected).log2() * 1200.0;
        let db = 20.0 * (rms(middle) / (0.4 / 2f64.sqrt())).log10();
        assert!(
            cents.abs() < 15.0,
            "pitch={semitones}: {cents} cents, {db} dB"
        );
        assert!(db.abs() < 3.0, "pitch={semitones}: {db} dB");
    }
}

#[test]
fn full_range_stages_bound_memory_retain_stereo_and_cancel_between_passes() {
    for offset in [-127.0, -96.0, -25.5, 25.5, 96.0, 127.0] {
        let pitch = Pitch::from_semitones(offset).unwrap();
        assert!(pitch.stages().all(|p| p.semitones().abs() <= 24.0));
        assert!((pitch.stages().map(|p| p.semitones()).sum::<f64>() - offset).abs() < 1e-12);
        for rate in SAMPLE_RATES {
            let input: Vec<_> = sine(rate, 440.0, 0.025)
                .iter()
                .flat_map(|v| [*v, -*v])
                .collect();
            let request = Request::new(rate, 2, Ratio::new(3, 2).unwrap()).with_pitch(pitch);
            assert!(request.working_memory_bytes(input.len() / 2).unwrap() < 32 * 1024 * 1024);
            let output = request.process(&input).unwrap();
            assert_eq!(
                output.len(),
                request
                    .stretch
                    .ratio
                    .output_frames(input.len() / 2)
                    .unwrap()
                    * 2
            );
            assert!(
                output
                    .chunks_exact(2)
                    .all(|s| s[0].is_finite() && (s[0] + s[1]).abs() < 0.0001)
            );
            let mut previous = 0.0;
            assert_eq!(
                request.process_with_progress(&input, |p| {
                    assert!(p >= previous);
                    previous = p;
                    if p >= 0.55 {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                }),
                Err(Error::Cancelled)
            );
        }
    }
}

#[test]
/// TP-01: invalid offsets and exact compatibility of the unison path.
fn validation_and_zero_pitch_preserve_existing_contract() {
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -127.001,
        127.001,
    ] {
        assert_eq!(Pitch::from_semitones(value), Err(Error::InvalidPitch));
    }
    for text in ["NaN", "inf", "128", "-128", "invalid"] {
        assert_eq!(text.parse::<Pitch>(), Err(Error::InvalidPitch));
    }
    assert_eq!(Pitch::default(), Pitch::UNISON);
    assert_eq!("-0".parse::<Pitch>().unwrap(), Pitch::UNISON);
    assert_eq!("12".parse::<Pitch>().unwrap().factor(), 2.0);
    assert_eq!("-24".parse::<Pitch>().unwrap().factor(), 0.25);
    for ratio in [Ratio::new(1, 1).unwrap(), Ratio::new(3, 2).unwrap()] {
        let request = Request::new(48000, 1, ratio);
        let input = sine(48000, 440.0, 0.05);
        assert_eq!(
            request.process(&input),
            request.with_pitch(Pitch::UNISON).process(&input)
        );
        assert_eq!(
            request.with_pitch(Pitch::UNISON).process(&[f32::NAN]),
            Err(Error::NonfiniteInput)
        );
    }
    let input = [0.0, -0.0, 1.25, -2.0];
    let output = Request::new(48000, 1, Ratio::new(1, 1).unwrap())
        .with_pitch(Pitch::UNISON)
        .process(&input)
        .unwrap();
    assert_eq!(
        input.map(f32::to_bits),
        output
            .iter()
            .map(|s| s.to_bits())
            .collect::<Vec<_>>()
            .as_slice()
    );
}

#[test]
/// TP-03: measured shifted frequency/level, all rates, low/mid/high tones.
fn pitch_and_level_at_every_rate_and_fractional_semitones() {
    for rate in SAMPLE_RATES {
        for semitones in [-24.0, -12.0, -0.5, 7.0, 12.0, 24.0] {
            check_tone(rate, Ratio::new(1, 1).unwrap(), semitones);
        }
        for hz in [55.0, 1000.0, 5000.0] {
            for semitones in [-24.0, 24.0] {
                check_tone_at(rate, hz, Ratio::new(1, 1).unwrap(), semitones);
            }
        }
    }
}

#[test]
/// TP-02/03: exact final length and pitched tones at combined staging extremes.
fn independent_duration_and_pitch_include_combined_extremes() {
    for (p, q) in [(1, 8), (1, 2), (3, 2), (2, 1), (8, 1)] {
        for semitones in [-24.0, -12.0, 7.0, 12.0, 24.0] {
            check_tone(48000, Ratio::new(p, q).unwrap(), semitones);
        }
    }
}

#[test]
/// TP-02: rate/layout validation, tiny results, exact silence and DC.
fn short_lengths_dc_silence_and_nonfinite_validation() {
    for rate in SAMPLE_RATES {
        for frames in [0, 1, 2, 7, 31, 257, 1025] {
            for (p, q) in [(1, 8), (1, 1), (3, 2), (8, 1)] {
                let ratio = Ratio::new(p, q).unwrap();
                for semitones in [-24.0, 7.5, 24.0] {
                    let request = Request::new(rate, 2, ratio)
                        .with_pitch(Pitch::from_semitones(semitones).unwrap());
                    for level in [0.0, 1.25] {
                        let output = request.process(&vec![level; frames * 2]);
                        match ratio.output_frames(frames) {
                            Ok(n) => {
                                let output = output.unwrap();
                                assert_eq!(output.len(), n * 2);
                                assert!(output.iter().all(|s| *s == level));
                            }
                            Err(e) => assert_eq!(output, Err(e)),
                        }
                    }
                }
            }
        }
    }
    let pitch = Pitch::from_semitones(12.0).unwrap();
    assert_eq!(
        Request::new(0, 1, Ratio::new(1, 1).unwrap())
            .with_pitch(pitch)
            .process(&[]),
        Err(Error::InvalidRate)
    );
    assert_eq!(
        Request::new(48000, 0, Ratio::new(1, 1).unwrap())
            .with_pitch(pitch)
            .process(&[0.0]),
        Err(Error::InvalidLayout)
    );
    assert_eq!(
        Request::new(48000, 2, Ratio::new(1, 1).unwrap())
            .with_pitch(pitch)
            .process(&[0.0]),
        Err(Error::InvalidLayout)
    );
    assert_eq!(
        Request::new(48000, 1, Ratio::new(1, 1).unwrap())
            .with_pitch(pitch)
            .process(&[f32::INFINITY]),
        Err(Error::NonfiniteInput)
    );
}

#[test]
/// TP-04: linked channel relationships, swap symmetry, and repeatability.
fn stereo_relationships_swapping_and_determinism() {
    let mono = sine(48000, 440.0, 0.12);
    for semitones in [-24.0, 7.5, 24.0] {
        let pitch = Pitch::from_semitones(semitones).unwrap();
        let ratio = Ratio::new(3, 2).unwrap();
        let expected = Request::new(48000, 1, ratio)
            .with_pitch(pitch)
            .process(&mono)
            .unwrap();
        for factor in [0.0, 0.5, 1.0, -1.0] {
            let input: Vec<_> = mono.iter().flat_map(|s| [*s, *s * factor]).collect();
            let request = Request::new(48000, 2, ratio).with_pitch(pitch);
            let output = request.process(&input).unwrap();
            assert_eq!(output, request.process(&input).unwrap());
            for (i, frame) in output.chunks_exact(2).enumerate() {
                assert!((frame[0] - expected[i]).abs() < 1e-6);
                assert!((frame[1] - frame[0] * factor).abs() < 1e-7);
            }
            let swapped: Vec<_> = input.chunks_exact(2).flat_map(|s| [s[1], s[0]]).collect();
            let swapped = request.process(&swapped).unwrap();
            for (a, b) in output.chunks_exact(2).zip(swapped.chunks_exact(2)) {
                assert_eq!(a, [b[1], b[0]]);
            }
        }
    }
}

#[test]
/// TP-06: cancellation at every stage and low-budget rejection.
fn cancellation_is_monotonic_through_each_pitch_stage_and_memory_is_preflighted() {
    let input = sine(48000, 440.0, 0.2);
    let mut request = Request::new(48000, 1, Ratio::new(3, 2).unwrap())
        .with_pitch(Pitch::from_semitones(7.0).unwrap());
    for threshold in [0.0, 0.03, 0.1, 0.4, 0.79, 0.82, 0.9, 0.999] {
        let mut last = 0.0;
        assert_eq!(
            request.process_with_progress(&input, |p| {
                assert!(p >= last && p <= 1.0, "{last} -> {p}");
                last = p;
                if p >= threshold {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
            Err(Error::Cancelled)
        );
    }
    request.stretch.memory_limit_bytes = 1;
    assert_eq!(request.process(&input), Err(Error::ResourceLimit));
    request.stretch.memory_limit_bytes = 8 * 1024 * 1024 + input.len() * 12;
    assert_eq!(request.process(&input), Err(Error::ResourceLimit));
}

#[test]
/// TP-05: independent stopband probe with a unity intermediate.
fn upward_pitch_filters_frequencies_above_output_nyquist() {
    // Duration 1/2 + pitch +12 has a unity intermediate: isolate the new filter.
    let ratio = Ratio::new(1, 2).unwrap();
    let request = Request::new(48000, 1, ratio).with_pitch(Pitch::from_semitones(12.0).unwrap());
    let low = request.process(&sine(48000, 2000.0, 0.2)).unwrap();
    let high = request.process(&sine(48000, 18000.0, 0.2)).unwrap();
    let mid = |s: &[f32]| rms(&s[s.len() / 4..s.len() * 3 / 4]);
    assert!(20.0 * (mid(&high) / mid(&low)).log10() < -70.0);
}

#[test]
/// TP-04: unrelated stereo, phase, amplitude scaling, and numerical extremes.
fn unrelated_channels_phase_headroom_and_short_nonconstant_signals() {
    let ratio = Ratio::new(1, 1).unwrap();
    for semitones in [-12.0, 7.5, 12.0] {
        let pitch = Pitch::from_semitones(semitones).unwrap();
        let request = Request::new(48000, 2, ratio).with_pitch(pitch);
        let left = sine(48000, 440.0, 0.5);
        let right = sine(48000, 997.0, 0.5);
        let input: Vec<_> = left
            .iter()
            .zip(&right)
            .flat_map(|(l, r)| [*l, *r])
            .collect();
        let output = request.process(&input).unwrap();
        for (c, hz) in [(0, 440.0), (1, 997.0)] {
            let channel: Vec<_> = output.chunks_exact(2).map(|s| s[c]).collect();
            let measured = frequency(&channel[channel.len() / 4..channel.len() * 3 / 4], 48000);
            assert!((1200.0 * (measured / (hz * pitch.factor())).log2()).abs() < 5.0);
        }
        let phase = 0.7;
        let input: Vec<_> = (0..24000)
            .flat_map(|i| {
                let angle = TAU * 440.0 * i as f64 / 48000.0;
                [angle.sin() as f32 * 2.0, (angle + phase).sin() as f32 * 2.0]
            })
            .collect();
        let output = request.process(&input).unwrap();
        assert!(output.iter().any(|s| s.abs() > 1.5));
        let mut measured_phase = [0.0; 2];
        for (c, p) in measured_phase.iter_mut().enumerate() {
            let (mut real, mut imaginary) = (0.0, 0.0);
            for i in 6000..18000 {
                let angle = TAU * 440.0 * pitch.factor() * i as f64 / 48000.0;
                real += f64::from(output[i * 2 + c]) * angle.sin();
                imaginary += f64::from(output[i * 2 + c]) * angle.cos();
            }
            *p = imaginary.atan2(real);
        }
        let error = ((measured_phase[1] - measured_phase[0] - phase + std::f64::consts::PI)
            .rem_euclid(TAU)
            - std::f64::consts::PI)
            .to_degrees();
        assert!(error.abs() < 5.0, "{semitones}: {error} degrees");
        let half: Vec<_> = input.iter().map(|s| s * 0.5).collect();
        let half = request.process(&half).unwrap();
        let residual: Vec<_> = output.iter().zip(half).map(|(a, b)| a - 2.0 * b).collect();
        assert!(rms(&residual) / rms(&output) < 1e-3);
    }
    for frames in [2, 7, 31, 257, 1025] {
        for semitones in [-24.0, 24.0] {
            let input: Vec<_> = (0..frames)
                .map(|i| if i % 3 == 0 { 1.25 } else { -0.25 })
                .collect();
            let request =
                Request::new(48000, 1, ratio).with_pitch(Pitch::from_semitones(semitones).unwrap());
            let output = request.process(&input).unwrap();
            assert_eq!(output.len(), frames);
            assert!(output.iter().all(|s| s.is_finite()));
            assert!(output.iter().any(|s| s.abs() > 1e-8));
        }
    }
    let extreme: Vec<_> = (0..257)
        .map(|i| if i % 3 == 0 { f32::MAX } else { 0.0 })
        .collect();
    let result = Request::new(48000, 1, ratio)
        .with_pitch(Pitch::from_semitones(12.0).unwrap())
        .process(&extreme);
    assert!(
        matches!(result, Err(Error::NumericalFailure))
            || result.unwrap().iter().all(|s| s.is_finite())
    );
}
