use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ScreenToClient,
    UI::WindowsAndMessaging::GetCursorPos,
};

pub struct FileDragPointer {
    window: HWND,
}

impl FileDragPointer {
    pub fn new(context: &eframe::CreationContext<'_>) -> Option<Self> {
        let RawWindowHandle::Win32(handle) = context.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some(Self {
            window: handle.hwnd.get(),
        })
    }

    pub fn update(&self, context: &egui::Context, input: &mut egui::RawInput) {
        if input.hovered_files.is_empty() && input.dropped_files.is_empty() {
            return;
        }
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: eframe owns this live HWND. Both functions write only to the
        // provided POINT; calls run on the window's UI thread.
        let positioned = unsafe {
            GetCursorPos(&mut point) != 0 && ScreenToClient(self.window, &mut point) != 0
        };
        if positioned {
            let scale = context.pixels_per_point();
            input.events.push(egui::Event::PointerMoved(egui::pos2(
                point.x as f32 / scale,
                point.y as f32 / scale,
            )));
        }
    }
}
