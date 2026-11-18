use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    pub static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    pub static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

pub struct CountingAllocator<const COUNT_FREES: bool>;

fn record() {
    TRACK_ALLOCATIONS.with(|enabled| {
        if enabled.get() {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: This allocator forwards unchanged layouts and pointers to System.
unsafe impl<const COUNT_FREES: bool> GlobalAlloc for CountingAllocator<COUNT_FREES> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNT_FREES {
            record();
        }
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
