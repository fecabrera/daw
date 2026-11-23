#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
#[path = "../../../crates/expander-plugin/tests/common/direct.rs"]
mod direct;
use daw_expander_vst3 as plugin_export;
use direct::Direct as Drive;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;
