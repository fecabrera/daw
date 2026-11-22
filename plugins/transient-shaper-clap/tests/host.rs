#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_transient_shaper::TransientShaper as Drive;
use daw_transient_shaper_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../saturator-clap/tests/host/cases.rs"]
mod cases;

#[path = "../../drive-clap/tests/host/performance.rs"]
mod performance;
