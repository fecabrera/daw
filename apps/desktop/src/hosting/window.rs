//! Host-owned editor parents. Destroy the plugin editor before its parent.
#[cfg(target_os = "macos")]
mod platform {
    use objc2::{MainThreadOnly, rc::Retained};
    use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
    pub struct Window {
        window: Retained<NSWindow>,
    }
    impl Window {
        pub fn new(title: &str) -> Result<Self, String> {
            let main = MainThreadMarker::new().ok_or("Editor window requires the main thread")?;
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(main),
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(640.0, 480.0)),
                    NSWindowStyleMask::Titled
                        | NSWindowStyleMask::Closable
                        | NSWindowStyleMask::Miniaturizable,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe {
                window.setReleasedWhenClosed(false);
            }
            window.setTitle(&NSString::from_str(title));
            window.center();
            Ok(Self { window })
        }
        pub fn parent(&self) -> daw_plugin_host::NativeParent {
            let view = self.window.contentView().expect("window content view");
            daw_plugin_host::NativeParent::Cocoa(
                (&*view as *const objc2_app_kit::NSView).cast_mut().cast(),
            )
        }
        pub fn resize(&self, width: u32, height: u32) {
            self.window
                .setContentSize(NSSize::new(f64::from(width), f64::from(height)));
        }
        pub fn resizable(&self, allowed: bool) {
            let mut mask = self.window.styleMask();
            mask.set(NSWindowStyleMask::Resizable, allowed);
            self.window.setStyleMask(mask);
        }
        pub fn size(&self) -> (u32, u32) {
            let size = self
                .window
                .contentView()
                .expect("window content")
                .frame()
                .size;
            (size.width.max(1.0) as u32, size.height.max(1.0) as u32)
        }
        pub fn scale(&self, _fallback: f64) -> f64 {
            self.window.backingScaleFactor()
        }
        pub fn focused(&self) -> bool {
            self.window.isKeyWindow()
        }
        pub fn show(&self) {
            self.window.makeKeyAndOrderFront(None);
        }
        pub fn visible(&mut self) -> bool {
            self.window.isVisible()
        }
    }
    impl Drop for Window {
        fn drop(&mut self) {
            self.window.close();
        }
    }
}
#[cfg(target_os = "windows")]
mod platform {
    use windows_sys::Win32::{
        Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
    };
    const CLASS: [u16; 16] = [
        68, 65, 87, 80, 108, 117, 103, 105, 110, 69, 100, 105, 116, 111, 114, 0,
    ];
    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        if message == WM_CLOSE {
            unsafe { ShowWindow(window, SW_HIDE) };
            0
        } else {
            unsafe { DefWindowProcW(window, message, w, l) }
        }
    }
    pub struct Window {
        handle: HWND,
    }
    impl Window {
        pub fn new(title: &str) -> Result<Self, String> {
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: CLASS.as_ptr(),
                ..unsafe { std::mem::zeroed() }
            };
            unsafe {
                RegisterClassW(&class);
            }
            let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let handle = unsafe {
                CreateWindowExW(
                    0,
                    CLASS.as_ptr(),
                    title.as_ptr(),
                    WS_OVERLAPPEDWINDOW & !WS_THICKFRAME & !WS_MAXIMIZEBOX,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    640,
                    480,
                    0,
                    0,
                    instance,
                    std::ptr::null(),
                )
            };
            if handle == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self { handle })
        }
        pub fn parent(&self) -> daw_plugin_host::NativeParent {
            daw_plugin_host::NativeParent::Win32(self.handle as *mut std::ffi::c_void)
        }
        pub fn resize(&self, width: u32, height: u32) {
            unsafe {
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                AdjustWindowRectEx(
                    &mut rect,
                    (GetWindowLongW(self.handle, GWL_STYLE)) as u32,
                    0,
                    0,
                );
                SetWindowPos(
                    self.handle,
                    0,
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        pub fn resizable(&self, allowed: bool) {
            unsafe {
                let style = GetWindowLongW(self.handle, GWL_STYLE) as u32;
                SetWindowLongW(
                    self.handle,
                    GWL_STYLE,
                    if allowed {
                        style | WS_THICKFRAME | WS_MAXIMIZEBOX
                    } else {
                        style & !WS_THICKFRAME & !WS_MAXIMIZEBOX
                    } as i32,
                );
            }
        }
        pub fn size(&self) -> (u32, u32) {
            unsafe {
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(self.handle, &mut rect);
                (
                    (rect.right - rect.left).max(1) as u32,
                    (rect.bottom - rect.top).max(1) as u32,
                )
            }
        }
        pub fn scale(&self, _fallback: f64) -> f64 {
            f64::from(unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(self.handle) }) / 96.0
        }
        pub fn focused(&self) -> bool {
            unsafe { GetForegroundWindow() == self.handle }
        }
        pub fn show(&self) {
            unsafe {
                ShowWindow(self.handle, SW_SHOW);
                SetForegroundWindow(self.handle);
            }
        }
        pub fn visible(&mut self) -> bool {
            unsafe { IsWindow(self.handle) != 0 && IsWindowVisible(self.handle) != 0 }
        }
    }
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.handle);
            }
        }
    }
}
#[cfg(target_os = "linux")]
#[path = "window_x11.rs"]
mod platform;
pub use platform::Window;
