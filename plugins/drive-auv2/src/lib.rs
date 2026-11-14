//! Shared native Audio Unit v2 export over the existing typed products.
#![cfg(target_os = "macos")]
#![allow(non_upper_case_globals)] // Apple SDK names remain unchanged.
mod native;
pub use native::DAWAudioUnitFactory;

#[cfg(test)]
mod allocations;
