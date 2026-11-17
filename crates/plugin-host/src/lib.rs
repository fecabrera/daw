//! Control-thread discovery and isolated native plugin inspection.
//! No filesystem or loader API in this crate may run on an audio callback.
mod aliases;
mod architecture;
mod discovery;
mod folders;
#[cfg(target_os = "macos")]
mod macos_bundle;
mod native;
mod registry;
mod scan;
mod scanner_entry;
mod service;
mod state;
pub use daw_unit_core::{HostedPluginDescriptor, HostedRole, PluginFormat, PluginIdentity};
pub use discovery::*;
pub use folders::*;
pub use native::*;
pub use registry::*;
pub use scan::*;
pub use scanner_entry::*;
pub use service::*;
pub use state::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Plugin I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plugin loader: {0}")]
    Loader(#[from] libloading::Error),
    #[error("Plugin metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, Error>;
