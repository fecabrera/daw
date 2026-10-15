use std::{f64::consts::TAU, ops::ControlFlow};
use timestretch::{Error, Ratio, Request, SAMPLE_RATES};

fn power(samples: &[f32]) -> f64 {
    samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64
}
fn noise(frames: usize) -> Vec<f32> {
    let mut random = 37u32;
    (0..frames)
        .map(|_| {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            (f64::from(random) / f64::from(u32::MAX) - 0.5) as f32
        })
        .collect()
}

#[test]
fn moving_spectral_peaks_preserve_energy_without_gain_matching() {
    let input = noise(48000);
    for (p, q) in [(1, 2), (3, 2), (2, 1)] {
        let request = Request::new(48000, 1, Ratio::new(p, q).unwrap());
        let new = request.process(&input).unwrap();
        let new_db = 10. * (power(&new) / power(&input)).log10();
        assert!(new_db.abs() < if p < q { 2.3 } else { 0.9 }, "{new_db}");
        // Shared phase decisions, not a nonlinear level normalizer or peak clamp.
        let doubled: Vec<_> = input.iter().map(|s| s * 2.).collect();
        let doubled = request.process(&doubled).unwrap();
        let residual: Vec<_> = doubled.iter().zip(&new).map(|(a, b)| a - 2. * b).collect();
        assert!(power(&residual) < power(&doubled) * 1e-8);
    }
}

fn percussion(rate: u32, noisy: bool, background: bool) -> Vec<f32> {
    let mut signal = noise(rate as usize);
    for (i, sample) in signal.iter_mut().enumerate() {
        let time = i as f64 / f64::from(rate);
        let relative = time - if time < 0.5 { 0.2 } else { 0.7 };
        let pulse = if (0.0..0.15).contains(&relative) {
            let envelope =
                (relative / 0.0005).min(1.) * (-relative / if noisy { 0.02 } else { 0.06 }).exp();
            if noisy {
                f64::from(*sample) * envelope
            } else {
                0.7 * (TAU * (55. * relative + 70. * 0.015 * (1. - (-relative / 0.015).exp())))
                    .sin()
                    * envelope
            }
        } else {
            0.
        };
        *sample = (pulse
            + if background {
                0.1 * (TAU * 220. * time).sin()
            } else {
                0.
            }) as f32;
    }
    signal
}

#[test]
fn expansion_retains_attacks_on_kicks_noise_and_mixed_material() {
    for rate in [48000, 96000] {
        for (noisy, background) in [(false, false), (true, false), (false, true)] {
            let input = percussion(rate, noisy, background);
            for (p, q) in [(3, 2), (2, 1), (8, 1)] {
                let request = Request::new(rate, 1, Ratio::new(p, q).unwrap());
                let new = request.process(&input).unwrap();
                let onset = rate as usize / 5;
                let target = onset * p as usize / q as usize;
                let length = rate as usize / if noisy { 2000 } else { 100 };
                let reference = &input[onset..onset + length];
                let residual = |signal: &[f32]| -> f64 {
                    signal[target..target + length]
                        .iter()
                        .zip(reference)
                        .map(|(a, b)| f64::from(*a - *b).powi(2))
                        .sum::<f64>()
                        / reference.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
                };
                assert!(
                    residual(&new) < 0.02,
                    "attack {rate} {noisy} {background} {p}/{q}: {}",
                    residual(&new)
                );
                if !background {
                    let pre = power(&new[target - length..target]);
                    assert!(pre < power(reference) * 0.002, "pre-echo: {pre}");
                }
            }
        }
    }
}

#[test]
fn transient_processing_preserves_stereo_gain_and_cancellation() {
    let mono = percussion(48000, false, true);
    let ratio = Ratio::new(3, 2).unwrap();
    let reference = Request::new(48000, 1, ratio).process(&mono).unwrap();
    for factor in [0., 0.5, -1.] {
        let input: Vec<_> = mono.iter().flat_map(|s| [*s, *s * factor]).collect();
        let request = Request::new(48000, 2, ratio);
        let out = request.process(&input).unwrap();
        for (frame, s) in out.chunks_exact(2).enumerate() {
            assert!((s[0] - reference[frame]).abs() < 1e-6);
            assert!((s[1] - s[0] * factor).abs() < 1e-7);
        }
        for threshold in [0.08, 0.12, 0.5, 0.95] {
            let mut last = 0.;
            assert_eq!(
                request.process_with_progress(&input, |p| {
                    assert!(p >= last);
                    last = p;
                    if p > threshold {
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
fn dense_off_grid_and_boundary_attacks_stay_finite_and_ordered() {
    for rate in SAMPLE_RATES {
        let mut input = vec![0.; rate as usize / 4];
        for i in [
            0,
            1,
            rate as usize / 30 + 7,
            rate as usize / 12 + 13,
            rate as usize / 4 - 1,
        ] {
            input[i] = 0.5;
        }
        for (p, q) in [(10, 9), (3, 2), (8, 1)] {
            let ratio = Ratio::new(p, q).unwrap();
            let out = Request::new(rate, 1, ratio).process(&input).unwrap();
            assert_eq!(out.len(), ratio.output_frames(input.len()).unwrap());
            assert!(out.iter().all(|s| s.is_finite()));
            // Each separated impulse must survive near its mapped position.
            for index in [
                0,
                rate as usize / 30 + 7,
                rate as usize / 12 + 13,
                rate as usize / 4 - 1,
            ] {
                let at = (index as f64 * ratio.as_f64()).round() as usize;
                let radius = rate as usize * 15 / 1000;
                let lo = at.saturating_sub(radius);
                let hi = (at + radius).min(out.len());
                assert!(
                    out[lo..hi].iter().any(|s| s.abs() > 0.01),
                    "lost event {rate} {p}/{q} {index}"
                );
            }
        }
    }
}

#[test]
fn short_band_limited_noise_keeps_its_decay_energy() {
    // A high cymbal-like band with empty low frequencies must not be mistaken
    // for a low-frequency tone just because full-spectrum flatness is small.
    let source = noise(4800 + 128);
    let taps: Vec<_> = (0..129)
        .map(|i| {
            let n = i as f64 - 64.;
            let lowpass = if n == 0. {
                0.125
            } else {
                (TAU * 3000. * n / 48000.).sin() / (std::f64::consts::PI * n)
            };
            lowpass * (TAU * 8000. * n / 48000.).cos() * (0.5 - 0.5 * (TAU * i as f64 / 128.).cos())
        })
        .collect();
    let input: Vec<_> = (0..4800)
        .map(|i| {
            let filtered: f64 = taps
                .iter()
                .enumerate()
                .map(|(j, t)| t * f64::from(source[i + j]))
                .sum();
            (filtered * (i as f64 / 24.).min(1.) * (-(i as f64) / 720.).exp()) as f32
        })
        .collect();
    for (p, q) in [(3, 2), (2, 1), (8, 1)] {
        let out = Request::new(48000, 1, Ratio::new(p, q).unwrap())
            .process(&input)
            .unwrap();
        let db = 10. * (power(&out) / power(&input)).log10();
        assert!(db.abs() < 2.5, "lost noise decay {p}/{q}: {db} dB");
    }
}
