//! Opt-in test instrumentation; absent from normal development exports.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static COUNTS: Cell<Option<[u64; 2]>> = const { Cell::new(None) }; }
pub struct Allocator;
fn record(index: usize) {
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            counts[index] += 1;
            cell.set(Some(counts));
        }
    });
}
// SAFETY: every operation delegates the unchanged allocation/layout to System.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(0);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(1);
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(0);
        record(1);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(0);
        unsafe { System.alloc_zeroed(layout) }
    }
}
pub fn begin() {
    COUNTS.with(|cell| cell.set(Some([0; 2])));
}
/// # Safety
/// Non-null pointers must refer to writable u64 output storage.
pub unsafe fn end(allocations: *mut u64, deallocations: *mut u64) {
    let counts = COUNTS.with(|cell| cell.replace(None)).unwrap_or([0; 2]);
    if !allocations.is_null() {
        unsafe {
            allocations.write(counts[0]);
        }
    }
    if !deallocations.is_null() {
        unsafe {
            deallocations.write(counts[1]);
        }
    }
}

/// Positive control for the loaded-library allocator guard.
pub fn probe() {
    let allocation = std::hint::black_box(Box::new([0_u8; 128]));
    drop(std::hint::black_box(allocation));
}
