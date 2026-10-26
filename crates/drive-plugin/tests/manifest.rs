#[path = "../manifest.rs"]
mod manifest;
#[path = "../src/parameters.rs"]
#[allow(dead_code)]
mod parameters;

const INPUT: &str = include_str!("../../../plugins/drive/unit.toml");

#[test]
fn checked_manifest_generates_the_public_descriptor_and_panel() {
    let output = manifest::generate(INPUT).unwrap();
    assert!(output.contains("io.github.fecabrera.daw.drive"));
    assert!(output.contains("Parameter::Bypass"));
    assert!(output.contains("Control::Shape"));
}

#[test]
fn invalid_manifest_contracts_cannot_build() {
    let invalid = [
        INPUT.replacen("schema_version = 1", "schema_version = 2", 1),
        INPUT.replacen("settings_version = 1", "settings_version = 2", 1),
        INPUT.replacen("state_version = 1", "state_version = 0", 1),
        INPUT.replacen("type_id = \"daw.drive\"", "type_id = \"daw.drive-v2\"", 1),
        INPUT.replace("version = \"0.1.0\"", "version = \"0.2.0\""),
        INPUT.replace("id = 101", "id = 100"),
        INPUT.replace("id = 101", "id = 200"),
        INPUT.replace("key = \"volume\"", "key = \"other\""),
        INPUT.replace("minimum = -1000.0", "minimum = -1001.0"),
        INPUT.replace("maximum = 2000.0", "maximum = 2001.0"),
        INPUT.replace("default = 0.2", "default = 2.0"),
        INPUT.replace("default = 0.2", "default = nan"),
        INPUT.replace("scale = \"logarithmic\"", "scale = \"linear\""),
        INPUT.replace("parameter = \"volume\"", "parameter = \"gain\""),
        INPUT.replace("control = \"gain\"", "control = \"script\""),
        INPUT.replace("diagram = \"drive_color\"", "diagram = \"waveform\""),
        INPUT.replace(", diagram = \"drive_color\"", ""),
        INPUT.replace("enabled_by = \"color_shape\"", "enabled_by = \"bypass\""),
        INPUT.replace("enabled_by = \"color_shape\"", "enabled_by = 0"),
        INPUT.replace(", enabled_by = \"color_shape\"", ""),
        INPUT.replace("footer = \"Drive\"", "footer = \"Other\""),
        INPUT.replace("{ parameter = \"gain\", control = \"percent\" },", ""),
        INPUT.replace(
            "f399b92b5d264c7d98c6de3c2654d534",
            "c212153f328b4d87a659bb681c55f768",
        ),
        INPUT.replace(
            "f399b92b5d264c7d98c6de3c2654d534",
            "00000000000000000000000000000000",
        ),
        INPUT.replace("f399b92b5d264c7d98c6de3c2654d534", "invalid"),
        INPUT.replace("io.github.fecabrera.daw.drive", "invalid id"),
        format!("execute = \"foo\"\n{INPUT}"),
        INPUT.replace(
            "control = \"gain\"",
            "control = \"gain\", execute = \"foo\"",
        ),
    ];
    for (index, input) in invalid.into_iter().enumerate() {
        assert_ne!(input, INPUT, "mutation {index} did not apply");
        assert!(
            manifest::generate(&input).is_err(),
            "mutation {index} was accepted"
        );
    }
}
