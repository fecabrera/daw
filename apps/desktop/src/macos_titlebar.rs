use daw_ui::settings::TitlebarStyle;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow, NSWindowStyleMask, NSWindowTitleVisibility};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Change window chrome without recreating eframe's window or native adapters.
pub struct Titlebar {
    window: Retained<NSWindow>,
    applied: Option<TitlebarStyle>,
    native_movable: bool,
}

impl Titlebar {
    pub fn new(context: &eframe::CreationContext<'_>, style: TitlebarStyle) -> Option<Self> {
        let RawWindowHandle::AppKit(handle) = context.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: eframe supplies a live NSView on AppKit's main thread. Retain
        // the view while obtaining its window; retain the window for this adapter.
        // Creation and update both run on the main thread, while eframe is alive.
        let view = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?;
        let window = view.window()?;
        let mut titlebar = Self {
            native_movable: window.isMovable(),
            window,
            applied: None,
        };
        titlebar.update(style);
        Some(titlebar)
    }

    pub fn update(&mut self, style: TitlebarStyle) {
        if self.applied == Some(style) {
            return;
        }
        let custom = style == TitlebarStyle::Custom;
        self.window
            .setStyleMask(style_mask(self.window.styleMask(), style));
        self.window.setTitlebarAppearsTransparent(custom);
        self.window.setTitleVisibility(if custom {
            NSWindowTitleVisibility::Hidden
        } else {
            NSWindowTitleVisibility::Visible
        });
        self.window.setMovable(!custom && self.native_movable);
        self.applied = Some(style);
    }

    pub fn prepare_input(&self) {
        if self.applied == Some(TitlebarStyle::Custom) {
            // AppKit can start titlebar drags before egui sees the pointer press.
            self.window.setMovable(false);
        }
    }

    pub fn finish(&self, ctx: &egui::Context) {
        let requested = daw_ui::window_drag::take_window_drag_request(ctx);
        if self.applied == Some(TitlebarStyle::Custom) {
            // eframe processes StartDrag after this callback. Enable AppKit movement
            // only for an explicit gesture from the title or unused header space.
            self.window.setMovable(requested);
        }
    }
}

fn style_mask(mut mask: NSWindowStyleMask, style: TitlebarStyle) -> NSWindowStyleMask {
    // Preserve caption buttons, resizing, and the current fullscreen state.
    mask.set(
        NSWindowStyleMask::FullSizeContentView,
        style == TitlebarStyle::Custom,
    );
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_titlebars_preserves_other_window_features() {
        let original = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullScreen;
        let custom = style_mask(original, TitlebarStyle::Custom);
        assert!(custom.contains(NSWindowStyleMask::FullSizeContentView));
        assert_eq!(style_mask(custom, TitlebarStyle::Native), original);
        assert_eq!(style_mask(custom, TitlebarStyle::Custom), custom);
    }
}
