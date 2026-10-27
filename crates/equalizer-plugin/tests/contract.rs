use daw_equalizer::Equalizer as Direct;
use daw_equalizer_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");

#[test]
fn builtin_band_levels_are_published_through_the_existing_meter_snapshot() {
    use std::sync::Arc;
    let meter = Arc::new(plugin::OutputMeter::default());
    let mut processor =
        plugin::Processor::new(48000.0, 1, 512, plugin::PluginState::default()).unwrap();
    processor.set_output_meter(meter.clone());
    for i in 0..4096 {
        let v = (i as f32 * std::f32::consts::TAU * 100.0 / 48000.0).sin() * 0.25;
        processor.process_sample([v, -v]);
    }
    assert!(meter.bands.readings().iter().any(|v| *v > 0.0));
    let values = plugin::meter_values(&meter);
    let received = plugin::OutputMeter::default();
    plugin::receive_meter(&received, values.map(|v| v as f32));
    assert_eq!(meter.bands.readings(), received.bands.readings());
    processor.set(plugin::Parameter::Bypass, 1.0).unwrap();
    assert_eq!(meter.bands.readings(), [0.0; 3]);
}
