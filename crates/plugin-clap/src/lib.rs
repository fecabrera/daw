//! Narrow CLAP binding facade. Shared boundary helpers are format-neutral.
pub use clap_sys as sys;
pub use daw_plugin_support::{Snapshot, allocation_probe, boundary, write_text};
