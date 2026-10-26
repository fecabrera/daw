#![allow(non_snake_case, clippy::undocumented_unsafe_blocks)]
use daw_drive::DriveV2 as Drive;
use daw_drive_v2_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../drive-clap/tests/host/lifecycle.rs"]
mod lifecycle;
#[path = "../../drive-clap/tests/host/parity.rs"]
mod parity;
#[path = "../../drive-clap/tests/host/performance.rs"]
mod performance;
#[path = "host/v2.rs"]
mod v2;
