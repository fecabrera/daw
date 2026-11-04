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
    for i in 0..4096 {
        let v = (i as f32 * std::f32::consts::TAU * 1000.0 / 48000.0).sin() * 0.25;
        processor.process_sample([v, -v]);
    }
    let values = plugin::meter_values(&meter);
    assert_eq!(values.len(), 77);
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
    processor.set(plugin::Parameter::Bypass, 1.0).unwrap();
    for _ in 0..512 {
        processor.process_sample([0.25, -0.25]);
    }
    assert!(plugin::meter_values(&meter)[13..].iter().all(|v| *v == 0.0));
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
