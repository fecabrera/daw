//! Catalog inspection must not require a native plugin, display or audio device.
use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-cli-plugins-{}", daw_core::Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli(scratch: &Scratch, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_daw-cli"))
        .arg("plugins")
        .arg("--cache")
        .arg(scratch.0.join("registry.json"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn cached_empty_registry_never_creates_a_cache_or_needs_a_scanner() {
    let scratch = Scratch::new();
    let result = cli(
        &scratch,
        &["--cached", "--json", "--scanner", "missing-scanner"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["entries"], serde_json::json!([]));
    assert!(!scratch.0.join("registry.json").exists());
}

#[test]
fn corrupt_cached_data_fails_without_replacing_user_data() {
    let scratch = Scratch::new();
    let path = scratch.0.join("registry.json");
    fs::write(&path, b"corrupt").unwrap();
    let result = cli(&scratch, &["--cached"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(&path).unwrap(), b"corrupt");
    assert!(result.stdout.is_empty());
    assert_eq!(
        cli(&scratch, &["--cached", "--retry-failed"]).status.code(),
        Some(2)
    );
}

#[test]
fn cached_catalog_shows_shadowing_and_quarantine_without_loading_modules() {
    let scratch = Scratch::new();
    let plugin = serde_json::json!({"identity":{"format":"clap","id":"org.test.plugin"},"name":"Test Plugin","vendor":"Test","version":"1","role":"effect"});
    let entry = |path: &str, state: serde_json::Value| serde_json::json!({"candidate":{"format":"clap","path":path,"executable":path,"fingerprint":"a".repeat(64)},"plugins":[plugin.clone()],"state":state,"present":true});
    let catalog = serde_json::json!({"version":1,"entries":[entry("missing-user.clap",serde_json::json!({"status":"ready"})),entry("missing-system.clap",serde_json::json!({"status":"ready"})),entry("failed.clap",serde_json::json!({"status":"quarantined","reason":"previous scan failed"}))]});
    fs::write(
        scratch.0.join("registry.json"),
        serde_json::to_vec(&catalog).unwrap(),
    )
    .unwrap();
    let result = cli(&scratch, &["--cached"]);
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("Test Plugin\tavailable\tmissing-user.clap"));
    assert!(text.contains("Test Plugin\tshadowed\tmissing-system.clap"));
    assert!(text.contains("Unavailable\tfailed.clap\tprevious scan failed"));
}
