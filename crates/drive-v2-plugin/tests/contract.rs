use daw_drive_v2_plugin::{IDENTITY, MAX_STATE_BYTES, PANEL, Parameter as P, PluginState};
use daw_unit_core::{DriveV2Clip, DriveV2ColorShape, DriveV2Settings, SILENT_GAIN_DB};
#[test]
fn frozen_identity_default_state_and_daw_settings() {
    let identity: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/identity.json")).unwrap();
    assert_eq!(IDENTITY.product, identity["product"]);
    assert_eq!(IDENTITY.clap_id, identity["clap_id"]);
    let hex = |b: [u8; 16]| b.map(|v| format!("{v:02x}")).join("");
    assert_eq!(hex(IDENTITY.processor_id), identity["processor_id"]);
    assert_eq!(hex(IDENTITY.controller_id), identity["controller_id"]);
    assert_eq!(
        P::ALL.map(|p| p as u32),
        [100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110]
    );
    assert_eq!(
        PluginState::default().to_settings(),
        DriveV2Settings::default()
    );
    let bytes: Vec<_> = include_str!("fixtures/default.hex")
        .split_whitespace()
        .map(|v| u8::from_str_radix(v, 16).unwrap())
        .collect();
    assert_eq!(PluginState::default().encode().as_slice(), bytes);
    assert_eq!(PluginState::decode(&bytes).unwrap(), PluginState::default());
    assert_eq!(MAX_STATE_BYTES, 104);
    assert_eq!(PANEL.len(), 3);
    assert_eq!(PANEL[1].power, Some(P::ColorEnabled));
}
#[test]
fn v2_settings_roundtrip_retains_power_choices_silence_and_mix() {
    let s = DriveV2Settings {
        color_enabled: false,
        color_shape: DriveV2ColorShape::MidHumpBassCut,
        clip: DriveV2Clip::Hard,
        input_gain_db: SILENT_GAIN_DB,
        level_db: 6.0,
        dry_wet: 0.37,
        ..Default::default()
    };
    let state = PluginState::from_settings(&s).unwrap();
    assert_eq!(state.to_settings(), s);
    for p in [P::ColorShape, P::ColorStrength, P::ColorFrequency] {
        assert!(!state.control_enabled(p));
    }
    assert_eq!(P::InputGain.normalize(f64::from(SILENT_GAIN_DB)), Ok(0.0));
    assert_eq!(P::InputGain.parse("-inf dB"), Ok(f64::from(SILENT_GAIN_DB)));
    assert_eq!(P::Clip.parse("Hard"), Ok(1.0));
    assert_eq!(P::ColorEnabled.parse("Off"), Ok(0.0));
    assert_eq!(P::DryWet.parse("37%"), Ok(0.37));
}
#[test]
fn every_parameter_roundtrips_and_rejects_invalid_values() {
    for p in P::ALL {
        let info = p.info();
        for value in [info.minimum, info.maximum, info.default] {
            let result = p.denormalize(p.normalize(value).unwrap()).unwrap();
            assert!((value - result).abs() < 1e-9, "{p:?}: {value} vs {result}");
            assert!(p.parse(&p.format(value).unwrap()).is_ok());
        }
        for value in [
            f64::NAN,
            f64::INFINITY,
            info.maximum + 1.0,
            info.minimum - 1.0,
        ] {
            assert!(p.validate(value).is_err());
        }
        if info.steps > 0 {
            assert!(p.validate(0.5).is_err());
        }
    }
}
#[test]
fn malformed_foreign_and_future_state_do_not_replace_working_state() {
    let mut working = PluginState::default();
    working.set(P::InputGain, 6.0).unwrap();
    let original = working;
    let bytes = original.encode();
    for index in [0, 8, 12] {
        let mut invalid = bytes;
        invalid[index] ^= 0xff;
        assert!(working.restore(&invalid).is_err());
        assert_eq!(working, original);
    }
    for p in P::ALL {
        let mut invalid = bytes;
        let start = 16 + 8 * (p as usize - 100);
        invalid[start..start + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(working.restore(&invalid).is_err());
        assert_eq!(working, original);
    }
    assert!(working.restore(&bytes[..103]).is_err());
    let mut too_long = bytes.to_vec();
    too_long.push(0);
    assert!(working.restore(&too_long).is_err());
    let mut output = Vec::new();
    working.write_to(&mut output).unwrap();
    assert_eq!(
        PluginState::read_from(&mut output.as_slice()).unwrap(),
        original
    );
}

use daw_drive_v2_plugin::{Parameter as CopyParameter, PluginState as CopyState, Processor};
#[test]
fn timed_parameter_application_copies_only_the_selected_validated_value() {
    let mut processor = Processor::new(48000.0, 1, 512, CopyState::default()).unwrap();
    let mut source = CopyState::default();
    for parameter in CopyParameter::ALL {
        source.set(parameter, parameter.info().maximum).unwrap();
    }
    let mut expected = CopyState::default();
    for parameter in CopyParameter::ALL.into_iter().rev() {
        expected.set(parameter, source.get(parameter)).unwrap();
        processor.apply_parameter(parameter, &source);
        assert_eq!(processor.state(), expected);
        assert!(processor.state().to_settings().valid());
    }
}
