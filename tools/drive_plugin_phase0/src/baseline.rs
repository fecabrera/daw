//! Direct Drive baseline and bounded impulse-tail survey, without a DAW or device.
use daw_drive::Drive;
use daw_unit_core::{DriveColorShape, DriveSettings, EffectUnit, ProcessingContext};
use std::{hint::black_box, time::Instant};

const RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];
const THRESHOLD: f32 = 1e-6; // -120 dBFS absolute per-channel amplitude.

fn main() {
    let mut tails = Vec::new();
    for rate in RATES {
        let mut worst = 0_u32;
        let mut cases = 0;
        for shape in DriveColorShape::ALL {
            for frequency in [250.0, 750.0, 2000.0] {
                for tone in [0.0, 0.5, 1.0] {
                    for gain in [0.0, 0.2, 1.0] {
                        let settings = DriveSettings {
                            color_shape: shape,
                            color_frequency_hz: frequency,
                            color_strength: 1.0,
                            gain,
                            tone,
                            volume_db: 12.0,
                            ..Default::default()
                        };
                        let mut processor = Drive::new(
                            ProcessingContext {
                                sample_rate_hz: rate,
                                max_block_frames: 512,
                            },
                            &settings,
                        );
                        processor.process_sample([4.0, -2.0]);
                        let mut last = 0;
                        // Survey two seconds. The declared policy must include a safety margin.
                        for frame in 1..=rate * 2 {
                            let out = processor.process_sample([0.0; 2]);
                            assert!(out.iter().all(|x| x.is_finite()), "nonfinite tail");
                            if out.iter().any(|x| x.abs() > THRESHOLD) {
                                last = frame;
                            }
                        }
                        worst = worst.max(last);
                        cases += 1;
                    }
                }
            }
        }
        assert!(worst < rate, "tail does not fit proposed one-second policy");
        tails.push(serde_json::json!({"rate": rate, "cases": cases, "last_above_threshold_frame": worst, "last_above_threshold_ms": 1000.0 * f64::from(worst) / f64::from(rate)}));
    }
    let mut timing = Vec::new();
    for frames in [32, 512] {
        for instances in [1, 8] {
            let mut processors: Vec<_> = (0..instances)
                .map(|_| {
                    Drive::new(
                        ProcessingContext {
                            sample_rate_hz: 48_000,
                            max_block_frames: 512,
                        },
                        &DriveSettings {
                            color_shape: DriveColorShape::MidHumpBassCut,
                            ..Default::default()
                        },
                    )
                })
                .collect();
            let input: Vec<[f32; 2]> = (0..frames)
                .map(|frame| {
                    [
                        (frame as f32 * 0.1).sin() * 0.1,
                        (frame as f32 * 0.13).cos() * 0.07,
                    ]
                })
                .collect();
            let mut elapsed = Vec::with_capacity(10_000);
            for block in 0..11_000 {
                let start = Instant::now();
                for frame in 0..frames {
                    let mut sample = input[frame as usize];
                    for processor in &mut processors {
                        sample = processor.process_sample(sample);
                    }
                    black_box(sample);
                }
                if block >= 1000 {
                    elapsed.push(start.elapsed().as_nanos() as u64);
                }
            }
            elapsed.sort_unstable();
            timing.push(serde_json::json!({"rate": 48_000, "frames": frames, "instances": instances, "blocks": elapsed.len(), "median_ns": elapsed[5000], "p99_ns": elapsed[9900], "period_ns": f64::from(frames) / 48_000.0 * 1e9}));
        }
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"tail_threshold_dbfs": -120, "tail_input": [4.0, -2.0], "tail_policy_seconds": 1, "tails": tails, "direct_drive_timings": timing})).unwrap());
}
