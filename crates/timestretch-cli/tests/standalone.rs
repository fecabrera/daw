use std::{fs, path::Path, process::Command};

#[test]
fn extracted_manifests_resolve_without_the_daw_workspace() {
    let extracted = tempfile::tempdir().unwrap();
    let cli = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Resolve path dependencies too; --no-deps would miss workspace inheritance.
    for (name, source, entry) in [
        ("audio-analysis", cli.join("../audio-analysis"), "lib.rs"),
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

fn copy_sources(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_sources(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn external_consumer_builds_and_processes_without_daw_or_native_dependencies() {
    let extracted = tempfile::tempdir().unwrap();
    let cli = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in ["timestretch", "audio-analysis"] {
        let source = cli.join("..").join(name);
        let destination = extracted.path().join(name);
        copy_sources(&source.join("src"), &destination.join("src"));
        fs::copy(source.join("Cargo.toml"), destination.join("Cargo.toml")).unwrap();
    }
    let consumer = extracted.path().join("consumer");
    fs::create_dir_all(consumer.join("src")).unwrap();
    fs::write(consumer.join("Cargo.toml"), "[package]\nname='external-consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\ntimestretch={path='../timestretch'}\n").unwrap();
    fs::write(
        consumer.join("src/main.rs"),
        r#"
fn main() {
    let input = vec![0.25_f32; 4800];
    let output = timestretch::Request::new(48000, 1, timestretch::Ratio::new(1, 1).unwrap())
        .process(&input).unwrap();
    assert_eq!(input, output);
}
"#,
    )
    .unwrap();
    // A separate target avoids the outer test runner's Cargo build lock.
    let result = Command::new(env!("CARGO"))
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", extracted.path().join("target"))
        .args(["run", "--offline", "--quiet"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "external consumer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
