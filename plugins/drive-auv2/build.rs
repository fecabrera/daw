#[cfg(target_os = "macos")]
fn main() {
    use std::{env, path::PathBuf, process::Command};
    // Build scripts run on the host; select native AU work from Cargo's target.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }
    let directory = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let package = env::var("CARGO_PKG_NAME").expect("package name");
    let slug = package
        .strip_prefix("daw-")
        .and_then(|s| s.strip_suffix("-auv2"))
        .expect("AU package name");
    let manifest = directory.join("..").join(slug).join("unit.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let document = std::fs::read_to_string(manifest)
        .expect("product manifest")
        .parse::<toml_edit::DocumentMut>()
        .expect("product TOML");
    let name = document["auv2"]["cocoa_class"]
        .as_str()
        .expect("Cocoa factory class");
    assert!(
        !name.is_empty()
            && !name.as_bytes()[0].is_ascii_digit()
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "invalid Cocoa identifier"
    );
    let clang = Command::new("xcrun")
        .args(["--sdk", "macosx", "--find", "clang"])
        .output()
        .expect("Apple clang");
    assert!(clang.status.success());
    let clang = String::from_utf8(clang.stdout)
        .expect("clang path")
        .trim()
        .to_owned();
    let sdk = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("Apple SDK");
    assert!(sdk.status.success());
    let sdk = String::from_utf8(sdk.stdout)
        .expect("SDK path")
        .trim()
        .to_owned();
    let source = directory.join("../../crates/plugin-auv2/cocoa.m");
    println!("cargo:rerun-if-changed={}", source.display());
    cc::Build::new()
        .compiler(clang)
        .file(source)
        .flag("-isysroot")
        .flag(sdk)
        .flag("-mmacosx-version-min=12.0")
        .flag("-fno-objc-arc")
        .define("DAW_AU_FACTORY", Some(name))
        .define(
            "DAW_AU_CONTAINER",
            Some(format!("{name}Container").as_str()),
        )
        .define("DAW_AU_TIMER", Some(format!("{name}Timer").as_str()))
        .compile("daw_au_cocoa");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-arg=-mmacosx-version-min=12.0");
}
#[cfg(not(target_os = "macos"))]
fn main() {}
