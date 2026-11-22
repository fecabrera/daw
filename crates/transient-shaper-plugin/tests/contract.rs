use daw_transient_shaper::TransientShaper;
use daw_transient_shaper_plugin::{
    IDENTITY, LATENCY_FRAMES, MAX_STATE_BYTES, PANEL, Parameter, PluginState, Processor,
    SUPPORTED_RATES, StateExchange, tail_frames,
};
use daw_unit_core::{
    EffectUnit, ProcessingContext, TransientShaperClipMode, TransientShaperSettings,
};

#[test]
fn state_and_clip_mode_automation_round_trip() {
    let identity: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/identity.json")).unwrap();
    let hex = |id: [u8; 16]| id.map(|byte| format!("{byte:02x}")).join("");
    assert_eq!(IDENTITY.product, identity["product"]);
    assert_eq!(IDENTITY.clap_id, identity["clap_id"]);
    assert_eq!(hex(IDENTITY.processor_id), identity["processor_id"]);
    assert_eq!(hex(IDENTITY.controller_id), identity["controller_id"]);
    let default: Vec<_> = include_str!("fixtures/default.hex")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(PluginState::default().encode().as_slice(), default);
    assert_eq!(
        PANEL.map(|strip| strip.footer),
        ["Shape", "Detector", "Out"]
    );
    assert_eq!(
        Parameter::ALL.map(|p| p as u32),
        [100, 101, 102, 103, 104, 105, 106, 107]
    );
    assert_eq!(
        PluginState::default().to_settings(),
        TransientShaperSettings::default()
    );
    let mut state = PluginState::default();
    for (index, mode) in TransientShaperClipMode::ALL.into_iter().enumerate() {
        let plain = index as f64;
        let parameter = Parameter::ClipMode;
        assert_eq!(parameter.parse(mode.label()), Ok(plain));
        assert_eq!(parameter.format(plain).unwrap(), mode.label());
        state
            .set_normalized(parameter, parameter.normalize(plain).unwrap())
            .unwrap();
        assert_eq!(state.to_settings().clip_mode, mode);
        let bytes = state.encode();
        assert_eq!(&bytes[..8], b"DAWTRNS1");
        assert_eq!(PluginState::decode(&bytes).unwrap(), state);
        assert!(PluginState::decode(&bytes[..MAX_STATE_BYTES - 1]).is_err());
        for offset in [0, 8, 12] {
            let mut corrupt = bytes;
            corrupt[offset] ^= 0xff;
            assert!(PluginState::decode(&corrupt).is_err());
        }
    }
    assert!(state.set(Parameter::ClipMode, 1.5).is_err());
    assert!(state.set(Parameter::Attack, f64::NAN).is_err());
    let exchange = StateExchange::default();
    exchange.recall(state);
    assert_eq!(exchange.consume(&mut 0), Some(state));
}

#[test]
fn standalone_and_built_in_use_the_same_dsp_at_each_supported_rate() {
    for rate in SUPPORTED_RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut state = PluginState::default();
        let mut plugin = Processor::new(f64::from(rate), 1, 512, state).unwrap();
        let mut dsp = TransientShaper::new(context, &state.to_settings());
        assert_eq!(dsp.latency_frames(), LATENCY_FRAMES);
        assert_eq!(dsp.tail_frames(), u64::from(tail_frames(rate)));
        for parameter in Parameter::ALL {
            let info = parameter.info();
            for value in [info.minimum, info.default, info.maximum] {
                assert!(
                    (parameter
                        .denormalize(parameter.normalize(value).unwrap())
                        .unwrap()
                        - value)
                        .abs()
                        < 1e-8
                );
                assert!(parameter.parse(&parameter.format(value).unwrap()).is_ok());
                state.set(parameter, value).unwrap();
                plugin.apply_parameter(parameter, &state);
                dsp.retarget(&state.to_settings());
                for sample in 0..512 {
                    let input = [(sample as f32 * 0.13).sin() * 1.1, -0.2];
                    assert_eq!(plugin.process_sample(input), dsp.process_sample(input));
                }
            }
        }
    }
}
