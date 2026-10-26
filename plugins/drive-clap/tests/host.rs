#![allow(clippy::undocumented_unsafe_blocks)]
use daw_drive::Drive;
use daw_drive_clap as plugin_export;
include!("host/support.rs");
#[path = "host/lifecycle.rs"]
mod lifecycle;
#[path = "host/parity.rs"]
mod parity;
#[path = "host/performance.rs"]
mod performance;
