use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static WATCH:Cell<bool>=const {Cell::new(false)};static HEAP:Cell<u64>=const {Cell::new(0)};}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        WATCH.with(|w| {
            if w.get() {
                HEAP.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        WATCH.with(|w| {
            if w.get() {
                HEAP.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        WATCH.with(|w| {
            if w.get() {
                HEAP.with(|c| c.set(c.get() + 1));
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
pub fn measured<T>(call: impl FnOnce() -> T) -> T {
    HEAP.with(|n| n.set(0));
    WATCH.with(|w| w.set(true));
    let out = call();
    WATCH.with(|w| w.set(false));
    assert_eq!(HEAP.with(Cell::get), 0, "host callback heap operations");
    out
}
