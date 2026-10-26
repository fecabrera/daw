#![allow(non_snake_case, clippy::undocumented_unsafe_blocks)]
use daw_drive::DriveV2 as Drive;
use daw_drive_v2_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../drive-vst3/tests/host/lifecycle.rs"]
mod lifecycle;
#[path = "../../drive-vst3/tests/host/parity.rs"]
mod parity;
#[path = "../../drive-vst3/tests/host/performance.rs"]
mod performance;
#[path = "host/v2.rs"]
mod v2;
