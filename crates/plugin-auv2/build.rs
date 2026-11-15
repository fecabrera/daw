#[cfg(target_os = "macos")]
fn main() {
    use std::{env, path::PathBuf, process::Command};
    // Build scripts run on the host; select native AU work from Cargo's target.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("xcrun SDK path");
    assert!(
        output.status.success(),
        "Apple macOS SDK is required for AUv2 exports"
    );
    let sdk = String::from_utf8(output.stdout)
        .expect("SDK path UTF-8")
        .trim()
        .to_owned();
    let target = env::var("TARGET").expect("Cargo TARGET");
    let clang_target = "arm64-apple-macos12.0";
    use sha2::{Digest, Sha256};
    use std::sync::{Arc, Mutex};
    #[derive(Debug)]
    struct Includes(Arc<Mutex<Vec<String>>>);
    impl bindgen::callbacks::ParseCallbacks for Includes {
        fn include_file(&self, path: &str) {
            self.0.lock().expect("header inventory").push(path.into());
        }
    }
    let includes = Arc::new(Mutex::new(Vec::new()));
    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(Includes(includes.clone())))
        .clang_args(["-isysroot", &sdk, "-target", clang_target])
        .generate_comments(false)
        .layout_tests(false)
        .derive_debug(false);
    println!("cargo:rerun-if-changed=symbols.txt");
    for line in include_str!("symbols.txt").lines() {
        let (kind, name) = line.split_once(' ').expect("symbol inventory");
        let exact = format!("^{name}$");
        builder = match kind {
            "type" => builder.allowlist_type(exact),
            "function" => builder.allowlist_function(exact),
            "variable" => builder.allowlist_var(exact),
            _ => panic!("invalid symbol inventory"),
        };
    }
    let bindings = builder
        .generate()
        .expect("generate Apple C ABI with libclang");
    let declarations = bindings.to_string();
    for line in include_str!("symbols.txt").lines() {
        let (kind, name) = line.split_once(' ').expect("symbol inventory");
        let present = match kind {
            "type" => ["type", "struct", "union", "enum"]
                .iter()
                .any(|kind| declarations.contains(&format!("pub {kind} {name} "))),
            "function" => declarations.contains(&format!("pub fn {name}(")),
            "variable" => ["const", "static", "static mut"]
                .iter()
                .any(|kind| declarations.contains(&format!("pub {kind} {name}:"))),
            _ => false,
        };
        assert!(present, "required AU C symbol missing from SDK: {name}");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("write local bindings");
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let sdk_version = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-version"])
        .output()
        .expect("SDK version");
    assert!(sdk_version.status.success(), "read SDK version");
    let sdk_version = String::from_utf8(sdk_version.stdout)
        .expect("SDK version UTF-8")
        .trim()
        .to_owned();
    let clang = bindgen::clang_version().full;
    let compiler = Command::new("xcrun")
        .args(["--sdk", "macosx", "--find", "clang"])
        .output()
        .expect("Apple clang path");
    assert!(compiler.status.success(), "Apple clang is required");
    let compiler = String::from_utf8(compiler.stdout)
        .expect("clang path UTF-8")
        .trim()
        .to_owned();
    let compiler_version = Command::new(&compiler)
        .arg("--version")
        .output()
        .expect("Apple clang version");
    let compiler_version = String::from_utf8(compiler_version.stdout)
        .expect("clang version UTF-8")
        .lines()
        .next()
        .expect("clang identity")
        .to_owned();
    assert_eq!(
        compiler_version, clang,
        "C ABI compiler and libclang must match"
    );
    let receipt = Command::new("pkgutil")
        .args(["--pkg-info", "com.apple.pkg.CLTools_Executables"])
        .output()
        .expect("CLTools receipt");
    let receipt = String::from_utf8(receipt.stdout).expect("CLTools receipt UTF-8");
    let receipt = receipt
        .lines()
        .find_map(|l| l.strip_prefix("version: "))
        .expect("CLTools receipt version");
    let mut headers = std::collections::BTreeMap::new();
    for header in includes.lock().expect("header inventory").iter() {
        let path = std::path::Path::new(header);
        let key = path
            .strip_prefix(&sdk)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| {
                let text = path.to_string_lossy();
                text.split("/lib/clang/")
                    .nth(1)
                    .map(|s| format!("clang/{s}"))
                    .unwrap_or_else(|| {
                        path.file_name()
                            .expect("header name")
                            .to_string_lossy()
                            .into_owned()
                    })
            });
        println!("cargo:rerun-if-changed={header}");
        headers.insert(key, hash(&std::fs::read(path).expect("hash SDK header")));
    }
    let generated = std::fs::read(out.join("bindings.rs")).expect("generated bindings");
    let provenance = serde_json::json!({"bindgen":"0.72.1","cltools":receipt,"libclang":clang,"sdk":sdk_version,"target":target,"deployment":"12.0",
        "wrapper_sha256":hash(include_bytes!("wrapper.h")),"symbols_sha256":hash(include_bytes!("symbols.txt")),
        "bindings_sha256":hash(&generated),"headers":headers});
    std::fs::write(
        out.join("provenance.json"),
        serde_json::to_vec_pretty(&provenance).expect("provenance JSON"),
    )
    .expect("write provenance");
    println!("cargo:rerun-if-changed=approved.json");
    println!("cargo:rerun-if-env-changed=DAW_AUV2_VERIFY_SDK");
    // Verification produces a candidate; only a reviewed approved.json update
    // accepts another generation tuple. Never regenerate that baseline in build.rs.
    if env::var_os("DAW_AUV2_VERIFY_SDK").is_none() {
        let approved: serde_json::Value = serde_json::from_slice(include_bytes!("approved.json"))
            .expect("approved generator tuple");
        assert_eq!(
            provenance, approved,
            "Unapproved AU SDK/libclang/bindings. Use DAW_AUV2_VERIFY_SDK=1 for ABI verification and review the candidate provenance before updating approved.json."
        );
    }
    println!("cargo:rerun-if-changed=pin.c");
    println!("cargo:rerun-if-changed=abi.c");
    cc::Build::new()
        .compiler(&compiler)
        .file("abi.c")
        .flag("-isysroot")
        .flag(&sdk)
        .flag("-mmacosx-version-min=12.0")
        .compile("daw_au_abi");
    cc::Build::new()
        .compiler(&compiler)
        .file("pin.c")
        .flag("-isysroot")
        .flag(&sdk)
        .flag("-mmacosx-version-min=12.0")
        .compile("daw_au_pin");
    for framework in ["AudioToolbox", "CoreAudio", "CoreFoundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {}
