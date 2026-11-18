//! Opt-in test instrumentation; absent from normal development exports.
use daw_plugin_clap::allocation_probe;

#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[unsafe(no_mangle)]
pub extern "C" fn drive_clap_allocation_begin() {
    allocation_probe::begin();
}

/// # Safety
/// Non-null pointers must refer to writable u64 output storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drive_clap_allocation_end(allocations: *mut u64, deallocations: *mut u64) {
    unsafe { allocation_probe::end(allocations, deallocations) };
}

#[unsafe(no_mangle)]
pub extern "C" fn drive_clap_allocation_probe() {
    allocation_probe::probe();
}
