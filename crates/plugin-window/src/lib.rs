//! Native child-window boundary. Construct and retire handles on the host UI thread.
pub use baseview::{HandlerError, Window, host::Host};
pub use egui_baseview::{App, Frame};
use egui_baseview::{EguiWindow, EguiWindowSettings};
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};
use std::ffi::c_void;
#[cfg(not(target_os = "macos"))]
use std::num::NonZeroUsize;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;

pub struct Parent(RawWindowHandle);
impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: caller keeps the SDK parent alive until the child is dropped.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}
impl Parent {
    /// # Safety
    /// `pointer` is a live native parent of this platform; its lifetime covers the child.
    pub unsafe fn new(pointer: *mut c_void) -> Option<Self> {
        #[cfg(target_os = "macos")]
        let raw = RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(
            NonNull::new(pointer)?,
        ));
        #[cfg(target_os = "windows")]
        let raw = RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
            NonZeroUsize::new(pointer as usize)?.cast_signed(),
        ));
        #[cfg(target_os = "linux")]
        let raw = RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle::new(
            NonZeroUsize::new(pointer as usize)?.get() as _,
        ));
        Some(Self(raw))
    }
}
pub fn create(
    app: impl App,
    parent: &Parent,
    width: f64,
    height: f64,
    scale: f32,
    host: Host,
) -> Result<Window, baseview::Error> {
    let size = if cfg!(target_os = "macos") {
        baseview::dpi::Size::Logical(baseview::dpi::LogicalSize::new(width, height))
    } else {
        baseview::dpi::Size::Physical(baseview::dpi::PhysicalSize::new(
            (width * scale as f64).round() as u32,
            (height * scale as f64).round() as u32,
        ))
    };
    EguiWindow::create_with_host(
        EguiWindowSettings::new()
            .with_title("Drive")
            .with_size(size)
            .with_resizable(false)
            .with_zoom_factor(scale)
            .with_parent(parent),
        app,
        Some(host),
    )
}
pub fn resize(window: &Window, width: f64, height: f64) -> bool {
    window
        .resize(baseview::dpi::PhysicalSize::new(
            width.round() as u32,
            height.round() as u32,
        ))
        .is_ok()
}

struct MainWake<F: FnMut() + Send + 'static>(F);
impl<F: FnMut() + Send + 'static> baseview::host::HostMainThreadCaller for MainWake<F> {
    fn call_main_thread(&mut self) {
        (self.0)();
    }
}
pub fn host(wake: impl FnMut() + Send + 'static) -> Host {
    Host::new().with_main_thread(MainWake(wake))
}
