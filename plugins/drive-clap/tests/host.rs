#![allow(clippy::undocumented_unsafe_blocks)]
include!("host/support.rs");
#[path = "host/lifecycle.rs"]
mod lifecycle;
#[path = "host/parity.rs"]
mod parity;
#[path = "host/performance.rs"]
mod performance;
