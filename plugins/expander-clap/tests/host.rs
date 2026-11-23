#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
#[path = "../../../crates/expander-plugin/tests/common/direct.rs"]
mod direct;
use daw_expander_clap as plugin_export;
use direct::Direct as Drive;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;
