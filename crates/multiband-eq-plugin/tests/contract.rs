use daw_equalizer::MultibandEq as Direct;
use daw_multiband_eq_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");

#[test]
fn all_fixed_band_levels_survive_meter_transport_and_clear_on_bypass() {
    use std::sync::Arc;
    let mut state = plugin::PluginState::default();
    state.set(plugin::Parameter::ALL[1 + 5 * 63], 1.0).unwrap();
    let meter = Arc::new(plugin::OutputMeter::default());
    let mut processor = plugin::Processor::new(48000.0, 1, 512, state).unwrap();
    processor.set_output_meter(meter.clone());
    for i in 0..12288 {
        let v = (i as f32 * std::f32::consts::TAU * 1000.0 / 48000.0).sin() * 0.25;
        processor.process_sample([v, -v]);
    }
    let values = plugin::meter_values(&meter);
    assert_eq!(values.len(), 668);
    assert!(
        values[13] > 0.0 && values[13 + 63] > 0.0,
        "first and final band must publish levels"
    );
    let received = plugin::OutputMeter::default();
    plugin::receive_meter(&received, values.map(|v| v as f32));
    for i in 0..64 {
        assert_eq!(
            received.eq_bands[i].readings()[0],
            meter.eq_bands[i].readings()[0]
        );
    }
    assert!(
        received
            .spectrum
            .reading()
            .magnitudes
            .iter()
            .any(|v| *v > 0.0)
    );
    assert_eq!(received.spectrum.reading().sample_rate_hz, 48000);
    assert_eq!(
        received.spectrum.reading().magnitudes,
        meter.spectrum.reading().magnitudes
    );
    processor.set(plugin::Parameter::Bypass, 1.0).unwrap();
    for _ in 0..512 {
        processor.process_sample([0.25, -0.25]);
    }
    assert!(
        plugin::meter_values(&meter)[13..141]
            .iter()
            .all(|v| *v == 0.0)
    );
    assert!(
        meter
            .spectrum
            .reading()
            .magnitudes
            .iter()
            .all(|v| *v == 0.0)
    );
}

#[test]
fn saved_band_order_preserves_slot_parameters_and_recalls_legacy_state() {
    use plugin::{Parameter as P, PluginState, StateExchange};
    let legacy = include_bytes!("fixtures/legacy-default-state.bin");
    assert_eq!(PluginState::decode(legacy).unwrap(), PluginState::default());
    let mut state = PluginState::default();
    state.set(P::ALL[4], 6.0).unwrap();
    let values = P::ALL.map(|p| state.get(p));
    let mut order = state.presentation();
    order.rotate_left(1);
    assert!(state.set_presentation(order));
    assert_eq!(P::ALL.map(|p| state.get(p)), values);
    assert_eq!(
        PluginState::from_settings(&state.to_settings()).unwrap(),
        state
    );
    assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    let exchange = StateExchange::default();
    exchange.recall(state);
    assert_eq!(exchange.current(), Some(state));
    let mut revision = 0;
    assert_eq!(exchange.consume(&mut revision), Some(state));
    exchange.publish(state, revision);
    assert_eq!(exchange.current(), Some(state));
    let mut invalid = state.encode();
    let length = invalid.len();
    invalid[length - 64] = invalid[length - 63];
    assert!(PluginState::decode(&invalid).is_err());
}

#[test]
fn prepared_slot_update_rejects_wrong_identity_or_index_without_changing_audio() {
    use daw_unit_core::{EffectUnit, ProcessingContext};
    let settings = plugin::PluginState::default().to_settings();
    let context = ProcessingContext {
        sample_rate_hz: 48000,
        max_block_frames: 512,
    };
    let mut actual = Direct::new(context, &settings);
    let mut expected = Direct::new(context, &settings);
    actual.prepare_fixed_band_slots();
    expected.prepare_fixed_band_slots();
    let wrong = &settings.bands[1];
    assert!(!actual.retarget_prepared_band(0, wrong));
    assert!(!actual.retarget_prepared_band(64, wrong));
    for _ in 0..512 {
        assert_eq!(
            actual.process_sample([0.2, -0.1]),
            expected.process_sample([0.2, -0.1])
        );
    }
}

#[test]
fn band_holds_capture_raw_samples_and_reset_without_affecting_other_bands_or_out() {
    use daw_unit_core::BandAudition;
    use std::sync::Arc;
    let state = plugin::PluginState::default();
    let mut processor = plugin::Processor::new(48000.0, 1, 512, state).unwrap();
    let meter = Arc::new(plugin::OutputMeter::default());
    processor.set_output_meter(meter.clone());
    processor.process_sample([0.8, -0.9]);
    for _ in 0..64 {
        processor.process_sample([0.0; 2]);
    }
    assert_eq!(meter.eq_bands[0].boost_peaks()[0].peak(), 0.9);
    assert!(meter.eq_bands[0].readings()[0] < 0.9);
    assert!(meter.reset_band_level_peak(0));
    processor.process_sample([0.0; 2]);
    assert_eq!(
        meter.eq_bands[0].boost_peaks()[0].peak(),
        0.0,
        "release smoothing must not repopulate the hold"
    );
    assert_eq!(meter.eq_bands[63].boost_peaks()[0].peak(), 0.9);
    assert_eq!(meter.boost_peaks()[0].peak(), 0.9);
    let wire = plugin::meter_values(&meter);
    let receiver = plugin::OutputMeter::default();
    plugin::receive_meter(&receiver, wire.map(|v| v as f32));
    assert_eq!(receiver.eq_bands[63].boost_peaks()[0].peak(), 0.9);
    assert_eq!(receiver.eq_bands[0].boost_peaks()[0].peak(), 0.0);
    assert!(!receiver.reset_band_level_peak(64));
    processor.set(plugin::Parameter::Bypass, 1.0).unwrap();
    assert!(
        meter
            .eq_bands
            .iter()
            .all(|m| m.boost_peaks()[0].peak() == 0.0)
    );
    assert_eq!(processor.state().get(plugin::Parameter::Bypass), 1.0);
}

#[test]
fn coherent_spectrum_wire_preserves_full_clocks_and_rejects_malformed_frames_atomically() {
    use daw_unit_core::{MeterStorage, SPECTRUM_BINS, SpectrumReading};
    let source = plugin::OutputMeter::default();
    let frame = SpectrumReading {
        sample_rate_hz: 192000,
        magnitudes: std::array::from_fn(|i| i as f32 / SPECTRUM_BINS as f32),
        valid_branches: 3,
        generation: u64::MAX - 17,
        upper_end_frame: u64::MAX - 3,
        bass_end_frame: u64::MAX - 4000,
    };
    source.spectrum.store(frame);
    let wire = plugin::meter_values(&source).map(|v| v as f32);
    let receiver = plugin::OutputMeter::default();
    plugin::receive_meter(&receiver, wire);
    assert_eq!(receiver.spectrum.reading(), frame);
    for (slot, value) in [
        (plugin::SPECTRUM_SCHEMA, 1.0),
        (plugin::SPECTRUM_VALID, 2.0),
        (plugin::SPECTRUM_GENERATION, 65536.0),
        (plugin::SPECTRUM_UPPER_END + 1, 0.5),
        (plugin::SPECTRUM_FIRST, -1.0),
        (plugin::SPECTRUM_FIRST + 1, f32::NAN),
        (141, 12345.0),
        (1, f32::INFINITY),
    ] {
        let mut malformed = wire;
        malformed[0] = 100.0;
        malformed[slot] = value;
        plugin::receive_meter(&receiver, malformed);
        assert_eq!(receiver.spectrum.reading(), frame);
        assert_eq!(receiver.readings()[0], 0.0);
    }
    source.spectrum.clear();
    let wire = plugin::meter_values(&source).map(|v| v as f32);
    plugin::receive_meter(&receiver, wire);
    assert_eq!(receiver.spectrum.reading().valid_branches, 0);
    assert!(
        receiver
            .spectrum
            .reading()
            .magnitudes
            .iter()
            .all(|v| *v == 0.0)
    );
}

#[test]
fn serialized_processing_stop_clears_spectrum_and_consumer_cache_without_resetting_holds() {
    use daw_unit_core::{BandAudition, MeterStorage, SpectrumCache, SpectrumReading};
    let meter = plugin::OutputMeter::default();
    meter.output.store([0.8, 0.0, 0.0]);
    meter.spectrum.store(SpectrumReading {
        magnitudes: [0.5; daw_unit_core::SPECTRUM_BINS],
        valid_branches: 3,
        upper_end_frame: 48000,
        bass_end_frame: 47039,
        ..Default::default()
    });
    let mut cache = SpectrumCache::default();
    assert!(cache.read(&meter.spectrum).is_some());
    meter.clear_spectrum();
    assert!(cache.read(&meter.spectrum).is_none());
    assert_eq!(meter.boost_peaks()[0].peak(), 0.8);
    assert_eq!(meter.readings()[0], 0.8);
}

#[test]
fn empty_prepared_spectrum_uses_the_configured_processing_rate_before_first_audio() {
    use std::sync::Arc;
    for rate in plugin::SUPPORTED_RATES {
        let meter = Arc::new(plugin::OutputMeter::default());
        let mut processor =
            plugin::Processor::new(f64::from(rate), 1, 512, plugin::PluginState::default())
                .unwrap();
        processor.set_output_meter(meter.clone());
        let wire = plugin::meter_values(&meter);
        assert_eq!(wire[141], f64::from(rate));
        assert_eq!(wire[plugin::SPECTRUM_VALID], 0.0);
    }
}

#[test]
fn silent_partial_and_complete_readiness_survive_both_meter_paths() {
    use daw_unit_core::{BandAudition, MeterStorage, SpectrumCache, SpectrumReading};
    let shared = plugin::OutputMeter::default();
    let receiver = plugin::OutputMeter::default();
    let mut shared_cache = SpectrumCache::default();
    let mut wire_cache = SpectrumCache::default();
    for ready in [1, 3, 1] {
        let frame = SpectrumReading {
            sample_rate_hz: 48000,
            valid_branches: ready,
            upper_end_frame: 34816,
            bass_end_frame: if ready == 3 { 32831 } else { 0 },
            ..Default::default()
        };
        shared.spectrum.store(frame);
        plugin::receive_meter(&receiver, plugin::meter_values(&shared).map(|v| v as f32));
        // CLAP shares this handle; VST3 receives the same metadata over its wire.
        assert_eq!(shared_cache.read(&shared.spectrum), Some(frame));
        assert_eq!(wire_cache.read(&receiver.spectrum), Some(frame));
    }
    shared.clear_spectrum();
    plugin::receive_meter(&receiver, plugin::meter_values(&shared).map(|v| v as f32));
    assert!(shared_cache.read(&shared.spectrum).is_none());
    assert!(wire_cache.read(&receiver.spectrum).is_none());
}

#[test]
fn musical_transport_stop_retains_readiness_but_bypass_and_reset_clear_it() {
    use std::sync::Arc;
    let meter = Arc::new(plugin::OutputMeter::default());
    let mut processor =
        plugin::Processor::new(48000.0, 1, 512, plugin::PluginState::default()).unwrap();
    processor.set_output_meter(meter.clone());
    for _ in 0..12000 {
        assert_eq!(processor.process_sample([0.; 2]), [0.; 2]);
    }
    let partial = meter.spectrum.reading();
    assert_eq!(partial.valid_branches, 1);
    processor.set_transport_state(Some(12000), Some(120.0), Some(false));
    assert_eq!(meter.spectrum.reading(), partial);
    for _ in 12000..48000 {
        processor.process_sample([0.; 2]);
    }
    assert_eq!(meter.spectrum.reading().valid_branches, 1);
    processor.set(plugin::Parameter::Bypass, 1.0).unwrap();
    assert_eq!(meter.spectrum.reading().valid_branches, 0);
    processor.set(plugin::Parameter::Bypass, 0.0).unwrap();
    for _ in 0..12000 {
        processor.process_sample([0.; 2]);
    }
    assert_eq!(meter.spectrum.reading().valid_branches, 1);
    processor.reset();
    assert_eq!(meter.spectrum.reading().valid_branches, 0);
}
