use daw_loudness::Loudness as Direct;
use daw_loudness_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");
#[test]
fn telemetry_and_analysis_commands_cross_format_bridge_without_parameter_changes() {
    let source = plugin::OutputMeter::default();
    let destination = plugin::OutputMeter::default();
    let mut reading = daw_unit_core::LoudnessReading::default();
    reading.values[0] = -23.0;
    reading.values[1] = f32::NEG_INFINITY;
    reading.values[12] = 4.0;
    reading.values[daw_unit_core::LOUDNESS_SCALARS] = -23.0;
    source.loudness.publish(source.loudness.epoch(), reading);
    plugin::receive_meter(
        &destination,
        plugin::meter_values(&source).map(|v| v as f32),
    );
    assert_eq!(destination.loudness.reading(), reading);
    plugin::reset_meter(&source, 512);
    assert!(source.loudness.held());
    plugin::receive_meter(
        &destination,
        plugin::meter_values(&source).map(|v| v as f32),
    );
    assert!(destination.loudness.held());
    plugin::reset_meter(&source, 1024);
    assert!(!source.loudness.held());
    plugin::reset_meter(&source, 256);
    assert_eq!(source.loudness.reading(), Default::default());
}
#[test]
fn transport_mode_ignores_host_stopped_blocks_and_continuous_counts_them() {
    let mut p = Processor::new(48000.0, 1, 512, PluginState::default()).unwrap();
    let meter = std::sync::Arc::new(plugin::OutputMeter::default());
    p.set_output_meter(meter.clone());
    p.set_transport(None, None);
    for _ in 0..4800 {
        assert_eq!(p.process_sample([0.2; 2]), [0.2; 2]);
    }
    assert_eq!(meter.loudness.reading().values[12], 0.0);
    p.set(P::Clock, 1.0).unwrap();
    p.set_transport(None, None);
    for _ in 0..4800 {
        p.process_sample([0.2; 2]);
    }
    let start = std::time::Instant::now();
    while meter.loudness.reading().values[12] < 0.1 {
        assert!(start.elapsed().as_secs() < 3);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn playing_without_position_and_missing_context_resume_the_local_clock() {
    for playing in [Some(true), None] {
        let mut p = Processor::new(48000.0, 1, 512, PluginState::default()).unwrap();
        let meter = std::sync::Arc::new(plugin::OutputMeter::default());
        p.set_output_meter(meter.clone());
        p.set_transport_state(None, None, Some(false));
        for _ in 0..4800 {
            p.process_sample([0.2; 2]);
        }
        assert_eq!(meter.loudness.reading().values[12], 0.0);
        p.set_transport_state(None, Some(120.0), playing);
        for _ in 0..4800 {
            p.process_sample([0.2; 2]);
        }
        let start = std::time::Instant::now();
        while meter.loudness.reading().values[12] < 0.1 {
            assert!(
                start.elapsed().as_secs() < 3,
                "local clock stayed suspended: {playing:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}
