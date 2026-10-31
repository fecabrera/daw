//! Build compatibility for disposable derived audio; no source paths are persisted.
use std::process::Command;
fn main() {
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = Command::new(compiler)
        .arg("-Vv")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).replace(['\n', '\r'], "|"))
        .unwrap_or_default();
    let mut identity = version;
    for name in [
        "TARGET",
        "PROFILE",
        "CARGO_ENCODED_RUSTFLAGS",
        "CFLAGS",
        "CXXFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        identity.push_str(&format!(
            "|{name}={}",
            std::env::var(name).unwrap_or_default()
        ));
    }
    println!("cargo:rustc-env=DAW_MEDIA_BUILD={identity}");
    println!("cargo:rerun-if-changed=build.rs");
}
