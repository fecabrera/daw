//! Headless development CLAP export of the original built-in Drive.
//! All unsafe code is confined to CLAP pointer/ownership boundaries.
#[cfg(feature = "allocation-check")]
mod allocations;
mod audio;
mod editor;
mod instance;
mod params;
mod state;
use daw_drive_plugin::IDENTITY;
use daw_plugin_clap::{
    boundary,
    sys::{entry::*, factory::plugin_factory::*, plugin::*, version::*},
};
use std::{
    ffi::{CStr, c_char, c_void},
    ptr,
};

struct Features([*const c_char; 4]);
// SAFETY: pointers refer only to immutable static NUL-terminated literals.
unsafe impl Sync for Features {}
static FEATURES: Features = Features([
    c"audio-effect".as_ptr(),
    c"distortion".as_ptr(),
    c"stereo".as_ptr(),
    ptr::null(),
]);
const fn terminated<const N: usize>(text: &str) -> [c_char; N] {
    assert!(text.len() < N);
    let mut bytes = [0; N];
    let mut index = 0;
    while index < text.len() {
        assert!(text.as_bytes()[index] != 0);
        bytes[index] = text.as_bytes()[index] as c_char;
        index += 1;
    }
    bytes
}
static ID: [c_char; 256] = terminated(IDENTITY.clap_id);
static NAME: [c_char; 64] = terminated(IDENTITY.product);
static VENDOR: [c_char; 64] = terminated(IDENTITY.vendor);
static VERSION: [c_char; 64] = terminated(IDENTITY.version);
static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: ID.as_ptr(),
    name: NAME.as_ptr(),
    vendor: VENDOR.as_ptr(),
    version: VERSION.as_ptr(),
    url: c"https://github.com/fecabrera/daw".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"https://github.com/fecabrera/daw/issues".as_ptr(),
    description: c"Stereo distortion development plugin".as_ptr(),
    features: FEATURES.0.as_ptr(),
};
unsafe extern "C" fn entry_init(_: *const c_char) -> bool {
    true
}
unsafe extern "C" fn entry_deinit() {}
unsafe extern "C" fn factory_count(_: *const clap_plugin_factory) -> u32 {
    1
}
unsafe extern "C" fn factory_descriptor(
    _: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    boundary(ptr::null(), || {
        if index == 0 { &DESCRIPTOR } else { ptr::null() }
    })
}
unsafe extern "C" fn create(
    _: *const clap_plugin_factory,
    host: *const daw_plugin_clap::sys::host::clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    boundary(ptr::null(), || {
        if host.is_null() || id.is_null() {
            return ptr::null();
        }
        // SAFETY: factory arguments are live host-owned CLAP structures/C strings.
        if unsafe { CStr::from_ptr(id) }.to_bytes() != IDENTITY.clap_id.as_bytes()
            || !clap_version_is_compatible(unsafe { (*host).clap_version })
        {
            return ptr::null();
        }
        instance::Instance::create(host, &DESCRIPTOR)
    })
}
static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_count),
    get_plugin_descriptor: Some(factory_descriptor),
    create_plugin: Some(create),
};
unsafe extern "C" fn get_factory(id: *const c_char) -> *const c_void {
    boundary(ptr::null(), || {
        // SAFETY: the host supplies a live C string when non-null.
        if !id.is_null() && unsafe { CStr::from_ptr(id) } == CLAP_PLUGIN_FACTORY_ID {
            (&FACTORY as *const clap_plugin_factory).cast()
        } else {
            ptr::null()
        }
    })
}
#[unsafe(no_mangle)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(get_factory),
};
