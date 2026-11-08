//! Reproducible synthetic detector comparison. No audio files or reference engines.
use std::{f64::consts::TAU, hint::black_box, ops::ControlFlow, time::Instant};
use timestretch::{Ratio, Request, TransientDetector as Mode, detect_transients};
fn fixture(name: &str, rate: u32) -> (Vec<f32>, Vec<usize>) {
    let events: Vec<_> = if matches!(name, "dyad" | "vibrato" | "silence") {
        vec![]
    } else if name == "dense" {
        (0..12)
            .map(|i| rate as usize * (100 + i * 35) / 1000)
            .collect()
    } else {
        [0.2, 0.5, 0.8]
            .map(|t| (t * f64::from(rate)) as usize)
            .to_vec()
    };
    let mut seed = 0x7ac3_9d21_u32;
    let samples = (0..rate as usize)
        .map(|i| {
            let t = i as f64 / f64::from(rate);
            let bass = match name {
                "dyad" => 0.3 * (TAU * 51.9130872 * t).sin() + 0.3 * (TAU * 65.40639133 * t).sin(),
                "vibrato" => 0.4 * (TAU * 440.0 * t + 0.7 * (TAU * 5.0 * t).sin()).sin(),
                "masked" => 0.7 * (TAU * 65.40639133 * t).sin(),
                _ => 0.0,
            };
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0;
            let mut value = bass;
            for &at in &events {
                if i >= at && i - at < rate as usize / 80 {
                    let age = (i - at) as f64 / f64::from(rate);
                    value += if name == "clicks" {
                        if i - at < 3 { 0.5 } else { 0.0 }
                    } else {
                        noise * (-age * 300.0).exp() * if name == "masked" { 0.025 } else { 0.5 }
                    };
                }
            }
            value as f32
        })
        .collect();
    (samples, events)
}
fn score(found: &[usize], truth: &[usize], rate: u32) -> (usize, usize, usize, f64) {
    // Startup is a known signal boundary, not an annotated interior onset.
    let mut used = vec![false; truth.len()];
    let mut tp = 0;
    let mut fp = 0;
    let mut error = 0.0_f64;
    for &at in found.iter().filter(|&&at| at > rate as usize / 20) {
        let nearest = truth
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .min_by_key(|(_, p)| at.abs_diff(**p));
        if let Some((i, &p)) = nearest
            && at.abs_diff(p) <= rate as usize / 100
        {
            used[i] = true;
            tp += 1;
            error = error.max(at.abs_diff(p) as f64 * 1000.0 / f64::from(rate));
        } else {
            fp += 1;
        }
    }
    (tp, fp, truth.len() - tp, error)
}
fn main() {
    println!(
        "case,rate,channels,scale,detector,fixture_fnv64,tp,fp,fn,max_error_ms,median_ms,min_ms,max_ms"
    );
    for rate in timestretch::SAMPLE_RATES {
        for name in [
            "silence", "dyad", "vibrato", "clicks", "bursts", "dense", "masked",
        ] {
            let (mono, truth) = fixture(name, rate);
            for (channels, scale) in [(1, 1.0_f32), (2, 0.01), (2, 2.0)] {
                let input: Vec<_> = mono
                    .iter()
                    .flat_map(|&s| {
                        if channels == 1 {
                            vec![s * scale]
                        } else {
                            vec![s * scale, -s * scale]
                        }
                    })
                    .collect();
                let hash = input
                    .iter()
                    .flat_map(|s| s.to_bits().to_le_bytes())
                    .fold(0xcbf29ce484222325_u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                    });
                let request = Request::new(rate, channels, Ratio::new(3, 2).unwrap());
                let mut times = [vec![], vec![]];
                let mut result = [vec![], vec![]];
                for pass in 0..6 {
                    for index in if pass % 2 == 0 { [0, 1] } else { [1, 0] } {
                        let mode = if index == 0 {
                            Mode::Energy
                        } else {
                            Mode::Combined
                        };
                        let start = Instant::now();
                        let found = detect_transients(request, black_box(&input), mode, |_| {
                            ControlFlow::Continue(())
                        })
                        .unwrap();
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        if pass > 0 {
                            times[index].push(ms);
                        }
                        result[index] = found;
                    }
                }
                for (index, mode) in [Mode::Energy, Mode::Combined].into_iter().enumerate() {
                    times[index].sort_by(f64::total_cmp);
                    let (tp, fp, miss, error) = score(&result[index], &truth, rate);
                    println!(
                        "{name},{rate},{channels},{scale},{},{hash:016x},{tp},{fp},{miss},{error:.4},{:.6},{:.6},{:.6}",
                        mode.revision(),
                        times[index][2],
                        times[index][0],
                        times[index][4]
                    );
                }
            }
        }
    }
    eprintln!(
        "render_case,ratio,detector,median_ms,min_ms,max_ms,max_onset_error_ms,frames,output_fnv64"
    );
    for name in ["bursts", "masked", "dyad"] {
        let rate = 48000;
        let (input, truth) = fixture(name, rate);
        for ratio in [
            Ratio::new(3, 4).unwrap(),
            Ratio::new(3, 2).unwrap(),
            Ratio::new(2, 1).unwrap(),
        ] {
            for mode in [Mode::Energy, Mode::Combined] {
                let request = Request::new(rate, 1, ratio);
                let mut times = Vec::new();
                let mut output = Vec::new();
                for pass in 0..6 {
                    let start = Instant::now();
                    output =
                        request
                            .process_with_transient_detector(&input, mode, |_| {
                                ControlFlow::Continue(())
                            })
                            .unwrap();
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    if pass > 0 {
                        times.push(ms);
                    }
                }
                times.sort_by(f64::total_cmp);
                let hash = output
                    .iter()
                    .flat_map(|s| s.to_bits().to_le_bytes())
                    .fold(0xcbf29ce484222325_u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                    });
                let error = truth
                    .iter()
                    .map(|&at| {
                        let expected = ratio.output_frames(at).unwrap();
                        let width = rate as usize / 1000;
                        let lo = expected.saturating_sub(rate as usize / 50);
                        let hi = (expected + rate as usize / 50).min(output.len() - width);
                        let found = (lo..hi)
                            .step_by(width)
                            .max_by(|&a, &b| {
                                let energy = |p| {
                                    output[p..p + width]
                                        .windows(2)
                                        .map(|s| f64::from(s[1] - s[0]).powi(2))
                                        .sum::<f64>()
                                };
                                energy(a).total_cmp(&energy(b))
                            })
                            .unwrap();
                        found.abs_diff(expected) as f64 * 1000.0 / f64::from(rate)
                    })
                    .fold(0.0_f64, f64::max);
                eprintln!(
                    "{name},{ratio},{},{:.6},{:.6},{:.6},{error:.4},{},{hash:016x}",
                    mode.revision(),
                    times[2],
                    times[0],
                    times[4],
                    output.len()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_finds_masked_attacks_without_tonal_false_positives_across_rates_and_layouts() {
        for rate in timestretch::SAMPLE_RATES {
            for name in [
                "silence", "dyad", "vibrato", "clicks", "bursts", "dense", "masked",
            ] {
                let (mono, truth) = fixture(name, rate);
                let mut reference = None;
                for (channels, scale) in [(1, 1.0_f32), (2, 0.01), (2, 2.0)] {
                    let input: Vec<_> = mono
                        .iter()
                        .flat_map(|&s| {
                            if channels == 1 {
                                vec![s * scale]
                            } else {
                                vec![s * scale, -s * scale]
                            }
                        })
                        .collect();
                    let request = Request::new(rate, channels, Ratio::new(3, 2).unwrap());
                    let found = detect_transients(request, &input, Mode::Combined, |_| {
                        ControlFlow::Continue(())
                    })
                    .unwrap();
                    let (tp, fp, miss, _) = score(&found, &truth, rate);
                    assert_eq!(
                        (tp, fp, miss),
                        (truth.len(), 0, 0),
                        "{name} {rate} {channels} {scale}: {found:?}"
                    );
                    if let Some(previous) = &reference {
                        assert_eq!(&found, previous, "gain/polarity invariance {name} {rate}");
                    }
                    reference = Some(found);
                }
            }
        }
    }
}
