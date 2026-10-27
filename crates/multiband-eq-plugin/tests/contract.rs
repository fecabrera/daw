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
