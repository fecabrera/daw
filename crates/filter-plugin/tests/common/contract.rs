use daw_unit_core::{EffectUnit, ProcessingContext};
use plugin::{Parameter as P, PluginState, Processor, SUPPORTED_RATES};
#[test]
fn frozen_manifest_identity_and_default_state_remain_compatible() {
    let identity: serde_json::Value = serde_json::from_str(IDENTITY_FIXTURE).unwrap();
    assert_eq!(plugin::IDENTITY.product, identity["product"]);
    assert_eq!(plugin::IDENTITY.clap_id, identity["clap_id"]);
    let hex = |bytes: [u8; 16]| bytes.map(|v| format!("{v:02x}")).join("");
    assert_eq!(hex(plugin::IDENTITY.processor_id), identity["processor_id"]);
    assert_eq!(
        hex(plugin::IDENTITY.controller_id),
        identity["controller_id"]
    );
    assert_eq!(PluginState::default().encode().as_slice(), DEFAULT_FIXTURE);
    let bytes = PluginState::default().encode();
    assert_eq!(PluginState::decode(&bytes).unwrap(), PluginState::default());
    for index in [0, 8, 12, 16] {
        let mut invalid = bytes;
        invalid[index..index + 4].fill(255);
        assert!(PluginState::decode(&invalid).is_err());
    }
    assert!(PluginState::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(PluginState::decode(&[bytes.as_slice(), &[0]].concat()).is_err());
}
#[test]
fn every_parameter_validates_and_roundtrips_in_host_and_text_domains() {
    for (index, p) in P::ALL.into_iter().enumerate() {
        assert_eq!(p as u32, 100 + index as u32);
        assert_eq!(P::from_id(p as u32).unwrap(), p);
        for v in [p.info().minimum, p.info().default, p.info().maximum] {
            let n = p.normalize(v).unwrap();
            let restored = p.denormalize(n).unwrap();
            assert!((v - restored).abs() < 1e-7);
            let text = p.format(v).unwrap();
            let read = p.parse(&text).unwrap();
            assert!((read - v).abs() <= 0.001, "{p:?}: {text}");
            let mut s = PluginState::default();
            s.set(p, v).unwrap();
            assert!(s.to_settings().valid());
            assert_eq!(PluginState::decode(&s.encode()).unwrap(), s);
        }
        for bad in [
            f64::NAN,
            f64::INFINITY,
            p.info().minimum - 1.0,
            p.info().maximum + 1.0,
        ] {
            assert!(p.validate(bad).is_err());
        }
    }
    assert!(P::from_id(0).is_err());
    assert!(P::from_id(u32::MAX).is_err());
}
#[test]
fn prepared_wrapper_matches_portable_dsp_and_meters_follow_bypass_at_all_rates() {
    for rate in SUPPORTED_RATES {
        let state = PluginState::default();
        let mut processor = Processor::new(f64::from(rate), 1, 4096, state).unwrap();
        let meter = std::sync::Arc::new(plugin::OutputMeter::default());
        processor.set_output_meter(meter.clone());
        let mut direct = Direct::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &state.to_settings(),
        );
        for frame in 0..4096 {
            let input = [(frame as f32 * 0.17).sin() * 0.7, -0.2];
            assert_eq!(
                processor.process_sample(input),
                direct.process_sample(input)
            );
        }
        processor.reset();
        direct.reset();
        for frame in 0..4096 {
            let input = [(frame as f32 * 0.11).sin() * 0.5, -0.3];
            assert_eq!(
                processor.process_sample(input),
                direct.process_sample(input)
            );
        }
        assert!(meter.readings()[0] > 0.0);
        let mut off = state;
        off.set(P::Bypass, 1.0).unwrap();
        processor.restore(off);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        for _ in 0..1024 {
            processor.process_sample([1.0; 2]);
        }
        assert_eq!(meter.readings(), [0.0; 3]);
    }
    for rate in [0.0, 48000.5, f64::NAN] {
        assert!(Processor::new(rate, 1, 512, PluginState::default()).is_err());
    }
}
