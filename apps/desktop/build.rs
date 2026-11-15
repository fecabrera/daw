//! Embed the Explorer icon in the Windows executable using the Windows SDK.
use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../crates/ui/assets/app-icon/DAW.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"))
        .join("../../crates/ui/assets/app-icon/DAW.ico");
    let script = output.join("icon.rc");
    // RC quoted paths accept forward slashes, including paths with spaces.
    std::fs::write(
        &script,
        format!(
            "1 ICON \"{}\"\n",
            icon.display().to_string().replace('\\', "/")
        ),
    )
    .expect("write icon resource script");
    let resource = output.join("icon.res");
    let status = Command::new("rc.exe")
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(&script)
        .status()
        .expect("Windows SDK rc.exe required; use the Visual Studio developer command prompt");
    assert!(status.success(), "Windows icon resource compilation failed");
    println!(
        "cargo:rustc-link-arg-bin=daw-desktop={}",
        resource.display()
    );
}
