#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_filter::Filter as Drive;
use daw_filter_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;
