use daw_unit_core::{EffectUnit, ProcessingContext};
#[test]
fn identity_defaults_and_state_are_frozen_and_foreign_states_are_rejected() {
    let identity: serde_json::Value = serde_json::from_str(IDENTITY_FIXTURE).unwrap();
    assert_eq!(IDENTITY.product, identity["product"]);
    assert_eq!(IDENTITY.clap_id, identity["clap_id"]);
    let hex = |b: [u8; 16]| b.map(|v| format!("{v:02x}")).join("");
    assert_eq!(hex(IDENTITY.processor_id), identity["processor_id"]);
    assert_eq!(hex(IDENTITY.controller_id), identity["controller_id"]);
    let expected: Vec<_> = DEFAULT_FIXTURE
        .split_whitespace()
        .map(|v| u8::from_str_radix(v, 16).unwrap())
        .collect();
    assert_eq!(PluginState::default().encode().as_slice(), expected);
    assert_eq!(PluginState::default().to_settings(), Settings::default());
    assert_eq!(P::ALL.map(|p| p as u32), [100, 101, 102, 103, 104, 105]);
    assert_eq!(PANEL.len(), 2);
    assert_eq!(PANEL[0].controls.len(), 3);
    assert_eq!(PANEL[1].controls.len(), 2);
    let mut state = PluginState::default();
    state.set(P::Drive, 12.0).unwrap();
    let original = state;
    let bytes = state.encode();
    for offset in [0, 8, 12, 16] {
        let mut invalid = bytes;
        invalid[offset..offset + 4].fill(255);
        assert!(state.restore(&invalid).is_err());
        assert_eq!(state, original);
    }
    for size in 0..MAX_STATE_BYTES {
        assert!(PluginState::decode(&bytes[..size]).is_err());
    }
    assert!(PluginState::decode(&[bytes.as_slice(), &[0]].concat()).is_err());
    let mut reader = std::io::Cursor::new(bytes);
    assert_eq!(PluginState::read_from(&mut reader).unwrap(), state);
    let mut encoded = vec![];
    state.write_to(&mut encoded).unwrap();
    assert_eq!(encoded, bytes);
}
#[test]
fn parameters_cover_bounds_integer_choices_text_and_silence() {
    for p in P::ALL {
        let info = p.info();
        for value in [info.minimum, info.default, info.maximum] {
            assert!((p.denormalize(p.normalize(value).unwrap()).unwrap() - value).abs() < 1e-8);
            assert!(p.parse(&p.format(value).unwrap()).is_ok());
        }
        for value in [
            f64::NAN,
            f64::INFINITY,
            info.minimum - 1.0,
            info.maximum + 1.0,
        ] {
            assert!(p.validate(value).is_err());
        }
        if info.steps > 0 {
            for value in info.minimum as u32..=info.maximum as u32 {
                let value = f64::from(value);
                assert_eq!(p.denormalize(p.normalize(value).unwrap()).unwrap(), value);
            }
            assert!(p.validate(info.minimum + 0.5).is_err());
        }
        for n in [f64::NAN, -0.01, 1.01] {
            assert!(p.denormalize(n).is_err());
        }
    }
    assert_eq!(P::Level.parse("-inf dB"), Ok(-1000.0));
    assert_eq!(P::Drive.unit(), "dB");
    assert_eq!(P::DryWet.unit(), "%");
}
#[test]
fn portable_dsp_parameter_updates_latency_and_output_monitoring_match() {
    for rate in SUPPORTED_RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut state = PluginState::default();
        let mut processor = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let mut direct = Dsp::new(context, &state.to_settings());
        let meter = std::sync::Arc::new(OutputMeter::default());
        processor.set_output_meter(meter.clone());
        assert_eq!(direct.latency_frames(), LATENCY_FRAMES);
        for p in P::ALL {
            state.set(p, p.info().maximum).unwrap();
            processor.apply_parameter(p, &state);
            direct.retarget(&state.to_settings());
            for i in 0..512 {
                let input = [(i as f32 * 0.13).sin() * 0.7, -0.2];
                assert_eq!(
                    processor.process_sample(input),
                    direct.process_sample(input)
                );
            }
            assert_eq!(processor.state(), state);
        }
        state.set(P::Bypass, 0.0).unwrap();
        processor.restore(state);
        direct.retarget(&state.to_settings());
        for _ in 0..512 {
            assert_eq!(
                processor.process_sample([0.2, -0.4]),
                direct.process_sample([0.2, -0.4])
            );
        }
        assert!(meter.boost_peaks()[0].peak() > 0.0);
        state.set(P::Bypass, 1.0).unwrap();
        processor.restore(state);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
    }
    for rate in [0.0, 8000.0, 48000.5, f64::NAN] {
        assert!(Processor::new(rate, 1, 512, PluginState::default()).is_err());
    }
    for (lo, hi) in [(0, 512), (2, 1), (1, u32::MAX)] {
        assert!(Processor::new(48000.0, lo, hi, PluginState::default()).is_err());
    }
}
#[test]
fn atomic_recall_publishes_coherent_valid_snapshots() {
    let exchange = StateExchange::default();
    let mut applied = 0;
    let mut s = PluginState::default();
    s.set(P::Drive, 12.0).unwrap();
    exchange.recall(s);
    assert_eq!(exchange.consume(&mut applied), Some(s));
    assert!(exchange.consume(&mut applied).is_none());
    exchange.publish(s, applied);
    assert_eq!(exchange.current(), Some(s));
}
