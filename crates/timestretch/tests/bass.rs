//! Independent stationary bass mixtures and spectral measurements. No recorded media.
use rustfft::{FftPlanner, num_complex::Complex};
use std::f64::consts::TAU;
use timestretch::{Pitch, Ratio, Request, SAMPLE_RATES};

fn bass(rate: u32, harmonics: bool) -> Vec<f32> {
    (0..rate as usize * 3)
        .map(|frame| {
            let time = frame as f64 / f64::from(rate);
            [51.9130872, 65.40639133]
                .into_iter()
                .map(|hz| {
                    let phase = TAU * hz * time;
                    0.2 * phase.sin()
                        + if harmonics {
                            0.09 * (phase * 2.0 + 0.2).sin() + 0.04 * (phase * 3.0 + 0.4).sin()
                        } else {
                            0.0
                        }
                })
                .sum::<f64>() as f32
        })
        .collect()
}

// Hann-windowed, zero-padded spectrum, with log-power peak interpolation.
// Decimation is safe here: generated fixture partials are all below 400 Hz.
fn measured_partial(samples: &[f32], rate: u32, expected: f64) -> (f64, f64) {
    let samples: Vec<_> = samples[samples.len() / 4..samples.len() * 3 / 4]
        .iter()
        .step_by(8)
        .copied()
        .collect();
    let width = (samples.len() * 4).next_power_of_two();
    let mut spectrum = vec![Complex::default(); width];
    let mut weight = 0.0;
    for (i, sample) in samples.iter().enumerate() {
        let window = 0.5 - 0.5 * (TAU * i as f64 / samples.len() as f64).cos();
        spectrum[i].re = f64::from(*sample) * window;
        weight += window;
    }
    FftPlanner::new()
        .plan_fft_forward(width)
        .process(&mut spectrum);
    let bin_hz = f64::from(rate) / 8.0 / width as f64;
    let lo = ((expected - 5.0) / bin_hz).ceil() as usize;
    let hi = ((expected + 5.0) / bin_hz).floor() as usize;
    let peak = (lo..=hi)
        .max_by(|a, b| spectrum[*a].norm_sqr().total_cmp(&spectrum[*b].norm_sqr()))
        .unwrap();
    let a = spectrum[peak - 1].norm_sqr().ln();
    let b = spectrum[peak].norm_sqr().ln();
    let c = spectrum[peak + 1].norm_sqr().ln();
    let offset = 0.5 * (a - c) / (a - 2.0 * b + c);
    let hz = (peak as f64 + offset) * bin_hz;
    let amplitude = (b - 0.25 * (a - c) * offset).exp().sqrt() * 2.0 / weight;
    (hz, amplitude)
}

#[test]
fn bass_measurement_accepts_correct_control_and_rejects_pitch_and_level_errors() {
    let input = bass(48000, false);
    for expected in [51.9130872, 65.40639133] {
        let (hz, amplitude) = measured_partial(&input, 48000, expected);
        assert!((1200.0 * (hz / expected).log2()).abs() < 0.2);
        assert!((20.0 * (amplitude / 0.2).log10()).abs() < 0.01);
        assert!((1200.0 * (hz / (expected * 2f64.powf(6.0 / 12.0))).log2()).abs() > 500.0);
        assert!((20.0 * (amplitude / 0.4).log10()).abs() > 5.0);
    }
}

#[test]
fn neighboring_bass_notes_and_harmonics_retain_pitch_and_level() {
    for rate in SAMPLE_RATES {
        for harmonics in [false, true] {
            let input = bass(rate, harmonics);
            for (p, q, semitones) in [(17, 20, 6.0), (5, 4, -6.0), (3, 2, 0.0)] {
                let ratio = Ratio::new(p, q).unwrap();
                let pitch = Pitch::from_semitones(semitones).unwrap();
                let output = Request::new(rate, 1, ratio)
                    .with_pitch(pitch)
                    .process(&input)
                    .unwrap();
                assert_eq!(output.len(), ratio.output_frames(input.len()).unwrap());
                for fundamental in [51.9130872, 65.40639133] {
                    for (multiple, amplitude) in [(1.0, 0.2), (2.0, 0.09), (3.0, 0.04)] {
                        if !harmonics && multiple != 1.0 {
                            continue;
                        }
                        let expected = fundamental * multiple * pitch.factor();
                        let (hz, actual) = measured_partial(&output, rate, expected);
                        let cents = 1200.0 * (hz / expected).log2();
                        let db = 20.0 * (actual / amplitude).log10();
                        assert!(
                            cents.abs() <= 5.0,
                            "{rate} {harmonics} {p}/{q} {semitones} {expected} Hz: {cents} cents"
                        );
                        assert!(
                            db.abs() <= 0.5,
                            "{rate} {harmonics} {p}/{q} {semitones} {expected} Hz: {db} dB"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bass_analysis_retains_stereo_and_gain_without_channel_cancellation() {
    let mono = bass(48000, true);
    let request = Request::new(48000, 1, Ratio::new(17, 20).unwrap())
        .with_pitch(Pitch::from_semitones(6.0).unwrap());
    let reference = request.process(&mono).unwrap();
    for factor in [0.0, 0.5, 1.0, -1.0] {
        let input: Vec<_> = mono.iter().flat_map(|v| [*v, *v * factor]).collect();
        let mut stereo = request;
        stereo.stretch.channels = 2;
        let output = stereo.process(&input).unwrap();
        for (i, frame) in output.chunks_exact(2).enumerate() {
            assert!((frame[0] - reference[i]).abs() < 1e-6);
            assert!((frame[1] - factor * frame[0]).abs() < 1e-7);
        }
    }
    let input: Vec<_> = mono
        .iter()
        .enumerate()
        .flat_map(|(i, v)| {
            [
                *v,
                (TAU * 83.0 * i as f64 / 48000.0 + 0.8).sin() as f32 * 0.3,
            ]
        })
        .collect();
    let swapped: Vec<_> = input.chunks_exact(2).flat_map(|s| [s[1], s[0]]).collect();
    let mut stereo = request;
    stereo.stretch.channels = 2;
    let original = stereo.process(&input).unwrap();
    let reversed = stereo.process(&swapped).unwrap();
    assert!(
        original
            .chunks_exact(2)
            .zip(reversed.chunks_exact(2))
            .all(|(a, b)| { (a[0] - b[1]).abs() < 1e-6 && (a[1] - b[0]).abs() < 1e-6 })
    );
    let doubled: Vec<_> = mono.iter().map(|s| s * 2.0).collect();
    let doubled = request.process(&doubled).unwrap();
    assert!(
        doubled
            .iter()
            .zip(reference)
            .all(|(a, b)| (a - 2.0 * b).abs() < 1e-6)
    );
}

#[test]
fn drum_attacks_over_sustained_bass_stay_localized_when_stretched_and_pitched() {
    for rate in [48000, 96000] {
        let mut input = bass(rate, true);
        let mut random = 1729u32;
        for (frame, sample) in input.iter_mut().enumerate() {
            *sample *= 0.4;
            let time = frame as f64 / f64::from(rate);
            for onset in [0.6, 1.4, 2.2] {
                let relative = time - onset;
                if (0.0..0.06).contains(&relative) {
                    random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                    let noise = f64::from(random) / f64::from(u32::MAX) * 2.0 - 1.0;
                    *sample +=
                        (noise * (relative / 0.002).min(1.0) * (-relative / 0.008).exp()) as f32;
                }
            }
        }
        for (p, q, semitones) in [(3, 2, 0.0), (2, 1, 0.0), (17, 20, 6.0)] {
            let ratio = Ratio::new(p, q).unwrap();
            let output = Request::new(rate, 1, ratio)
                .with_pitch(Pitch::from_semitones(semitones).unwrap())
                .process(&input)
                .unwrap();
            for onset in [0.6, 1.4, 2.2] {
                let target = (onset * f64::from(rate) * ratio.as_f64()).round() as usize;
                // First differences suppress the low stationary bed. Measure the
                // energy distribution independently of the engine's detector/map.
                let radius = rate as usize / 10;
                let lo = target - radius;
                let hi = (target + radius).min(output.len() - 1);
                let energy: Vec<_> = output[lo..=hi]
                    .windows(2)
                    .map(|s| f64::from(s[1] - s[0]).powi(2))
                    .collect();
                let total: f64 = energy.iter().sum();
                assert!(total > 1.0, "lost attack {rate} {p}/{q} {semitones}");
                let quantile = |fraction: f64| {
                    let mut sum = 0.0;
                    lo + energy
                        .iter()
                        .position(|value| {
                            sum += value;
                            sum >= total * fraction
                        })
                        .unwrap()
                };
                let start = quantile(0.1);
                let end = quantile(0.9);
                assert!(
                    start.abs_diff(target) <= rate as usize / 200,
                    "attack timing {rate} {p}/{q} {semitones}: {} ms",
                    start.abs_diff(target) as f64 * 1000.0 / f64::from(rate)
                );
                assert!(
                    end - start <= rate as usize / 50,
                    "attack width {rate} {p}/{q} {semitones}: {} ms",
                    (end - start) as f64 * 1000.0 / f64::from(rate)
                );
            }
        }
    }
}

#[test]
fn bass_preparation_reuses_cancellation_and_memory_admission() {
    use std::ops::ControlFlow;
    use timestretch::Error;
    let input = bass(48000, true);
    let mut request = Request::new(48000, 1, Ratio::new(17, 20).unwrap())
        .with_pitch(Pitch::from_semitones(6.0).unwrap());
    let bytes = request.working_memory_bytes(input.len()).unwrap();
    request.stretch.memory_limit_bytes = bytes - 1;
    assert_eq!(request.process(&input), Err(Error::ResourceLimit));
    request.stretch.memory_limit_bytes = bytes;
    for threshold in [0.08, 0.14, 0.5, 0.95] {
        let mut previous = 0.0;
        assert_eq!(
            request.process_with_progress(&input, |p| {
                assert!(p >= previous);
                previous = p;
                if p >= threshold {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
            Err(Error::Cancelled)
        );
    }
    assert_eq!(
        request.process(&input).unwrap().len(),
        request.stretch.ratio.output_frames(input.len()).unwrap()
    );
}

#[test]
fn changing_bass_chords_retain_each_notes_partials() {
    for rate in [48000, 96000] {
        let chords = [
            [51.9130872, 65.40639133],
            [55.0, 69.29565774],
            [61.73541266, 77.78174593],
        ];
        let input: Vec<_> = chords
            .into_iter()
            .flat_map(|chord| {
                (0..rate).map(move |frame| {
                    let time = f64::from(frame) / f64::from(rate);
                    let gate = (time / 0.005).min((1.0 - time) / 0.005).min(1.0);
                    (chord
                        .into_iter()
                        .map(|hz| (TAU * hz * time).sin() * 0.2)
                        .sum::<f64>()
                        * gate) as f32
                })
            })
            .collect();
        for (p, q, semitones) in [(17, 20, 6.0), (3, 2, 0.0)] {
            let ratio = Ratio::new(p, q).unwrap();
            let pitch = Pitch::from_semitones(semitones).unwrap();
            let output = Request::new(rate, 1, ratio)
                .with_pitch(pitch)
                .process(&input)
                .unwrap();
            for (note, chord) in chords.into_iter().enumerate() {
                let lo = ratio.output_frames(rate as usize * note).unwrap();
                let hi = ratio.output_frames(rate as usize * (note + 1)).unwrap();
                for hz in chord {
                    let expected = hz * pitch.factor();
                    let (actual, amplitude) = measured_partial(&output[lo..hi], rate, expected);
                    let cents = 1200.0 * (actual / expected).log2();
                    let db = 20.0 * (amplitude / 0.2).log10();
                    assert!(
                        cents.abs() <= 5.0,
                        "{rate} note {note} {p}/{q} {semitones}: {cents} cents"
                    );
                    assert!(
                        db.abs() <= 0.5,
                        "{rate} note {note} {p}/{q} {semitones}: {db} dB"
                    );
                }
            }
        }
    }
}

#[test]
fn bass_heavy_drums_keep_compressed_attack_timing_and_limit_pre_echo() {
    for rate in SAMPLE_RATES {
        let mut random = 1729u32;
        let input: Vec<_> = (0..rate)
            .map(|frame| {
                let time = f64::from(frame) / f64::from(rate);
                let mut value = 0.0;
                for onset in [0.2, 0.7] {
                    let relative = time - onset;
                    if (0.0..0.15).contains(&relative) {
                        let phase = TAU
                            * (55.0 * relative + 70.0 * 0.015 * (1.0 - (-relative / 0.015).exp()));
                        value += 0.7
                            * phase.sin()
                            * (relative / 0.0005).min(1.0)
                            * (-relative / 0.06).exp();
                    }
                }
                for onset in [0.325, 0.45, 0.575, 0.825] {
                    let relative = time - onset;
                    if (0.0..0.02).contains(&relative) {
                        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                        let noise = f64::from(random) / f64::from(u32::MAX) * 2.0 - 1.0;
                        value +=
                            0.12 * noise * (relative / 0.0005).min(1.0) * (-relative / 0.003).exp();
                    }
                }
                value as f32
            })
            .collect();
        {
            let (p, q) = (24, 25);
            let ratio = Ratio::new(p, q).unwrap();
            let output = Request::new(rate, 1, ratio).process(&input).unwrap();
            let at = ratio.output_frames(rate as usize / 5).unwrap();
            let power =
                |samples: &[f32]| samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
            let before = power(&output[at - rate as usize / 10..at]);
            let after = power(&output[at..at + rate as usize / 10]);
            let pre_echo = 10.0 * (before / after).log10();
            // Pairwise pre-echo regression calibrated at the 48 kHz project rate.
            // Other rates retain the independent onset-timing gate below.
            if rate == 48000 {
                assert!(pre_echo < -30.0, "{rate}: pre-echo {pre_echo} dB");
            }
            let onset = |samples: &[f32], center: usize| {
                let radius = rate as usize / 10;
                let lo = center - radius;
                let hi = center + radius;
                let block = rate as usize / 1000;
                let energy: Vec<_> = samples[lo..hi]
                    .windows(block)
                    .map(|window| {
                        window.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / block as f64
                    })
                    .collect();
                let peak = energy.iter().copied().fold(0.0, f64::max);
                lo + energy.iter().position(|e| *e >= peak * 0.1).unwrap()
            };
            let original_onset = onset(&input, rate as usize / 5);
            let output_onset = onset(&output, at);
            let expected = ratio.output_frames(original_onset).unwrap();
            let error_ms = output_onset.abs_diff(expected) as f64 * 1000.0 / f64::from(rate);
            assert!(
                error_ms <= 5.0,
                "{rate} {p}/{q}: onset {error_ms} ms, pre-echo {pre_echo} dB"
            );
            assert_eq!(output.len(), ratio.output_frames(input.len()).unwrap());
            if rate == 48000 {
                for factor in [0.0, -1.0] {
                    let stereo: Vec<_> = input.iter().flat_map(|v| [*v, *v * factor]).collect();
                    let linked = Request::new(rate, 2, ratio).process(&stereo).unwrap();
                    assert!(linked.chunks_exact(2).zip(&output).all(|(frame, mono)| {
                        (frame[0] - mono).abs() < 1e-6
                            && (frame[1] - frame[0] * factor).abs() < 1e-7
                    }));
                }
            }
        }
    }
}
