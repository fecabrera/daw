use daw_multiband_compressor_plugin::{Parameter as P, PluginState, Processor, SUPPORTED_RATES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
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
#[test]
fn default_portable_audio_state_identity_and_all_parameter_conversions_match() {
    let state = PluginState::default();
    assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    assert_eq!(
        PluginState::from_settings(&state.to_settings()).unwrap(),
        state
    );
    for p in P::ALL {
        for value in [p.info().minimum, p.info().default, p.info().maximum] {
            assert!((p.denormalize(p.normalize(value).unwrap()).unwrap() - value).abs() < 1e-8);
            assert!((p.parse(&p.format(value).unwrap()).unwrap() - value).abs() < 0.001);
        }
    }
    use daw_unit_core::EffectUnit;
    for rate in SUPPORTED_RATES {
        let mut actual = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let mut expected = daw_multiband_compressor::MultibandCompressor::new(
            daw_unit_core::ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            },
            &state.to_settings().core,
        );
        for frame in 0..4096 {
            let x = (frame as f32 * 0.17).sin() * 0.3;
            assert_eq!(
                actual.process_sample([x, -x]),
                expected.process_sample([x, -x])
            );
        }
    }
    let mut invalid = state.encode();
    invalid[8] = 99;
    assert!(PluginState::decode(&invalid).is_err());
    let mut invalid = state.encode();
    invalid[16..24].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(PluginState::decode(&invalid).is_err());
}
#[test]
fn dense_automation_presence_changes_resets_and_meter_solos_do_not_allocate_or_retire() {
    for rate in SUPPORTED_RATES {
        let mut state = PluginState::default();
        let mut actual = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let meter = Arc::new(daw_multiband_compressor_plugin::OutputMeter::default());
        actual.set_output_meter(meter.clone());
        OPERATIONS.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        for (i, p) in P::ALL.into_iter().enumerate() {
            state.set(p, p.info().maximum).unwrap();
            actual.apply_parameter(p, &state);
            for _ in 0..2 {
                assert!(
                    actual
                        .process_sample([0.01, -0.01])
                        .iter()
                        .all(|v| v.is_finite())
                );
            }
            if i % 27 == 0 {
                actual.reset();
            }
        }
        actual.restore(PluginState::default());
        actual.process_sample([0.1, -0.2]);
        TRACK.with(|t| t.set(false));
        assert_eq!(OPERATIONS.with(Cell::get), 0);
    }
}
#[test]
fn plugin_solos_are_transient_and_offline_gate_ignores_audition_immediately() {
    use daw_unit_core::BandAudition;
    let state = PluginState::default();
    let mut actual = Processor::new(48000.0, 1, 512, state).unwrap();
    let mut reference = Processor::new(48000.0, 1, 512, state).unwrap();
    let meter = Arc::new(daw_multiband_compressor_plugin::OutputMeter::default());
    actual.set_output_meter(meter.clone());
    meter.solos[0].store(true, std::sync::atomic::Ordering::Relaxed);
    for i in 0..4096 {
        let x = (i as f32 * 0.18).sin() * 0.1;
        actual.process_sample([x, -x]);
        reference.process_sample([x, -x]);
    }
    meter.set_audition_allowed(false);
    for i in 0..512 {
        let x = (i as f32 * 0.18).sin() * 0.1;
        assert_eq!(
            actual.process_sample([x, -x]),
            reference.process_sample([x, -x])
        );
    }
    assert_eq!(actual.state(), state);
    assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    actual.set(P::Bypass, 1.0).unwrap();
    assert!(!meter.solos[0].load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn visual_order_is_persistent_and_never_rebinds_public_automation_slots() {
    assert_eq!(P::COUNT, 643);
    assert_eq!(daw_multiband_compressor_plugin::MAX_STATE_BYTES, 5224);
    let mut state = PluginState::default();
    let values = P::ALL.map(|p| state.get(p));
    let mut order = state.presentation();
    order.reverse();
    assert!(state.set_presentation(order));
    assert_eq!(P::ALL.map(|p| state.get(p)), values);
    assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    assert_eq!(
        PluginState::from_settings(&state.to_settings()).unwrap(),
        state
    );
    let mut invalid = order;
    invalid[0] = invalid[1];
    assert!(!state.set_presentation(invalid));
    assert_eq!(state.presentation(), order);
    let mut invalid = state.to_settings();
    invalid.core.bands[0].id = uuid::Uuid::from_u128((1_u128 << 64) + 1);
    invalid.core.presentation.clear();
    assert!(PluginState::from_settings(&invalid).is_err());
    let mut invalid = state.to_settings();
    invalid.core.bands.clear();
    invalid.core.presentation = vec![uuid::Uuid::from_u128(1); 65];
    assert!(PluginState::from_settings(&invalid).is_err());
}

#[test]
fn band_level_holds_capture_between_ui_reads_and_wire_resets_leave_reduction_and_out_unchanged() {
    use daw_unit_core::{BandAudition, MeterStorage};
    let mut state = PluginState::default();
    state.set(P::Band2Present, 0.0).unwrap();
    state.set(P::Band3Present, 0.0).unwrap();
    state.set(P::Band1Ratio, 1.0).unwrap();
    let meter = Arc::new(daw_multiband_compressor_plugin::OutputMeter::default());
    let mut p = Processor::new(48000.0, 1, 512, state).unwrap();
    p.set_output_meter(meter.clone());
    p.process_sample([0.8, -0.9]);
    meter.reductions[0].store(6.0);
    p.process_sample([0.0; 2]);
    assert!(meter.eq_bands[0].readings()[0] > 0.89);
    assert_eq!(meter.eq_bands[0].boost_peaks()[0].peak(), 0.9);
    let mirror = daw_multiband_compressor_plugin::OutputMeter::default();
    let values = daw_multiband_compressor_plugin::meter_values(&meter);
    assert_eq!(values.len(), 796);
    daw_multiband_compressor_plugin::receive_meter(&mirror, values.map(|v| v as f32));
    assert_eq!(mirror.eq_bands[0].boost_peaks()[0].peak(), 0.9);
    assert!(mirror.reset_band_level_peak(0));
    assert!(mirror.eq_bands[0].readings()[0] > 0.89);
    assert_eq!(mirror.eq_bands[0].boost_peaks()[0].peak(), 0.0);
    assert_eq!(mirror.reductions[0].peak(), 6.0);
    assert_eq!(mirror.boost_peaks()[0].peak(), 0.9);
    assert!(!mirror.reset_band_level_peak(64));
    p.set(P::Bypass, 1.0).unwrap();
    assert_eq!(meter.eq_bands[0].boost_peaks()[0].peak(), 0.0);
}

#[test]
fn spectrum_is_pcm_neutral_bounded_and_coherent_through_plugin_telemetry() {
    use daw_multiband_compressor_plugin as plugin;
    use daw_unit_core::{BandAudition, SpectrumCache};
    for rate in SUPPORTED_RATES {
        let state = PluginState::default();
        let meter = Arc::new(plugin::OutputMeter::default());
        let mirror = plugin::OutputMeter::default();
        let mut actual = Processor::new(rate as f64, 1, 512, state).unwrap();
        let mut reference = Processor::new(rate as f64, 1, 512, state).unwrap();
        actual.set_output_meter(meter.clone());
        assert_eq!(plugin::meter_values(&meter)[269], rate as f64);
        OPERATIONS.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        let mut equal = true;
        for i in 0..rate / 4 {
            let x = (i as f32 * std::f32::consts::TAU * 80.0 / rate as f32).sin() * 0.5;
            equal &= actual.process_sample([x, -x]) == reference.process_sample([x, -x]);
        }
        TRACK.with(|t| t.set(false));
        assert!(equal);
        assert_eq!(OPERATIONS.with(Cell::get), 0);
        let frame = meter.spectrum.reading();
        assert_eq!(frame.valid_branches, 1);
        assert!(frame.magnitudes.iter().any(|v| *v > 0.01));
        plugin::receive_meter(&mirror, plugin::meter_values(&meter).map(|v| v as f32));
        assert_eq!(mirror.spectrum.reading(), frame);
        let mut cache = SpectrumCache::default();
        assert!(cache.read(&mirror.spectrum).is_some());
        let before = plugin::meter_values(&mirror);
        for (index, value) in [
            (270, -1.0),
            (782, 1.0),
            (783, 2.0),
            (269, 12345.0),
            (790, f32::NAN),
        ] {
            let mut malformed = before.map(|v| v as f32);
            malformed[index] = value;
            plugin::receive_meter(&mirror, malformed);
            assert_eq!(plugin::meter_values(&mirror), before);
        }
        actual.set(P::Bypass, 1.0).unwrap();
        assert_eq!(meter.spectrum.reading().valid_branches, 0);
        actual.set(P::Bypass, 0.0).unwrap();
        for _ in 0..rate / 4 {
            actual.process_sample([0.1, -0.1]);
        }
        assert_eq!(meter.spectrum.reading().valid_branches, 1);
        actual.reset();
        assert_eq!(meter.spectrum.reading().valid_branches, 0);
        meter.clear_spectrum();
    }
}
