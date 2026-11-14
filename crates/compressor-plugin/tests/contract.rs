use daw_compressor::Compressor as Direct;
use daw_compressor_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");

#[test]
fn curve_detector_uses_existing_telemetry_and_stop_hook_preserves_output_and_reduction_holds() {
    use daw_unit_core::{BandAudition, MeterStorage};
    let meter = std::sync::Arc::new(plugin::OutputMeter::default());
    let mut processor =
        plugin::Processor::new(48000.0, 1, 512, plugin::PluginState::default()).unwrap();
    processor.set_output_meter(meter.clone());
    for _ in 0..4800 {
        processor.process_sample([0.5, -0.25]);
    }
    assert!(meter.bands.readings()[0] > 0.49);
    let values = plugin::meter_values(&meter);
    assert_eq!(values.len(), 13);
    assert_eq!(values[4] as f32, meter.bands.readings()[0]);
    let received = plugin::OutputMeter::default();
    plugin::receive_meter(&received, values.map(|v| v as f32));
    assert_eq!(received.bands.readings()[0], meter.bands.readings()[0]);
    let reduction = meter.reduction.peak();
    let peak = meter.boost_peaks()[0].peak();
    meter.clear_live_preview();
    assert_eq!(meter.bands.readings(), [0.0; 3]);
    assert_eq!(meter.reduction.peak(), reduction);
    assert_eq!(meter.boost_peaks()[0].peak(), peak);
    meter.clear();
    assert_eq!(meter.readings(), [0.0; 3]);
}

#[test]
fn curve_state_migrates_v1_and_preserves_parameter_ids_and_atomic_restore() {
    use plugin::{Parameter as P, PluginState};
    let legacy = include_bytes!("fixtures/default-state-v1.bin");
    assert_eq!(PluginState::decode(legacy).unwrap(), PluginState::default());
    assert_eq!(
        PluginState::read_from(&mut &legacy[..])
            .unwrap()
            .get(P::Curve),
        0.0
    );
    let mut edited = legacy.to_vec();
    for (chunk, value) in edited[16..]
        .chunks_exact_mut(8)
        .zip([0.0_f64, -24.0, 8.0, 1.0, 250.0, 12.0, 3.0, 0.25, 1.0])
    {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    let restored = PluginState::decode(&edited).unwrap().to_settings();
    assert_eq!(restored.threshold_db, -24.0);
    assert_eq!(restored.ratio, 8.0);
    assert_eq!(restored.detection, daw_unit_core::CompressorDetection::Rms);
    assert_eq!(restored.curve, 0.0);
    assert_eq!(P::Detection as u32, 108);
    assert_eq!(P::Curve as u32, 109);
    assert_eq!(plugin::STATE_VERSION, 2);
    for curve in [-1.0, 0.0, 0.75, 1.0] {
        let settings = daw_unit_core::CompressorSettings {
            curve,
            ..Default::default()
        };
        let state = PluginState::from_settings(&settings).unwrap();
        assert_eq!(state.to_settings().curve, curve);
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
        let mut current = state;
        let mut corrupt = state.encode();
        corrupt[88..96].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(current.restore(&corrupt).is_err());
        assert_eq!(current, state);
        corrupt[88..96].copy_from_slice(&1.01_f64.to_le_bytes());
        assert!(current.restore(&corrupt).is_err());
        assert_eq!(current, state);
    }
}
