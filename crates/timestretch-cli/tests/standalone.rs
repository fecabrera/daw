use std::{fs, path::Path, process::Command};

#[test]
fn extracted_manifests_resolve_without_the_daw_workspace() {
    let extracted = tempfile::tempdir().unwrap();
    let cli = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Only manifest resolution is under test; no compilation or downloads occur.
    for (name, source, entry) in [
        ("timestretch", cli.join("../timestretch"), "lib.rs"),
        ("timestretch-cli", cli.to_path_buf(), "main.rs"),
    ] {
        let destination = extracted.path().join(name);
        fs::create_dir_all(destination.join("src")).unwrap();
        fs::copy(source.join("Cargo.toml"), destination.join("Cargo.toml")).unwrap();
        fs::write(destination.join("src").join(entry), "").unwrap();
    }
    for reference_disabled in [false, true] {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(extracted.path()).args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version=1",
            "--manifest-path=timestretch-cli/Cargo.toml",
        ]);
        if reference_disabled {
            command.arg("--no-default-features");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "extracted manifest failed (reference_disabled={reference_disabled}): {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
