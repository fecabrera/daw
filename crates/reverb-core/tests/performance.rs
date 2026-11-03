//! Explicit release-only DSP measurement; it does not establish device deadlines.
use daw_reverb_core::{Parameters, Reverb};
use daw_unit_core::ProcessingContext;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[test]
#[ignore = "explicit release-only callback timing measurement"]
// An explicit debug invocation must fail instead of reporting misleading timings.
#[allow(clippy::assertions_on_constants)]
fn callback_timing_at_default_and_minimum_buffers() {
    assert!(!cfg!(debug_assertions), "run with --release");
    for (rate, block_frames) in [(48_000, 512), (192_000, 32)] {
        for plate in [true, false] {
            let p = Parameters {
                values: [
                    if plate { 15.0 } else { 60.0 },
                    0.0,
                    20.0,
                    15000.0,
                    0.0,
                    1.0,
                    1.0,
                    1.0,
                    1.0,
                    0.0,
                    0.5,
                    1.0,
                ],
                hq: true,
                gated: false,
                dark: false,
                drive_enabled: true,
                drive_mode: Default::default(),
                highpass_enabled: true,
                lowpass_enabled: true,
                highpass_slope: daw_unit_core::ReverbFilterSlope::Db12,
                lowpass_slope: daw_unit_core::ReverbFilterSlope::Db12,
            };
            let mut unit = Reverb::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: block_frames,
                },
                p,
                plate,
            );
            for _ in 0..65536 {
                black_box(unit.sample([0.1, -0.05], [0.0; 12]));
            }
            let mut maximum = Duration::ZERO;
            let start = Instant::now();
            for _ in 0..32768 / block_frames {
                let block = Instant::now();
                for _ in 0..block_frames {
                    black_box(unit.sample([0.1, -0.05], [0.0; 12]));
                }
                maximum = maximum.max(block.elapsed());
            }
            eprintln!(
                "{} at {} Hz / {} frames: average utilization {:.3}%, maximum block {:.3} us, nominal block period {:.3} us, prepared heap {} bytes",
                if plate { "Plate" } else { "Ambient" },
                rate,
                block_frames,
                start.elapsed().as_secs_f64() / (32768.0 / f64::from(rate)) * 100.0,
                maximum.as_secs_f64() * 1e6,
                f64::from(block_frames) / f64::from(rate) * 1e6,
                unit.heap_bytes()
            );
        }
    }
}
