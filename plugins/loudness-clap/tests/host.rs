#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_loudness::Loudness as Drive;
use daw_loudness_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;

#[test]
fn repeated_activation_and_library_unload_retires_analysis_workers() {
    let _serial = SERIAL.lock().unwrap();
    for _ in 0..32 {
        let mut plugin = Plugin::new();
        plugin.activate(48000, 512);
        // Drop destroys the processor, then unloads the dynamically loaded module.
        drop(plugin);
    }
}
