#[path = "../../drive-plugin/manifest.rs"]
mod manifest;
#[path = "../src/parameters.rs"]
#[allow(dead_code)]
mod parameters;
const INPUT: &str = include_str!("../../../plugins/saturator/unit.toml");
#[test]
fn manifest_enforces_identity_parameters_and_panel_bindings() {
    assert!(
        manifest::generate(INPUT)
            .unwrap()
            .contains("io.github.fecabrera.daw.saturator")
    );
    for (from, to) in [
        ("schema_version = 1", "schema_version = 99"),
        ("type_id = \"daw.saturator\"", "type_id = \"unknown\""),
        ("id = 101", "id = 100"),
        ("default = 0.0", "default = nan"),
        ("minimum = -1000.0", "minimum = -1001.0"),
        ("parameter = \"level\"", "parameter = \"drive\""),
        ("control = \"gain\"", "control = \"script\""),
        (
            "control = \"gain\"",
            "control = \"gain\", enabled_by = \"bypass\"",
        ),
    ] {
        let invalid = INPUT.replacen(from, to, 1);
        assert_ne!(invalid, INPUT);
        assert!(manifest::generate(&invalid).is_err());
    }
    assert!(manifest::generate(&format!("execute = \"script\"\n{INPUT}")).is_err());
}
