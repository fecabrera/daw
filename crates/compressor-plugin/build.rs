#[path = "../drive-plugin/manifest.rs"]
mod manifest;
#[path = "src/parameters.rs"]
#[allow(dead_code)]
mod parameters;

fn main() {
    let package = env!("CARGO_PKG_NAME");
    let slug = package
        .trim_start_matches("daw-")
        .trim_end_matches("-plugin");
    let path = format!("../../plugins/{slug}/unit.toml");
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-changed=../drive-plugin/manifest.rs");
    println!("cargo:rerun-if-changed=../compressor-plugin/src/parameters.rs");
    let input = std::fs::read_to_string(&path).expect("read dynamics plugin manifest");
    let output = manifest::generate(&input).expect("validate dynamics plugin manifest");
    let directory = std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR");
    std::fs::write(
        std::path::PathBuf::from(directory).join("manifest.rs"),
        output,
    )
    .expect("write generated dynamics plugin metadata");
}
