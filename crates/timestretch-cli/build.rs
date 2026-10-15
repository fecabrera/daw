use std::process::Command;
fn main() {
    for key in ["TARGET", "PROFILE"] {
        println!(
            "cargo:rustc-env=BUILD_{key}={}",
            std::env::var(key).unwrap_or_default()
        );
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    println!(
        "cargo:rustc-env=BUILD_REVISION={}",
        git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unavailable".into())
    );
    println!(
        "cargo:rustc-env=BUILD_DIRTY={}",
        git(&["status", "--porcelain"])
            .map(|s| (!s.is_empty()).to_string())
            .unwrap_or_else(|| "unavailable".into())
    );
    // Watch Git identity when available; extracted source archives report unavailable.
    for reference in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse", "--git-path", reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
    // Source archives have no Git identity; source changes still rebuild this record.
    println!("cargo:rerun-if-env-changed=TIMESTRETCH_BUILD_ID");
    println!("cargo:rerun-if-changed=src");
}
