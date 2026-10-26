//! Shared VST3 boundary for independent portable effect plugins.
#![allow(non_snake_case)] // SDK method names are part of its COM interfaces.
#[cfg(feature = "allocation-check")]
mod allocations;
mod component;
mod controller;
mod editor;
mod factory;
mod process;
mod queues;
mod telemetry;
use daw_drive_plugin::IDENTITY;
use daw_plugin_vst3::{class_id, sys::Steinberg::*};
pub const PROCESSOR_ID: TUID = class_id(IDENTITY.processor_id);
pub const CONTROLLER_ID: TUID = class_id(IDENTITY.controller_id);
pub use factory::GetPluginFactory;

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn bundleEntry(_: *mut std::ffi::c_void) -> bool {
    true
}
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn bundleExit() -> bool {
    true
}
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
pub extern "C" fn ModuleEntry(_: *mut std::ffi::c_void) -> bool {
    true
}
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
pub extern "C" fn ModuleExit() -> bool {
    true
}
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "system" fn InitDll() -> bool {
    true
}
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "system" fn ExitDll() -> bool {
    true
}
