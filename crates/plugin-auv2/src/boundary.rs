//! Callback containment. Hooks belong to this Rust image and are never replaced in render.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::OnceLock,
};
thread_local! { static DEPTH: Cell<u32> = const { Cell::new(0) }; }
static INSTALL: OnceLock<bool> = OnceLock::new();
pub fn install() -> bool {
    *INSTALL.get_or_init(|| {
        unsafe extern "C" {
            fn daw_au_pin_image() -> bool;
        }
        if !unsafe { daw_au_pin_image() } {
            return false;
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !DEPTH.try_with(|v| v.get() != 0).unwrap_or(false) {
                previous(info);
            }
        }));
        true
    })
}
/// The guard also handles recursive host callbacks without changing the hook.
pub fn call<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            DEPTH.with(|v| v.set(v.get() - 1));
        }
    }
    DEPTH.with(|v| v.set(v.get() + 1));
    let _guard = Guard;
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            fallback
        }
    }
}
