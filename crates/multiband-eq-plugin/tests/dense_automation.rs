//! Preserve the full-retarget reference while bounding each automation update.
use daw_multiband_eq_plugin::{Parameter as P, PluginState, Processor, SUPPORTED_RATES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::{hint::black_box, time::Instant};
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static OPERATIONS: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
// SAFETY: All operations forward unchanged pointers/layouts to System.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                OPERATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        TRACK.with(|track| {
            if track.get() {
                OPERATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                OPERATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn state() -> PluginState {
    let mut state = PluginState::default();
    for slot in 0..64 {
        state.set(P::ALL[1 + 5 * slot], 1.0).unwrap();
    }
    let mut order = state.presentation();
    order.reverse();
    assert!(state.set_presentation(order));
    state
}
#[test]
fn per_field_updates_preserve_dense_automation_ramps_reset_meters_and_heap_bounds() {
    for rate in SUPPORTED_RATES {
        let mut state = state();
        let mut actual = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let mut reference = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let meter = Arc::new(daw_multiband_eq_plugin::OutputMeter::default());
        let expected = Arc::new(daw_multiband_eq_plugin::OutputMeter::default());
        actual.set_output_meter(meter.clone());
        reference.set_output_meter(expected.clone());
        OPERATIONS.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        for frame in 0..64 {
            for p in P::ALL {
                state
                    .set_normalized(p, if frame % 2 == 0 { 0.25 } else { 0.75 })
                    .unwrap();
                if frame % 2 == 0 {
                    actual.apply_parameter(p, &state);
                } else {
                    actual.set(p, state.get(p)).unwrap();
                }
                // Existing full-bank path is the independent sample-time target reference.
                reference.restore(state);
            }
            // A transient target followed by the former target must still restart its ramp.
            let p = P::Band1Frequency;
            let previous = state.get(p);
            for value in [1300.0, previous] {
                state.set(p, value).unwrap();
                actual.set(p, value).unwrap();
                reference.restore(state);
            }
            if frame % 17 == 0 {
                actual.reset();
                reference.reset();
            }
            let input = [0.1, -0.1];
            assert_eq!(
                actual.process_sample(input),
                reference.process_sample(input),
                "rate={rate} frame={frame}"
            );
            assert_eq!(actual.state(), state);
            assert_eq!(
                daw_multiband_eq_plugin::meter_values(&meter),
                daw_multiband_eq_plugin::meter_values(&expected)
            );
        }
        TRACK.with(|t| t.set(false));
        assert_eq!(OPERATIONS.with(Cell::get), 0);
    }
}

fn workload(fields: usize, full: bool) -> f64 {
    let mut state = state();
    let mut processor = Processor::new(48000.0, 1, 512, state).unwrap();
    for _ in 0..1024 {
        black_box(processor.process_sample([0.1, -0.1]));
    }
    let start = Instant::now();
    for frame in 0..4800 {
        for index in 0..fields {
            let p = if fields == 64 {
                P::ALL[3 + 5 * index]
            } else {
                P::ALL[index]
            };
            state
                .set_normalized(p, if frame % 2 == 0 { 0.25 } else { 0.75 })
                .unwrap();
            if full {
                processor.restore(state);
            } else {
                processor.apply_parameter(p, &state);
            }
        }
        black_box(processor.process_sample(black_box([0.1, -0.1])));
    }
    start.elapsed().as_secs_f64() * 1000.0
}
#[test]
#[ignore = "explicit release-only automation cost regression; run without other CPU workloads"]
fn release_dense_automation_cost_is_bounded_against_full_bank_reference() {
    if cfg!(debug_assertions) {
        panic!("run this cost regression with --release");
    }
    println!(
        "48 kHz,64 active bands,4800 frames (100 ms audio); baseline {:.3} ms",
        workload(0, false)
    );
    for fields in [64, 322] {
        // Warm both complete paths, then balance order to avoid a systematic
        // first-path cache or clock bias. Keep total render time and the gate.
        black_box(workload(fields, false));
        black_box(workload(fields, true));
        let mut fast = [0.0_f64; 7];
        let mut full = [0.0_f64; 7];
        for i in 0..7 {
            if i % 2 == 0 {
                fast[i] = workload(fields, false);
                full[i] = workload(fields, true);
            } else {
                full[i] = workload(fields, true);
                fast[i] = workload(fields, false);
            }
        }
        fast.sort_by(f64::total_cmp);
        full.sort_by(f64::total_cmp);
        println!(
            "{fields} fields: per-slot median {:.3} ms,full-bank reference {:.3} ms",
            fast[3], full[3]
        );
        assert!(
            fast[3] < full[3] * 0.5,
            "dense automation regressed to full-bank cost"
        );
    }
}

#[test]
fn continuous_spectrum_processing_and_lifecycle_preserve_pcm_without_heap_operations() {
    use daw_unit_core::{EffectUnit, MultibandEqSettings, ProcessingContext, SpectrumMeter};
    for rate in SUPPORTED_RATES {
        let settings = MultibandEqSettings::default();
        let c = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 32,
        };
        let mut actual = daw_equalizer::MultibandEq::new(c, &settings);
        let mut reference = actual.clone();
        let meter = Arc::new(SpectrumMeter::default());
        actual.set_spectrum_meter(meter.clone());
        let mut prepared = daw_equalizer::MultibandEq::new(c, &settings);
        prepared.set_spectrum_meter(meter.clone());
        OPERATIONS.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        for frame in 0..rate {
            let phase = frame as f64 * std::f64::consts::TAU * 87.3 / f64::from(rate);
            let input = [phase.sin() as f32 * 1.3, phase.cos() as f32 * 0.7];
            assert_eq!(
                actual.process_sample(input),
                reference.process_sample(input)
            );
        }
        // Retain prepared/retired owners until after the heap guard ends.
        prepared.inherit(&mut actual);
        prepared.reset();
        prepared.retarget_enabled(false);
        for _ in 0..512 {
            black_box(prepared.process_sample([0.1, -0.1]));
        }
        prepared.retarget_enabled(true);
        for _ in 0..512 {
            black_box(prepared.process_sample([0.1, -0.1]));
        }
        TRACK.with(|t| t.set(false));
        assert_eq!(OPERATIONS.with(Cell::get), 0, "rate={rate}");
    }
}
