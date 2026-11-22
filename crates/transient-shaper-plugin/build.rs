#[path = "../drive-plugin/manifest.rs"]
mod manifest;
#[path = "src/parameters.rs"]
#[allow(dead_code)]
mod parameters;

fn main() {
    let path = "../../plugins/transient-shaper/unit.toml";
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-changed=../drive-plugin/manifest.rs");
    println!("cargo:rerun-if-changed=src/parameters.rs");
    let input = std::fs::read_to_string(path).expect("read Transient Shaper plugin manifest");
    let output = manifest::generate(&input).expect("validate Transient Shaper plugin manifest");
    let directory = std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR");
    std::fs::write(
        std::path::PathBuf::from(directory).join("manifest.rs"),
        output,
    )
    .expect("write Transient Shaper plugin metadata");
}
