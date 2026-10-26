#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_compressor::Compressor as Drive;
use daw_compressor_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;
