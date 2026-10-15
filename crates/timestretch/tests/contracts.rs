use std::ops::ControlFlow;
use timestretch::{Error, Ratio, Request, SAMPLE_RATES};
fn tone(rate: u32, frequency: f64, seconds: f64, channels: usize, factor: f32) -> Vec<f32> {
    (0..(rate as f64 * seconds) as usize)
        .flat_map(|i| {
            let x = (i as f64 * frequency * std::f64::consts::TAU / rate as f64).sin() as f32 * 0.4;
            (0..channels).map(move |c| if c == 0 { x } else { x * factor })
        })
        .collect()
}
fn frequency(signal: &[f32], rate: u32) -> f64 {
    let crossings: Vec<_> = signal
        .windows(2)
        .enumerate()
        .filter(|(_, s)| s[0] <= 0.0 && s[1] > 0.0)
        .map(|(i, s)| i as f64 - s[0] as f64 / (s[1] - s[0]) as f64)
        .collect();
    (crossings.len() - 1) as f64 * rate as f64 / (crossings.last().unwrap() - crossings[0])
}
#[test]
fn validation_rounding_identity_and_resources() {
    for invalid in ["0", "-1", "NaN", "9", "1/9", "1/0", "0.0", "1e0", "1/2/3"] {
        assert!(invalid.parse::<Ratio>().is_err());
    }
    assert_eq!("1.50".parse::<Ratio>().unwrap(), Ratio::new(3, 2).unwrap());
    assert_eq!(Ratio::new(1, 2).unwrap().output_frames(1), Ok(1));
    assert_eq!(
        Ratio::new(1, 8).unwrap().output_frames(1),
        Err(Error::OutputTooShort)
    );
    assert_eq!(
        Ratio::new(8, 1).unwrap().output_frames(usize::MAX),
        Err(Error::ResourceLimit)
    );
    for rate in SAMPLE_RATES {
        for channels in [1, 2] {
            let request = Request::new(rate, channels, Ratio::new(1, 1).unwrap());
            let data = [0.0, -0.0, 1.25, -2.0];
            assert_eq!(
                request
                    .process(&data)
                    .unwrap()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                data.map(f32::to_bits)
            );
            assert_eq!(request.process(&[f32::NAN; 2]), Err(Error::NonfiniteInput));
            assert_eq!(
                request.process_with_progress(&data, |_| ControlFlow::Break(())),
                Err(Error::Cancelled)
            );
            assert_eq!(
                Request {
                    memory_limit_bytes: 1,
                    ..request
                }
                .process(&data),
                Err(Error::ResourceLimit)
            );
        }
    }
    assert_eq!(
        Request::new(0, 1, Ratio::new(1, 1).unwrap()).process(&[]),
        Err(Error::InvalidRate)
    );
    assert_eq!(
        Request::new(48000, 2, Ratio::new(1, 1).unwrap()).process(&[0.0]),
        Err(Error::InvalidLayout)
    );
}
#[test]
fn short_length_silence_and_signal_presence() {
    for frames in [
        0, 1, 2, 7, 31, 255, 256, 257, 1023, 1024, 1025, 4095, 4096, 4097,
    ] {
        for (p, q) in [(1, 8), (1, 2), (1, 1), (3, 2), (8, 1)] {
            let ratio = Ratio::new(p, q).unwrap();
            let request = Request::new(48000, 1, ratio);
            for level in [0.0, 0.5] {
                let output = request.process(&vec![level; frames]);
                match ratio.output_frames(frames) {
                    Err(e) => assert_eq!(output, Err(e)),
                    Ok(n) => {
                        let output = output.unwrap();
                        assert_eq!(output.len(), n);
                        assert!(
                            output
                                .iter()
                                .all(|s| s.is_finite() && (*s - level).abs() < 1e-5)
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn pitch_level_and_linked_stereo() {
    for (p, q) in [(1, 8), (1, 2), (2, 3), (3, 2), (2, 1), (8, 1)] {
        for freq in [55., 440., 1000., 5000.] {
            let input = tone(48000, freq, 3.0, 2, -1.0);
            let request = Request::new(48000, 2, Ratio::new(p, q).unwrap());
            let output = request.process(&input).unwrap();
            assert!(output.chunks_exact(2).all(|s| (s[0] + s[1]).abs() < 1e-7));
            let left: Vec<_> = output.chunks_exact(2).map(|s| s[0]).collect();
            let middle = &left[left.len() / 4..left.len() * 3 / 4];
            let cents = 1200.0 * (frequency(middle, 48000) / freq).log2();
            let db = 20.0
                * ((middle.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
                    / middle.len() as f64)
                    .sqrt()
                    / (0.4 / 2f64.sqrt()))
                .log10();
            assert!(
                cents.abs()
                    <= if (1..=2).contains(&p) && q <= 2 {
                        5.0
                    } else {
                        15.0
                    },
                "{p}/{q} {freq}: {cents} cents"
            );
            assert!(
                db.abs()
                    <= if p as f64 / q as f64 >= 0.5 && p as f64 / q as f64 <= 2.0 {
                        0.5
                    } else {
                        1.5
                    },
                "{p}/{q} {freq}: {db} dB"
            );
        }
    }
}
#[test]
fn mono_equivalence_swap_silent_channel_determinism_cancel() {
    let mono = tone(48000, 440., 0.1, 1, 1.0);
    let r = Ratio::new(3, 2).unwrap();
    let reference = Request::new(48000, 1, r).process(&mono).unwrap();
    for a in [0., 0.5, 1., -1.] {
        let stereo: Vec<_> = mono.iter().flat_map(|s| [*s, *s * a]).collect();
        let request = Request::new(48000, 2, r);
        let output = request.process(&stereo).unwrap();
        for (i, s) in output.chunks_exact(2).enumerate() {
            assert!((s[0] - reference[i]).abs() < 1e-6);
            assert!((s[1] - s[0] * a).abs() < 1e-7);
        }
        assert_eq!(output, request.process(&stereo).unwrap());
        let swapped: Vec<_> = stereo.chunks_exact(2).flat_map(|s| [s[1], s[0]]).collect();
        let swapped = request.process(&swapped).unwrap();
        for (a, b) in output.chunks_exact(2).zip(swapped.chunks_exact(2)) {
            assert_eq!(a, [b[1], b[0]]);
        }
        let mut last = 0.0;
        assert_eq!(
            request.process_with_progress(&stereo, |p| {
                assert!(p >= last);
                last = p;
                if p > 0.5 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
            Err(Error::Cancelled)
        );
    }
}

/// Expanded acceptance matrix. Run with --release -- --ignored to keep normal CI bounded.
#[test]
#[ignore = "full six-rate tone matrix; run in release acceptance validation"]
fn acceptance_pitch_level_all_rates_and_ratios() {
    for rate in SAMPLE_RATES {
        for (p, q) in [
            (1, 8),
            (1, 4),
            (1, 2),
            (2, 3),
            (9, 10),
            (1, 1),
            (10, 9),
            (3, 2),
            (2, 1),
            (4, 1),
            (8, 1),
        ] {
            for hz in [55., 110., 440., 1000., 5000.] {
                let ratio = Ratio::new(p, q).unwrap();
                let seconds = (80.0 / hz / ratio.as_f64().min(1.0)).max(0.5);
                let input = tone(rate, hz, seconds, 1, 1.0);
                let output = Request::new(rate, 1, ratio).process(&input).unwrap();
                let middle = &output[output.len() / 4..output.len() * 3 / 4];
                let cents = 1200.0 * (frequency(middle, rate) / hz).log2();
                let db = 20.0
                    * ((middle.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
                        / middle.len() as f64)
                        .sqrt()
                        / (0.4 / 2f64.sqrt()))
                    .log10();
                let ordinary = (0.5..=2.).contains(&ratio.as_f64());
                assert!(
                    cents.abs() <= if ordinary { 5. } else { 15. },
                    "{rate} {p}/{q} {hz}: {cents} cents"
                );
                assert!(
                    db.abs() <= if ordinary { 0.5 } else { 1.5 },
                    "{rate} {p}/{q} {hz}: {db} dB"
                );
            }
        }
    }
}

#[test]
fn broadband_transient_timing_and_width() {
    let rate = 48000;
    let mut signal = vec![0.0; rate as usize];
    let mut random = 37u32;
    for onset in [9600, 24000, 38400] {
        for i in 0..480 {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let envelope = if i < 96 {
                0.5 - 0.5 * (std::f64::consts::PI * i as f64 / 96.).cos()
            } else {
                0.5 + 0.5 * (std::f64::consts::PI * (i - 96) as f64 / 384.).cos()
            };
            signal[onset + i] =
                (f64::from(random) / f64::from(u32::MAX) * 2. - 1.) as f32 * envelope as f32 * 0.7;
        }
    }
    // Independent metric: forward 1 ms mean-square envelope, first 10% peak crossing.
    // Cumulative energy 10..90% width, ties resolve to the earliest sample.
    let measure = |data: &[f32], center: usize, radius: usize| {
        let start = center.saturating_sub(radius);
        let end = (center + radius).min(data.len());
        let energy: Vec<_> = data[start..end]
            .iter()
            .map(|s| f64::from(*s).powi(2))
            .collect();
        let envelope: Vec<_> = energy
            .windows(48)
            .map(|v| v.iter().sum::<f64>() / 48.)
            .collect();
        let peak = envelope.iter().copied().fold(0., f64::max);
        assert!(peak > 1e-12, "lost event");
        let onset = envelope.iter().position(|e| *e >= peak * 0.1).unwrap() + start;
        let sum: f64 = energy.iter().sum();
        let mut acc = 0.;
        let (mut lo, mut hi) = (None, None);
        for (i, e) in energy.iter().enumerate() {
            acc += e;
            if lo.is_none() && acc >= sum * 0.1 {
                lo = Some(i)
            }
            if hi.is_none() && acc >= sum * 0.9 {
                hi = Some(i)
            }
        }
        (onset, (hi.unwrap() - lo.unwrap()) as f64)
    };
    for (p, q) in [(1, 8), (1, 2), (3, 2), (2, 1), (8, 1)] {
        let ratio = Ratio::new(p, q).unwrap();
        let r = ratio.as_f64();
        let output = Request::new(rate, 1, ratio).process(&signal).unwrap();
        for center in [9600, 24000, 38400] {
            let (source_onset, width) = measure(&signal, center, 3000);
            let mapped = (center as f64 * r).round() as usize;
            let (onset, out_width) = measure(&output, mapped, (3000. * r.min(1.)) as usize);
            let ordinary = (0.5..=2.).contains(&r);
            let error = (onset as f64 - source_onset as f64 * r).abs() / 48.;
            assert!(
                error <= if ordinary { 5. } else { 15. },
                "{p}/{q}: {error} ms"
            );
            assert!(
                out_width <= width.max(width * r) * if ordinary { 2. } else { 4. },
                "{p}/{q}: {out_width} vs {width}"
            );
        }
    }
}

#[test]
fn boundary_events_keep_energy_and_short_bursts_keep_pitch() {
    for (p, q) in [(1, 8), (1, 2), (3, 2), (8, 1)] {
        let ratio = Ratio::new(p, q).unwrap();
        let mut energies = Vec::new();
        for position in [0, 480, 24000, 47519, 47999] {
            let mut signal = vec![0.0; 48000];
            signal[position] = 0.5;
            let output = Request::new(48000, 1, ratio).process(&signal).unwrap();
            let energy: f64 = output.iter().map(|s| f64::from(*s).powi(2)).sum();
            assert!(energy > 1e-12, "lost boundary event {position} {p}/{q}");
            let weighted: f64 = output
                .iter()
                .enumerate()
                .map(|(i, s)| i as f64 * f64::from(*s).powi(2))
                .sum();
            let timing = (weighted / energy - position as f64 * ratio.as_f64()).abs() / 48.;
            assert!(timing <= 15., "boundary {position} {p}/{q}: {timing} ms");
            energies.push(energy);
        }
        for energy in &energies {
            assert!(
                *energy >= energies[2] / 10. && *energy <= energies[2] * 10.,
                "boundary energies {p}/{q}: {energies:?}"
            );
        }
    }
    // Below the 2048-sample tonal window, but with enough periods for a fit.
    for ratio in [Ratio::new(1, 8).unwrap(), Ratio::new(8, 1).unwrap()] {
        let input = tone(48000, 5000., 0.03, 1, 1.);
        let output = Request::new(48000, 1, ratio).process(&input).unwrap();
        let middle = &output[output.len() / 4..output.len() * 3 / 4];
        let cents = 1200. * (frequency(middle, 48000) / 5000.).log2();
        assert!(cents.abs() <= 15., "short burst {ratio}: {cents} cents");
    }
}

#[test]
fn stereo_phase_and_amplitude_homogeneity() {
    for (p, q) in [(1, 8), (1, 2), (3, 2), (8, 1)] {
        for hz in [440., 1000.] {
            let mono = tone(48000, hz, 1.0, 1, 1.);
            let phase = 0.7;
            let stereo: Vec<_> = mono
                .iter()
                .enumerate()
                .flat_map(|(i, l)| {
                    [
                        *l,
                        ((i as f64 * hz * std::f64::consts::TAU / 48000. + phase).sin() * 0.4)
                            as f32,
                    ]
                })
                .collect();
            let request = Request::new(48000, 2, Ratio::new(p, q).unwrap());
            let output = request.process(&stereo).unwrap();
            let amplified: Vec<_> = stereo.iter().map(|v| v * 2.).collect();
            let amplified = request.process(&amplified).unwrap();
            let residual = output
                .iter()
                .zip(&amplified)
                .map(|(a, b)| (f64::from(*b) - 2. * f64::from(*a)).powi(2))
                .sum::<f64>();
            let power = amplified.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
            assert!(residual <= power * 1e-6, "amplitude response changed");
            let frames = output.len() / 2;
            let mut phases = [0.; 2];
            for (c, phase) in phases.iter_mut().enumerate() {
                let (mut sine, mut cosine) = (0., 0.);
                for i in frames / 4..frames * 3 / 4 {
                    let angle = i as f64 * hz * std::f64::consts::TAU / 48000.;
                    sine += f64::from(output[2 * i + c]) * angle.sin();
                    cosine += f64::from(output[2 * i + c]) * angle.cos();
                }
                *phase = cosine.atan2(sine);
            }
            let error = (phases[1] - phases[0] - phase + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            assert!(
                error.abs().to_degrees()
                    <= if (0.5..=2.).contains(&request.ratio.as_f64()) {
                        5.
                    } else {
                        15.
                    }
            );
        }
    }
}

#[test]
#[ignore = "full short-input matrix; run in release acceptance validation"]
fn acceptance_short_layout_matrix() {
    for rate in SAMPLE_RATES {
        for (p, q) in [
            (1, 8),
            (1, 4),
            (1, 2),
            (2, 3),
            (9, 10),
            (1, 1),
            (10, 9),
            (3, 2),
            (2, 1),
            (4, 1),
            (8, 1),
        ] {
            for frames in [
                0, 1, 2, 7, 31, 255, 256, 257, 1023, 1024, 1025, 4095, 4096, 4097,
            ] {
                for channels in [1, 2] {
                    let ratio = Ratio::new(p, q).unwrap();
                    // Deterministic nonconstant input with unclipped headroom and channel asymmetry.
                    let data: Vec<_> = (0..frames)
                        .flat_map(|i| {
                            (0..channels).map(move |c| ((i * 17 + c * 7) % 31) as f32 / 6. - 2.)
                        })
                        .collect();
                    let output = Request::new(rate, channels, ratio).process(&data);
                    match ratio.output_frames(frames) {
                        Ok(n) => {
                            let output = output.unwrap();
                            assert_eq!(output.len(), n * channels);
                            assert!(output.iter().all(|s| s.is_finite()));
                        }
                        Err(error) => assert_eq!(output, Err(error)),
                    }
                }
            }
        }
    }
}
