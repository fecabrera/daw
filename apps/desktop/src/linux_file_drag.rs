use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11_dl::xlib;

pub struct FileDragPointer {
    api: xlib::Xlib,
    display: *mut xlib::Display,
    window: xlib::Window,
}

impl FileDragPointer {
    pub fn new(context: &eframe::CreationContext<'_>) -> Option<Self> {
        let window = match context.window_handle().ok()?.as_raw() {
            RawWindowHandle::Xlib(handle) => handle.window,
            RawWindowHandle::Xcb(handle) => handle.window.get().into(),
            // winit does not expose file-hover/drop events on Wayland.
            _ => return None,
        };
        let api = xlib::Xlib::open().ok()?;
        // SAFETY: A null display name opens the current X11 display. This
        // connection is owned here and is used only on the UI thread.
        let display = unsafe { (api.XOpenDisplay)(std::ptr::null()) };
        if display.is_null() {
            return None;
        }
        Some(Self {
            api,
            display,
            window,
        })
    }

    pub fn update(&self, context: &egui::Context, input: &mut egui::RawInput) {
        if input.hovered_files.is_empty() && input.dropped_files.is_empty() {
            return;
        }
        let (mut root, mut child, mut root_x, mut root_y, mut x, mut y, mut mask) =
            (0, 0, 0, 0, 0, 0, 0);
        // SAFETY: The display is live, eframe owns the window, and all output
        // pointers refer to the local values with XQueryPointer's required types.
        let positioned = unsafe {
            (self.api.XQueryPointer)(
                self.display,
                self.window,
                &mut root,
                &mut child,
                &mut root_x,
                &mut root_y,
                &mut x,
                &mut y,
                &mut mask,
            ) != 0
        };
        if positioned {
            let scale = context.pixels_per_point();
            input.events.push(egui::Event::PointerMoved(egui::pos2(
                x as f32 / scale,
                y as f32 / scale,
            )));
        }
    }
}

impl Drop for FileDragPointer {
    fn drop(&mut self) {
        // SAFETY: This is the connection opened above and it is closed once,
        // after the last query, on the same UI thread.
        unsafe {
            (self.api.XCloseDisplay)(self.display);
        }
    }
}
