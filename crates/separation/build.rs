//! Native prerequisites of the statically linked CoreML backend.
use std::{env, error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return Ok(());
    }

    // CoreML's Objective-C availability checks reference compiler-rt. Rust's
    // -nodefaultlibs link does not add Clang's runtime automatically, and thin
    // LTO can expose the missing symbol even when debug binaries link.
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "clang", "--print-resource-dir"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Cannot locate Apple Clang's runtime: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let directory = PathBuf::from(String::from_utf8(output.stdout)?.trim()).join("lib/darwin");
    let archive = directory.join("libclang_rt.osx.a");
    if !archive.is_file() {
        return Err(format!("Apple Clang runtime is missing: {}", archive.display()).into());
    }
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
    Ok(())
}
