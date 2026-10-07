use objc2::rc::Retained;
use objc2_app_kit::NSView;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub struct FileDragPointer {
    view: Retained<NSView>,
}

impl FileDragPointer {
    pub fn new(context: &eframe::CreationContext<'_>) -> Option<Self> {
        let RawWindowHandle::AppKit(handle) = context.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: eframe supplies the live AppKit NSView. Creation and all calls
        // run on AppKit's main thread; retaining it keeps this pointer valid.
        let view = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?;
        Some(Self { view })
    }

    pub fn update(&self, context: &egui::Context, input: &mut egui::RawInput) {
        if input.hovered_files.is_empty() && input.dropped_files.is_empty() {
            return;
        }
        let Some(window) = self.view.window() else {
            return;
        };
        // winit's macOS file-drag callbacks omit cursor updates. Query the
        // destination view while hovering and at drop, even if Finder has focus.
        let point = self
            .view
            .convertPoint_fromView(window.mouseLocationOutsideOfEventStream(), None);
        let bounds = self.view.bounds();
        let x = point.x - bounds.origin.x;
        let y = if self.view.isFlipped() {
            point.y - bounds.origin.y
        } else {
            bounds.size.height - (point.y - bounds.origin.y)
        };
        let zoom = context.zoom_factor();
        input.events.push(egui::Event::PointerMoved(egui::pos2(
            x as f32 / zoom,
            y as f32 / zoom,
        )));
    }
}
