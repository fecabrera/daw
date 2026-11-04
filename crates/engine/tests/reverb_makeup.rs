//! Wet-output calibration uses generated, peak-normalized stereo fixtures.
//! Run the ignored report in release mode to inspect settings and rate coverage.
use daw_ambient_reverb::AmbientReverb;
use daw_plate_reverb::PlateReverb;
use daw_shimmer_reverb::ShimmerReverb;
use daw_unit_core::{
    AmbientReverbSettings, EffectUnit, PlateReverbSettings, ProcessingContext,
    ShimmerReverbSettings,
};

const SOURCE_RATE: u32 = 48000;
const SECONDS: u32 = 8;

fn fixture(kind: &str) -> Vec<[f32; 2]> {
    let mut seed = 72_u32;
    let mut noise = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from(seed >> 8) / 16777216.0 * 2.0 - 1.0
    };
    let mut rows = [[0.0; 12]; 2];
    let mut samples: Vec<[f32; 2]> = (0..SOURCE_RATE * SECONDS)
        .map(|i| {
            let t = f64::from(i) / f64::from(SOURCE_RATE);
            std::array::from_fn(|ch| {
                let n = noise();
                let phase = std::f64::consts::TAU;
                let x = match kind {
                    "pink" => {
                        let row = (i + 1).trailing_zeros() as usize;
                        if row < 12 {
                            rows[ch][row] = n;
                        }
                        (rows[ch].iter().sum::<f64>() + noise()) / 13.0
                    }
                    "music" => {
                        let root = [110.0, 130.8128, 146.8324, 98.0][(t / 2.0) as usize % 4];
                        let position = t % 0.5;
                        let envelope = (-position * 5.0).exp();
                        let chord: f64 = [1.0, 1.259921, 1.498307]
                            .iter()
                            .map(|r| {
                                (1..=5)
                                    .map(|h| {
                                        (phase * root * r * f64::from(h) * t
                                            + ch as f64 * 0.3 * f64::from(h))
                                        .sin()
                                            / f64::from(h)
                                    })
                                    .sum::<f64>()
                            })
                            .sum();
                        chord * envelope + n * (-position * 70.0).exp() * 0.5
                    }
                    "transients" => {
                        let position = t % 0.5;
                        (phase * 70.0 * position).sin() * (-position * 24.0).exp()
                            + n * (-position * 100.0).exp()
                    }
                    "tone" => (phase * 1000.0 * t).sin(),
                    _ => n,
                };
                x as f32
            })
        })
        .collect();
    let peak = samples
        .iter()
        .flatten()
        .fold(0.0_f32, |a, b| a.max(b.abs()));
    for x in samples.iter_mut().flatten() {
        *x *= 0.25 / peak;
    }
    samples
}

#[derive(Debug)]
struct Reading {
    relative_db: f64,
    peak: f32,
}

fn source_at(source: &[[f32; 2]], i: u32, rate: u32) -> [f32; 2] {
    // Identical physical source across rates; interpolate the 48 kHz fixture.
    let frame = f64::from(i) * f64::from(SOURCE_RATE) / f64::from(rate);
    let index = frame as usize;
    let next = (index + 1).min(source.len() - 1);
    std::array::from_fn(|ch| {
        source[index][ch] + (source[next][ch] - source[index][ch]) * frame.fract() as f32
    })
}

fn measure(unit: &mut dyn EffectUnit, source: &[[f32; 2]], rate: u32) -> Reading {
    let mut energy = [0.0_f64; 2];
    let mut peak = 0.0_f32;
    for i in 0..rate * SECONDS {
        let x = source_at(source, i, rate);
        let wet = unit.process_sample(x);
        assert!(wet.into_iter().all(f32::is_finite));
        peak = wet.into_iter().fold(peak, |a, b| a.max(b.abs()));
        if i >= rate * 4 {
            for (sum, samples) in energy.iter_mut().zip([x, wet]) {
                *sum += samples
                    .into_iter()
                    .map(|v| f64::from(v).powi(2))
                    .sum::<f64>();
            }
        }
    }
    Reading {
        relative_db: 10.0 * (energy[1] / energy[0]).log10(),
        peak,
    }
}

fn processors(rate: u32, variant: &str) -> [Box<dyn EffectUnit>; 3] {
    let context = ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    };
    let mut plate = PlateReverbSettings {
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut ambient = AmbientReverbSettings {
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut shimmer = ShimmerReverbSettings {
        dry_wet: 1.0,
        ..Default::default()
    };
    match variant {
        "HQ" => {
            plate.hq = true;
            ambient.hq = true;
        }
        "short" => {
            plate.decay_seconds = 0.3;
            ambient.decay_seconds = 1.0;
            shimmer.size = 0.0;
        }
        "long" => {
            plate.decay_seconds = 15.0;
            ambient.decay_seconds = 60.0;
            shimmer.size = 1.0;
        }
        "dark" => {
            plate.dark = true;
            ambient.tone = 0.0;
            shimmer.high_damp = 1.0;
            shimmer.warm = true;
        }
        "shimmer" => {
            shimmer.pitch_1 = 15;
            shimmer.pitch_2 = -15;
            shimmer.amount_1 = 1.0;
            shimmer.amount_2 = 1.0;
        }
        "gated" => {
            plate.gated = true;
        }
        "mix" => {
            plate.dry_wet = 0.5;
            ambient.dry_wet = 0.5;
            shimmer.dry_wet = 0.5;
        }
        _ => {}
    }
    [
        Box::new(PlateReverb::new(context, &plate)),
        Box::new(AmbientReverb::new(context, &ambient)),
        Box::new(ShimmerReverb::new(context, &shimmer)),
    ]
}

#[test]
#[ignore = "release-mode calibration report; run with --ignored --nocapture"]
fn calibration_report() {
    for signal in ["white", "pink", "music", "transients", "tone"] {
        let source = fixture(signal);
        for variant in ["default", "HQ", "short", "long", "dark", "shimmer", "gated"] {
            for (name, mut unit) in ["Plate", "Ambient", "Shimmer"]
                .into_iter()
                .zip(processors(48000, variant))
            {
                let reading = measure(unit.as_mut(), &source, 48000);
                println!(
                    "{signal:10} {variant:8} {name:8} {:6.2} dB peak {:.3}",
                    reading.relative_db, reading.peak
                );
            }
        }
    }
}

#[test]
fn calibrated_wet_has_useful_level_and_headroom_at_every_project_rate() {
    for signal in ["pink", "music", "transients", "tone"] {
        let source = fixture(signal);
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for variant in ["default", "HQ"] {
                for (name, mut unit) in ["Plate", "Ambient", "Shimmer"]
                    .into_iter()
                    .zip(processors(rate, variant))
                {
                    let r = measure(unit.as_mut(), &source, rate);
                    println!("{signal} {rate} {variant} {name}: {r:?}");
                    // Tonal notches and transient tail energy are part of the tank's
                    // response. Calibrate broadband balance; guard tonal peak headroom.
                    if matches!(signal, "pink" | "music") {
                        assert!(
                            (-12.0..=3.0).contains(&r.relative_db),
                            "{signal} {rate} {name}: {r:?}"
                        );
                    }
                    assert!(
                        r.peak < 1.0,
                        "-12 dBFS fixture clipped: {signal} {rate} {name}: {r:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn settings_extremes_retain_headroom_without_level_tracking() {
    for signal in ["pink", "music", "transients", "tone"] {
        let source = fixture(signal);
        for variant in ["HQ", "short", "long", "dark", "shimmer", "gated", "mix"] {
            for (name, mut unit) in ["Plate", "Ambient", "Shimmer"]
                .into_iter()
                .zip(processors(48000, variant))
            {
                let r = measure(unit.as_mut(), &source, 48000);
                assert!(
                    r.peak < 1.0,
                    "-12 dBFS fixture clipped: {signal} {variant} {name}: {r:?}"
                );
            }
        }
    }
}

#[test]
fn loaded_tails_keep_headroom_through_decay_quality_and_gate_changes() {
    for signal in ["music", "tone"] {
        let source = fixture(signal);
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let mut p = PlateReverbSettings {
                decay_seconds: 15.0,
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut a = AmbientReverbSettings {
                decay_seconds: 60.0,
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut s = ShimmerReverbSettings {
                size: 1.0,
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut plate = PlateReverb::new(context, &p);
            let mut ambient = AmbientReverb::new(context, &a);
            let mut shimmer = ShimmerReverb::new(context, &s);
            let mut peaks = [0.0_f32; 3];
            for i in 0..rate * SECONDS {
                let x = source_at(&source, i, rate);
                if i > 0 && i % (rate * 2) == 0 {
                    let stage = i / (rate * 2);
                    p.decay_seconds = if stage == 1 { 0.3 } else { 3.0 };
                    p.hq = stage == 2;
                    p.gated = stage == 3;
                    a.decay_seconds = if stage == 1 { 1.0 } else { 7.5 };
                    a.hq = stage == 2;
                    s.size = if stage == 1 { 0.0 } else { 0.75 };
                    s.pitch_1 = 15;
                    s.amount_1 = 1.0;
                    plate.retarget(&p);
                    ambient.retarget(&a);
                    shimmer.retarget(&s);
                }
                for (peak, y) in peaks.iter_mut().zip([
                    plate.process_sample(x),
                    ambient.process_sample(x),
                    shimmer.process_sample(x),
                ]) {
                    for sample in y {
                        assert!(sample.is_finite());
                        *peak = peak.max(sample.abs());
                    }
                }
            }
            println!("loaded-tail transition {signal} {rate}: {peaks:?}");
            assert!(
                peaks.into_iter().all(|p| p < 1.0),
                "{signal} {rate}: {peaks:?}"
            );
        }
    }
}
