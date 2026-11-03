use daw_ambient_reverb::AmbientReverb as Direct;
use daw_ambient_reverb_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");

#[test]
fn legacy_state_loads_six_db_and_new_slopes_roundtrip() {
    let bytes = include_bytes!("fixtures/legacy-state-v1.bin");
    let mut state = PluginState::decode(bytes).unwrap();
    let mut expected = PluginState::default();
    expected.set(P::LowPassEnabled, 0.0).unwrap();
    assert_eq!(state, expected);
    assert_eq!(
        PluginState::read_from(&mut bytes.as_slice()).unwrap(),
        state
    );
    for (i, slope) in daw_unit_core::AmbientHighPassSlope::ALL
        .into_iter()
        .enumerate()
    {
        state.set(P::HighPassSlope, i as f64).unwrap();
        assert_eq!(state.to_settings().highpass_slope, slope);
        assert_eq!(
            PluginState::from_settings(&state.to_settings())
                .unwrap()
                .to_settings(),
            state.to_settings()
        );
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
        assert_eq!(P::HighPassSlope.parse(slope.label()).unwrap(), i as f64);
    }
    assert!(state.set(P::HighPassSlope, 0.5).is_err());
    assert!(state.set(P::HighPassSlope, 3.0).is_err());
}

#[test]
fn ambient_legacy_states_bypass_new_lowpass_and_new_settings_roundtrip() {
    for bytes in [
        include_bytes!("fixtures/legacy-state-v1.bin").as_slice(),
        include_bytes!("fixtures/legacy-state-v2.bin").as_slice(),
    ] {
        let old = PluginState::decode(bytes).unwrap();
        assert!(!old.to_settings().lowpass_enabled);
        assert_eq!(old.to_settings().lowpass_hz, 15000.0);
        assert_eq!(
            old.to_settings().lowpass_slope,
            daw_unit_core::ReverbFilterSlope::Db12
        );
    }
    let mut state = PluginState::default();
    for (i, slope) in daw_unit_core::ReverbFilterSlope::ALL
        .into_iter()
        .enumerate()
    {
        state.set(P::LowPassSlope, i as f64).unwrap();
        state.set(P::LowPass, 500.0).unwrap();
        assert_eq!(state.to_settings().lowpass_slope, slope);
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    }
}
