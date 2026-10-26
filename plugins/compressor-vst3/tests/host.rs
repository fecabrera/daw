#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_compressor::Compressor as Drive;
use daw_compressor_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;
