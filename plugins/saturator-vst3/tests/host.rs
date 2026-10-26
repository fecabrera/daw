#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_saturator::Saturator as Drive;
use daw_saturator_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "host/cases.rs"]
mod cases;

#[path = "../../drive-vst3/tests/host/performance.rs"]
mod performance;
