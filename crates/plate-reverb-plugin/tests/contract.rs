use daw_plate_reverb::PlateReverb as Direct;
use daw_plate_reverb_plugin as plugin;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");
include!("../../filter-plugin/tests/common/contract.rs");

#[test]
fn legacy_state_keeps_filters_on_and_independent_power_roundtrips() {
    let bytes = include_bytes!("fixtures/legacy-state-v1.bin");
    let mut state = PluginState::decode(bytes).unwrap();
    let mut expected = PluginState::default();
    expected.set(P::Drive, 0.0).unwrap();
    assert_eq!(state, expected);
    assert_eq!(
        PluginState::read_from(&mut bytes.as_slice()).unwrap(),
        state
    );
    for high in [0.0, 1.0] {
        for low in [0.0, 1.0] {
            state.set(P::HighPassEnabled, high).unwrap();
            state.set(P::LowPassEnabled, low).unwrap();
            assert_eq!(state.to_settings().highpass_enabled, high != 0.0);
            assert_eq!(state.to_settings().lowpass_enabled, low != 0.0);
            assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
            assert_eq!(
                PluginState::from_settings(&state.to_settings())
                    .unwrap()
                    .to_settings(),
                state.to_settings()
            );
        }
    }
}

#[test]
fn v2_preserves_power_and_defaults_slopes_to_twelve_and_v3_roundtrips_choices() {
    let mut bytes = *include_bytes!("fixtures/legacy-state-v2.bin");
    bytes[16 + 13 * 8..16 + 14 * 8].copy_from_slice(&0.0_f64.to_le_bytes());
    let mut state = PluginState::decode(&bytes).unwrap();
    assert!(!state.to_settings().highpass_enabled);
    assert!(state.to_settings().lowpass_enabled);
    assert_eq!(state.get(P::HighPassSlope), 1.0);
    assert_eq!(state.get(P::LowPassSlope), 1.0);
    assert_eq!(
        PluginState::read_from(&mut bytes.as_slice()).unwrap(),
        state
    );
    for (i, high) in daw_unit_core::ReverbFilterSlope::ALL
        .into_iter()
        .enumerate()
    {
        for (j, low) in daw_unit_core::ReverbFilterSlope::ALL
            .into_iter()
            .enumerate()
        {
            state.set(P::HighPassSlope, i as f64).unwrap();
            state.set(P::LowPassSlope, j as f64).unwrap();
            assert_eq!(state.to_settings().highpass_slope, high);
            assert_eq!(state.to_settings().lowpass_slope, low);
            assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
            assert_eq!(P::HighPassSlope.parse(high.label()).unwrap(), i as f64);
            assert_eq!(P::LowPassSlope.parse(low.label()).unwrap(), j as f64);
        }
    }
}

#[test]
fn v3_state_preserves_slopes_and_power_and_defaults_drive_to_soft() {
    let mut bytes = *include_bytes!("fixtures/legacy-state-v3.bin");
    bytes[16 + 11 * 8..16 + 12 * 8].copy_from_slice(&1.0_f64.to_le_bytes());
    bytes[16 + 15 * 8..16 + 16 * 8].copy_from_slice(&2.0_f64.to_le_bytes());
    let mut state = PluginState::decode(&bytes).unwrap();
    assert!(state.to_settings().drive_enabled);
    assert_eq!(
        state.to_settings().highpass_slope,
        daw_unit_core::ReverbFilterSlope::Db24
    );
    assert_eq!(
        state.to_settings().drive_mode,
        daw_unit_core::PlateDriveMode::Soft
    );
    assert_eq!(
        PluginState::read_from(&mut bytes.as_slice()).unwrap(),
        state
    );
    for (i, mode) in daw_unit_core::PlateDriveMode::ALL.into_iter().enumerate() {
        state.set(P::DriveMode, i as f64).unwrap();
        assert_eq!(state.to_settings().drive_mode, mode);
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
        assert_eq!(P::DriveMode.parse(mode.label()).unwrap(), i as f64);
    }
    assert!(state.set(P::DriveMode, 0.5).is_err());
    assert!(state.set(P::DriveMode, 3.0).is_err());
}

#[test]
fn percent_drive_defaults_and_legacy_negative_gain_and_neutral_tone_migrate() {
    assert_eq!(P::Drive.info().default, 2.4);
    assert_eq!(P::Drive.format(2.4).unwrap(), "20.00 %");
    assert!((P::Drive.parse("20%").unwrap() - 2.4).abs() < 1e-12);
    assert_eq!(P::Drive.parse("100%").unwrap(), 12.0);
    assert!(P::Drive.validate(-1.0).is_err());
    let mut bytes = *include_bytes!("fixtures/legacy-state-v4.bin");
    bytes[16 + 5 * 8..16 + 6 * 8].copy_from_slice(&(-6.0_f64).to_le_bytes());
    let migrated = PluginState::decode(&bytes).unwrap();
    assert_eq!(migrated.get(P::Drive), 0.0);
    assert_eq!(migrated.get(P::Tone), 0.5);
    for tone in [0.0, 0.5, 1.0] {
        let mut state = migrated;
        state.set(P::Tone, tone).unwrap();
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
        assert_eq!(state.to_settings().drive_tone, tone as f32);
    }
}
