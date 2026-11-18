//! Platform services for SDK control calls. These never run on audio.
use super::*;
#[cfg(target_os = "macos")]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_autoreleasePoolPush() -> *mut std::ffi::c_void;
    fn objc_autoreleasePoolPop(pool: *mut std::ffi::c_void);
}
#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: *const std::ffi::c_void;
    fn CFRunLoopRunInMode(
        mode: *const std::ffi::c_void,
        seconds: f64,
        return_after_source: bool,
    ) -> i32;
    fn pthread_main_np() -> i32;
}
pub(super) struct MainServices {
    #[cfg(target_os = "windows")]
    com_initialized: bool,
    #[cfg(target_os = "windows")]
    error: Option<i32>,
}
impl MainServices {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        let status = unsafe {
            windows_sys::Win32::System::Com::CoInitializeEx(
                std::ptr::null(),
                windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
            )
        };
        Self {
            #[cfg(target_os = "windows")]
            com_initialized: status >= 0,
            #[cfg(target_os = "windows")]
            error: if status < 0 && status != 0x80010106u32 as i32 {
                Some(status)
            } else {
                None
            },
        }
    }
    pub fn check(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        if unsafe { pthread_main_np() } == 0 {
            return Err(Error::Invalid(
                "Native plugin control calls require the application main thread".into(),
            ));
        }
        #[cfg(target_os = "windows")]
        if let Some(status) = self.error {
            return Err(Error::Invalid(format!(
                "Cannot initialize plugin COM services ({status:#x})"
            )));
        }
        Ok(())
    }
}
#[cfg(target_os = "windows")]
impl Drop for MainServices {
    fn drop(&mut self) {
        if self.com_initialized {
            unsafe {
                windows_sys::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

/// End this scope before returning to the event loop, which can drain its
/// enclosing pool before the host receives another event.
pub(super) struct MainFrame {
    #[cfg(target_os = "macos")]
    pool: *mut std::ffi::c_void,
}
impl MainFrame {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            pool: unsafe { objc_autoreleasePoolPush() },
        }
    }
}
impl Drop for MainFrame {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            objc_autoreleasePoolPop(self.pool);
        }
    }
}
/// CLI only. Desktop applications already own their native event loop.
pub(crate) fn pump_application_events() {
    let _scope = MainFrame::new();
    #[cfg(target_os = "macos")]
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, true);
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let mut message: MSG = std::mem::zeroed();
        for _ in 0..256 {
            if PeekMessageW(&mut message, 0, 0, 0, PM_REMOVE) == 0 {
                break;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn host_can_outlive_the_autorelease_pool_of_its_creation_event() {
        let host = {
            let _creation_event = MainFrame::new();
            NativeHost::new()
        };
        let _close_event = MainFrame::new();
        drop(host);
    }
}
