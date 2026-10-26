use daw_drive_plugin::{
    Control, IDENTITY, MANIFEST_SCHEMA_VERSION, MAX_STATE_BYTES, PANEL, PARAMETER_ORDER,
    Parameter as P, PluginState, STATE_VERSION, UNIT_TYPE_ID,
};
use daw_unit_core::{DriveColorShape, DriveSettings, SILENT_GAIN_DB};
use std::io::{self, Read, Write};

fn fixture(name: &str) -> Vec<u8> {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}.hex",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    text.split_whitespace()
        .map(|word| u8::from_str_radix(word, 16).unwrap())
        .collect()
}

#[test]
fn frozen_identity_and_wire_contract() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/identity.json")).unwrap();
    assert_eq!(IDENTITY.vendor, golden["vendor"]);
    assert_eq!(IDENTITY.product, golden["product"]);
    assert_eq!(IDENTITY.clap_id, golden["clap_id"]);
    let hex = |bytes: [u8; 16]| bytes.map(|b| format!("{b:02x}")).join("");
    assert_eq!(hex(IDENTITY.processor_id), golden["processor_id"]);
    assert_eq!(hex(IDENTITY.controller_id), golden["controller_id"]);
    assert_ne!(IDENTITY.processor_id, IDENTITY.controller_id);
    assert_eq!(IDENTITY.version, "0.1.0");
    assert_eq!(UNIT_TYPE_ID, "daw.drive");
    assert_eq!(MANIFEST_SCHEMA_VERSION, 1);
    assert_eq!(STATE_VERSION, 1);
    assert_eq!(
        PARAMETER_ORDER.map(|p| p as u32),
        [100, 101, 102, 103, 104, 105, 106]
    );
    assert_eq!(P::ALL, PARAMETER_ORDER);
    assert!(P::from_id(99).is_err());
    assert!(P::from_id(u32::MAX).is_err());
    assert_eq!(
        PluginState::default().encode().as_slice(),
        fixture("default-v1")
    );
}

#[test]
fn default_parameters_and_settings_match_the_builtin_processor() {
    let state = PluginState::default();
    assert_eq!(state.to_settings(), DriveSettings::default());
    for parameter in P::ALL {
        let info = parameter.info();
        assert_eq!(state.get(parameter), info.default);
        assert_eq!(parameter.denormalize(0.0).unwrap(), info.minimum);
        assert_eq!(parameter.denormalize(1.0).unwrap(), info.maximum);
        assert_eq!(parameter.normalize(info.minimum).unwrap(), 0.0);
        assert_eq!(parameter.normalize(info.maximum).unwrap(), 1.0);
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            info.minimum - 1.0,
            info.maximum + 1.0,
        ] {
            assert!(parameter.validate(value).is_err());
        }
        for value in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            assert!(parameter.denormalize(value).is_err());
        }
    }
}

#[test]
fn continuous_curves_are_monotonic_reversible_and_keep_unity_centered() {
    for parameter in [
        P::Volume,
        P::Gain,
        P::Tone,
        P::ColorStrength,
        P::ColorFrequency,
    ] {
        let mut previous = parameter.info().minimum;
        for index in 0..=10000 {
            let normalized = index as f64 / 10000.0;
            let plain = parameter.denormalize(normalized).unwrap();
            assert!(plain >= previous);
            assert!((parameter.normalize(plain).unwrap() - normalized).abs() < 1e-13);
            previous = plain;
        }
    }
    assert_eq!(P::Volume.normalize(0.0).unwrap(), 0.5);
    assert!(
        (P::ColorFrequency.denormalize(0.5).unwrap() - (250.0_f64 * 2000.0).sqrt()).abs() < 1e-10
    );
}

#[test]
fn low_finite_db_values_and_state_roundtrips_do_not_become_the_silence_sentinel() {
    let mut state = PluginState::default();
    for db in [
        -999.9999999999,
        -999.99,
        -950.0,
        -900.0,
        -300.0,
        -90.0,
        -60.0,
        0.0,
        12.0,
    ] {
        state.set(P::Volume, db).unwrap();
        let normalized = P::Volume.normalize(db).unwrap();
        assert!(normalized > 0.0);
        assert!((P::Volume.denormalize(normalized).unwrap() - db).abs() < 1e-10);
        assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
        assert!(state.to_settings().volume_db > SILENT_GAIN_DB);
        let text = P::Volume.format(db).unwrap();
        assert!(!text.contains('∞'));
        assert!(P::Volume.parse(&text).unwrap() > f64::from(SILENT_GAIN_DB));
        assert!((f64::from(state.to_settings().volume_db) - db).abs() <= 0.000062);
    }
    state.set(P::Volume, f64::from(SILENT_GAIN_DB)).unwrap();
    assert_eq!(P::Volume.normalize(state.get(P::Volume)).unwrap(), 0.0);
    assert_eq!(state.to_settings().volume_db, SILENT_GAIN_DB);
    assert_eq!(P::Volume.format(state.get(P::Volume)).unwrap(), "−∞ dB");
    state.set_normalized(P::Volume, 1e-300).unwrap();
    assert!(state.to_settings().volume_db > SILENT_GAIN_DB);
}

#[test]
fn shape_order_discrete_thresholds_and_off_values_are_retained() {
    let mut state = PluginState::decode(&fixture("color-off-v1")).unwrap();
    assert_eq!(state.get(P::ColorShape), 0.0);
    assert_eq!(state.get(P::ColorStrength), 0.875);
    assert_eq!(state.get(P::ColorFrequency), 1234.5);
    assert!(!state.control_enabled(P::ColorStrength));
    assert!(!state.control_enabled(P::ColorFrequency));
    state.set(P::ColorStrength, 0.9375).unwrap();
    state.set(P::ColorFrequency, 1750.25).unwrap();
    for (index, name) in ["Off", "Mid scoop", "Mid hump", "Mid hump + bass cut"]
        .into_iter()
        .enumerate()
    {
        state.set(P::ColorShape, index as f64).unwrap();
        assert_eq!(P::ColorShape.format(index as f64).unwrap(), name);
        assert_eq!(state.to_settings().color_shape, DriveColorShape::ALL[index]);
        assert_eq!(state.get(P::ColorStrength), 0.9375);
        assert_eq!(state.get(P::ColorFrequency), 1750.25);
    }
    assert!(P::ColorShape.validate(1.5).is_err());
    assert!(P::Bypass.validate(0.5).is_err());
    assert_eq!(P::ColorShape.denormalize(0.499).unwrap(), 1.0);
    assert_eq!(P::ColorShape.denormalize(0.5).unwrap(), 2.0);
    assert_eq!(P::Bypass.denormalize(0.499).unwrap(), 0.0);
    assert_eq!(P::Bypass.denormalize(0.5).unwrap(), 1.0);
    assert_eq!(PANEL[0].footer, "Drive");
    assert_eq!(PANEL[1].footer, "Color");
    assert_eq!(PANEL[1].controls[0].control, Control::Shape);
    assert_eq!(PANEL[1].controls[0].diagram, Some("drive_color"));
    assert_eq!(PANEL[1].controls[1].enabled_by, Some(P::ColorShape));
    assert_eq!(
        PANEL.map(|s| s.controls.map(|c| c.parameter)),
        [
            [P::Volume, P::Gain, P::Tone],
            [P::ColorShape, P::ColorStrength, P::ColorFrequency]
        ]
    );
}

#[test]
fn text_units_choices_and_invalid_input() {
    for (parameter, text, expected) in [
        (P::Volume, "-inf", -1000.0),
        (P::Volume, "−∞ dB", -1000.0),
        (P::Volume, " -950 dB ", -950.0),
        (P::Volume, "+12", 12.0),
        (P::Gain, "20%", 0.2),
        (P::Tone, "50", 0.5),
        (P::ColorFrequency, "1.5 kHz", 1500.0),
        (P::ColorFrequency, "750 Hz", 750.0),
        (P::ColorShape, "Mid hump + bass cut", 3.0),
        (P::ColorShape, "1", 1.0),
        (P::Bypass, "On", 1.0),
    ] {
        assert_eq!(parameter.parse(text).unwrap(), expected);
    }
    for parameter in P::ALL {
        for text in [
            "",
            "NaN",
            "inf",
            "∞",
            "-infinity Hz",
            "12 frogs",
            "1 2",
            "1%Hz",
        ] {
            assert!(parameter.parse(text).is_err(), "{parameter:?}: {text}");
        }
    }
    for (parameter, text) in [
        (P::Gain, "100.1%"),
        (P::Volume, "12.1 dB"),
        (P::ColorShape, "1.5"),
        (P::Bypass, "0.5"),
        (P::ColorFrequency, "249 Hz"),
    ] {
        assert!(parameter.parse(text).is_err());
    }
}

#[test]
fn settings_projection_and_independent_instances_preserve_only_base_values() {
    let settings = DriveSettings {
        collapsed: true,
        modulation_open: true,
        enabled: false,
        ..DriveSettings::default()
    };
    let state = PluginState::from_settings(&settings).unwrap();
    let projected = state.to_settings();
    assert!(!projected.enabled);
    assert!(!projected.collapsed && !projected.modulation_open && projected.modulations.is_empty());
    let mut duplicate = state;
    duplicate.set(P::Gain, 0.99).unwrap();
    assert_ne!(duplicate, state);
    assert_eq!(PluginState::decode(&state.encode()).unwrap(), state);
    let invalid = DriveSettings {
        version: 2,
        ..settings
    };
    assert!(PluginState::from_settings(&invalid).is_err());
}

#[test]
fn malformed_state_never_changes_the_current_snapshot() {
    let good = fixture("color-off-v1");
    let mut current = PluginState::decode(&good).unwrap();
    let old = current;
    let mut bad = Vec::new();
    for length in 0..MAX_STATE_BYTES {
        bad.push(good[..length].to_vec());
    }
    let mut trailing = good.clone();
    trailing.push(0);
    bad.push(trailing);
    for offset in [0, 8, 12] {
        let mut bytes = good.clone();
        bytes[offset] ^= 0xff;
        bad.push(bytes);
    }
    for version in [0_u32, 2, u32::MAX] {
        let mut bytes = good.clone();
        bytes[8..12].copy_from_slice(&version.to_le_bytes());
        bad.push(bytes);
    }
    for parameter in P::ALL {
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            parameter.info().minimum - 1.0,
            parameter.info().maximum + 1.0,
        ] {
            let mut bytes = good.clone();
            let start = 16 + (parameter as usize - 100) * 8;
            bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
            bad.push(bytes);
        }
    }
    for bytes in bad {
        assert!(current.restore(&bytes).is_err());
        assert_eq!(current, old);
    }
}

struct Tiny {
    bytes: Vec<u8>,
    offset: usize,
    interrupt: bool,
}
impl Read for Tiny {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.interrupt = !self.interrupt;
        if self.interrupt {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.offset == self.bytes.len() {
            return Ok(0);
        }
        buffer[0] = self.bytes[self.offset];
        self.offset += 1;
        Ok(1)
    }
}
impl Write for Tiny {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.interrupt = !self.interrupt;
        if self.interrupt {
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.bytes.push(buffer[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn short_streams_interruptions_truncation_and_trailing_data() {
    let state = PluginState::decode(&fixture("color-off-v1")).unwrap();
    let mut stream = Tiny {
        bytes: vec![],
        offset: 0,
        interrupt: false,
    };
    state.write_to(&mut stream).unwrap();
    assert_eq!(stream.bytes, state.encode());
    stream.interrupt = false;
    assert_eq!(PluginState::read_from(&mut stream).unwrap(), state);
    let mut truncated = &state.encode()[..71];
    assert!(PluginState::read_from(&mut truncated).is_err());
    let mut oversized = state.encode().to_vec();
    oversized.resize(4096, 0);
    let mut input = oversized.as_slice();
    assert!(PluginState::read_from(&mut input).is_err());
    assert_eq!(input.len(), 4096 - 73);
    assert!(state.write_to(&mut io::sink()).is_ok());
    assert!(state.write_to(&mut [].as_mut_slice()).is_err());
}

#[test]
fn frozen_plain_normalized_and_text_fixtures() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/parameters-v1.json")).unwrap();
    for row in fixtures.as_array().unwrap() {
        let parameter = P::from_id(row["id"].as_u64().unwrap() as u32).unwrap();
        let plain = row["plain"].as_f64().unwrap();
        let normalized = row["normalized"].as_f64().unwrap();
        assert_eq!(parameter.info().key, row["key"]);
        let actual = parameter.normalize(plain).unwrap();
        if normalized == 0.0 {
            assert_eq!(actual, 0.0);
        } else {
            assert!(((actual - normalized) / normalized).abs() < 1e-12);
        }
        assert!((parameter.denormalize(normalized).unwrap() - plain).abs() < 1e-10);
        assert_eq!(parameter.format(plain).unwrap(), row["text"]);
        assert!((parameter.parse(row["text"].as_str().unwrap()).unwrap() - plain).abs() < 1e-10);
    }
}

struct Interrupted;
impl Read for Interrupted {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Interrupted.into())
    }
}
impl Write for Interrupted {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Interrupted.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn infinite_interruption_and_io_errors_return_without_changing_state() {
    let state = PluginState::default();
    assert!(PluginState::read_from(&mut Interrupted).is_err());
    assert!(state.write_to(&mut Interrupted).is_err());
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Other.into())
        }
    }
    assert!(PluginState::read_from(&mut Broken).is_err());
    assert_eq!(state, PluginState::default());
}

#[test]
fn portable_rounding_stays_within_documented_parameter_tolerances() {
    let mut state = PluginState::default();
    for index in 0..=10000 {
        let value = index as f64 / 10000.0;
        state.set(P::Gain, value).unwrap();
        state.set(P::Tone, value).unwrap();
        state.set(P::ColorStrength, value).unwrap();
        state.set_normalized(P::ColorFrequency, value).unwrap();
        state.set(P::Volume, -1000.0 + 1012.0 * value).unwrap();
        let settings = state.to_settings();
        assert!(settings.valid());
        assert!((f64::from(settings.gain) - value).abs() <= 6e-8);
        assert!((f64::from(settings.tone) - value).abs() <= 6e-8);
        assert!((f64::from(settings.color_strength) - value).abs() <= 6e-8);
        assert!(
            (f64::from(settings.color_frequency_hz) - state.get(P::ColorFrequency)).abs()
                <= 0.000123
        );
        assert!((f64::from(settings.volume_db) - state.get(P::Volume)).abs() <= 0.000062);
    }
}

use daw_drive_plugin::{Parameter as CopyParameter, PluginState as CopyState, Processor};
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
