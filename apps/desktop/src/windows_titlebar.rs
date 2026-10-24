//! DWM-owned caption buttons and frame behavior around the egui header.
//! The subclass runs on the window thread; removal precedes freeing its boxed state.
use daw_ui::{
    settings::{ColorTheme, TitlebarStyle},
    titlebar::{CaptionMetrics, HeaderLayout},
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::{Cell, RefCell};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DWMWA_CAPTION_BUTTON_BOUNDS, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DwmDefWindowProc, DwmExtendFrameIntoClientArea,
            DwmGetWindowAttribute, DwmSetWindowAttribute,
        },
        Gdi::ScreenToClient,
    },
    UI::{
        Controls::MARGINS,
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
const SUBCLASS_ID: usize = 0x444157;
struct State {
    alive: Cell<bool>,
    custom: Cell<bool>,
    scale: Cell<f32>,
    frame_height: Cell<i32>,
    layout: RefCell<HeaderLayout>,
    context: egui::Context,
}
pub struct Titlebar {
    window: HWND,
    state: Box<State>,
    applied: Option<(ColorTheme, TitlebarStyle, f32, bool)>,
}
impl Titlebar {
    pub fn new(context: &eframe::CreationContext<'_>) -> Option<Self> {
        let RawWindowHandle::Win32(handle) = context.window_handle().ok()?.as_raw() else {
            return None;
        };
        Self::attach(handle.hwnd.get(), context.egui_ctx.clone())
    }
    fn attach(window: HWND, context: egui::Context) -> Option<Self> {
        let state = Box::new(State {
            alive: Cell::new(true),
            custom: Cell::new(false),
            scale: Cell::new(context.pixels_per_point()),
            frame_height: Cell::new(0),
            layout: RefCell::new(HeaderLayout::default()),
            context,
        });
        // SAFETY: live HWND on its owning thread. The Box stays stable until the subclass
        // is removed. No borrow of the geometry spans a reentrant Windows API call.
        if unsafe {
            SetWindowSubclass(
                window,
                Some(subclass),
                SUBCLASS_ID,
                (&*state as *const State) as usize,
            )
        } == 0
        {
            tracing::warn!("Cannot install the Windows titlebar adapter");
            return None;
        }
        Some(Self {
            window,
            state,
            applied: None,
        })
    }
    pub fn prepare(&mut self, ui: &mut daw_ui::DawUi, ctx: &egui::Context) {
        if !self.state.alive.get() {
            return;
        }
        let settings = ui.settings();
        // SAFETY: live HWND on the owning thread.
        let maximized = unsafe { IsZoomed(self.window) } != 0;
        let top_inset = if maximized {
            frame_inset(self.window)
        } else {
            0
        };
        let changed = self
            .state
            .custom
            .replace(settings.titlebar == TitlebarStyle::Custom)
            != (settings.titlebar == TitlebarStyle::Custom);
        self.state.scale.set(ctx.pixels_per_point());
        let appearance = (
            settings.theme,
            settings.titlebar,
            ctx.pixels_per_point(),
            maximized,
        );
        if self.applied != Some(appearance) {
            let theme_changed = self
                .applied
                .as_ref()
                .is_none_or(|(theme, _, _, _)| *theme != settings.theme);
            // SAFETY: live HWND and initialized attribute data with its documented size.
            unsafe {
                let dark = i32::from(settings.theme == ColorTheme::Dark);
                DwmSetWindowAttribute(
                    self.window,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    (&dark as *const i32).cast(),
                    size_of::<i32>() as u32,
                );
                let palette = daw_ui::theme::palette(ctx);
                for (attribute, color) in [
                    (DWMWA_CAPTION_COLOR, palette.panel),
                    (DWMWA_TEXT_COLOR, palette.text),
                ] {
                    let color = u32::from(color.r())
                        | (u32::from(color.g()) << 8)
                        | (u32::from(color.b()) << 16);
                    DwmSetWindowAttribute(
                        self.window,
                        attribute as u32,
                        (&color as *const u32).cast(),
                        size_of::<u32>() as u32,
                    );
                }
                let margins = MARGINS {
                    cxLeftWidth: 0,
                    cxRightWidth: 0,
                    cyTopHeight: if self.state.custom.get() {
                        // Initial estimate; finish uses the laid-out header bounds.
                        ((daw_ui::toolbars::HEIGHT + 1.0) * ctx.pixels_per_point()).ceil() as i32
                            + top_inset
                            + 1
                    } else {
                        0
                    },
                    cyBottomHeight: 0,
                };
                self.state.frame_height.set(margins.cyTopHeight);
                DwmExtendFrameIntoClientArea(self.window, &margins);
                if changed || theme_changed {
                    // Keep caption/system-menu/resize styles for native actions and Snap.
                    SetWindowPos(
                        self.window,
                        0,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                    ctx.request_repaint();
                }
            }
            self.applied = Some(appearance);
        }
        let mut metrics = CaptionMetrics {
            top_inset: top_inset as f32 / ctx.pixels_per_point(),
            ..Default::default()
        };
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: live HWND and initialized writable native output values.
        let found = unsafe {
            DwmGetWindowAttribute(
                self.window,
                DWMWA_CAPTION_BUTTON_BOUNDS as u32,
                (&mut bounds as *mut RECT).cast(),
                size_of::<RECT>() as u32,
            )
        } >= 0;
        if found && bounds.right > bounds.left {
            metrics.width = (bounds.right - bounds.left) as f32 / ctx.pixels_per_point();
        }
        ui.set_caption_metrics(metrics);
    }
    pub fn finish(&mut self, ui: &daw_ui::DawUi, ctx: &egui::Context) {
        let layout = ui.header_layout(ctx);
        if self.state.alive.get() && self.state.custom.get() && layout.bounds.is_positive() {
            // Keep the frame boundary inside the opaque playback row. Include the
            // 1-point divider and one physical pixel beyond it. Measured bounds
            // include the maximized inset and egui's layout rounding at this scale.
            let height =
                ((layout.bounds.bottom() + 1.0) * ctx.pixels_per_point()).ceil() as i32 + 1;
            if self.state.frame_height.replace(height) != height {
                let margins = MARGINS {
                    cxLeftWidth: 0,
                    cxRightWidth: 0,
                    cyTopHeight: height,
                    cyBottomHeight: 0,
                };
                // SAFETY: live HWND on its owning thread and initialized margins.
                unsafe { DwmExtendFrameIntoClientArea(self.window, &margins) };
            }
        }
        *self.state.layout.borrow_mut() = layout;
    }

    pub fn pointer_input(&self, input: &mut egui::RawInput) {
        if !self.state.custom.get() || !self.state.alive.get() {
            return;
        }
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: called on the live window thread with initialized output storage.
        if unsafe { GetCursorPos(&mut point) } != 0
            && unsafe { WindowFromPoint(point) } == self.window
            && unsafe { ScreenToClient(self.window, &mut point) } != 0
        {
            let pos = egui::pos2(
                point.x as f32 / self.state.scale.get(),
                point.y as f32 / self.state.scale.get(),
            );
            if self.state.layout.borrow().bounds.contains(pos) {
                // Non-client mouse moves do not arrive as winit client pointer events.
                // Feed the header position back for caption and shortened-title tooltips.
                input.events.push(egui::Event::PointerMoved(pos));
            }
        }
    }
}
impl Drop for Titlebar {
    fn drop(&mut self) {
        if self.state.alive.get() {
            // SAFETY: remove before freeing the Box. WM_NCDESTROY already removes it
            // when eframe destroys the HWND first. No callback can outlive the state.
            unsafe {
                RemoveWindowSubclass(self.window, Some(subclass), SUBCLASS_ID);
            }
        }
    }
}
unsafe extern "system" fn subclass(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: registered Box pointer remains live until the subclass is removed.
    let state = unsafe { &*(data as *const State) };
    if message == WM_NCDESTROY {
        state.alive.set(false);
        unsafe {
            RemoveWindowSubclass(window, Some(subclass), SUBCLASS_ID);
        }
    }
    if state.custom.get() && state.alive.get() {
        if matches!(message, WM_SIZE | WM_ACTIVATE | WM_DWMCOMPOSITIONCHANGED) {
            let margins = MARGINS {
                cxLeftWidth: 0,
                cxRightWidth: 0,
                cyTopHeight: state.frame_height.get(),
                cyBottomHeight: 0,
            };
            // SAFETY: reapply the initialized frame after native size/activation changes.
            unsafe { DwmExtendFrameIntoClientArea(window, &margins) };
        }
        if message == WM_NCCALCSIZE && wparam != 0 {
            // SAFETY: Windows supplies writable NCCALCSIZE_PARAMS for nonzero WPARAM.
            let bounds = unsafe { &mut (*(lparam as *mut NCCALCSIZE_PARAMS)).rgrc[0] };
            let border = frame_inset(window);
            bounds.left += border;
            bounds.right -= border;
            bounds.bottom -= border;
            // Keep the original top so DWM can draw and hit-test native captions in
            // both window states. egui offsets its content past the maximized frame.
            return 0;
        }
        if matches!(
            message,
            WM_NCMOUSEMOVE | WM_NCMOUSELEAVE | WM_NCLBUTTONUP | WM_CAPTURECHANGED | WM_SIZE
        ) {
            state.context.request_repaint();
        }
        let mut result = 0;
        // Let Windows paint, hit-test and track its caption buttons before application
        // drag targets. WM_CLOSE continues to eframe and the existing close guards.
        let handled =
            unsafe { DwmDefWindowProc(window, message, wparam, lparam, &mut result) } != 0;
        if handled {
            return result;
        }
        if message == WM_NCHITTEST {
            let mut point = POINT {
                x: lparam as i16 as i32,
                y: (lparam >> 16) as i16 as i32,
            };
            if unsafe { ScreenToClient(window, &mut point) } != 0 {
                let pos = egui::pos2(
                    point.x as f32 / state.scale.get(),
                    point.y as f32 / state.scale.get(),
                );
                let layout = state.layout.borrow();
                if point.y >= 0 && point.y < frame_inset(window) && unsafe { IsZoomed(window) } == 0
                {
                    return HTTOP as LRESULT;
                }
                if layout.can_drag(pos) {
                    return HTCAPTION as LRESULT;
                }
                if layout.bounds.contains(pos) {
                    return HTCLIENT as LRESULT;
                }
            }
        }
    }
    // SAFETY: all unhandled events continue to eframe's original window procedure.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}
fn frame_inset(window: HWND) -> i32 {
    // SAFETY: live HWND on the window thread; functions return scalar metrics.
    unsafe {
        let dpi = GetDpiForWindow(window);
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_window() -> HWND {
        // This hidden test window belongs only to this thread. It does not touch the
        // running desktop app, application storage, user projects or display focus.
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: valid class name; no parent, menu or creation data; destroyed below.
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                -1000,
                100,
                900,
                600,
                0,
                0,
                0,
                std::ptr::null(),
            )
        };
        assert_ne!(window, 0);
        window
    }

    #[test]
    fn custom_frame_boundary_stays_below_the_laid_out_header_divider() {
        let window = test_window();
        let ctx = egui::Context::default();
        daw_ui::theme::configure(&ctx);
        daw_ui::fonts::configure(&ctx);
        daw_ui::icons::configure(&ctx);
        let mut adapter = Titlebar::attach(window, ctx.clone()).expect("Window subclass");
        let mut ui = daw_ui::DawUi::default();
        let mut settings = ui.settings();
        settings.titlebar = TitlebarStyle::Custom;
        ui.set_settings(&ctx, settings);
        // A hidden window lets this test exercise native metrics and egui geometry
        // without changing display focus or requiring a composed GPU surface.
        for maximized in [false, true] {
            // SAFETY: this thread owns the live hidden window and changes only its style.
            unsafe {
                SetWindowLongPtrW(
                    window,
                    GWL_STYLE,
                    (WS_OVERLAPPEDWINDOW | if maximized { WS_MAXIMIZE } else { 0 }) as isize,
                );
            }
            for zoom in [1.0, 1.25, 1.5, 2.0, 3.0, 4.0] {
                ctx.set_zoom_factor(zoom);
                for _ in 0..2 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(900.0, 600.0),
                            )),
                            ..Default::default()
                        },
                        |root| {
                            adapter.prepare(&mut ui, root.ctx());
                            ui.show(root);
                            adapter.finish(&ui, root.ctx());
                        },
                    );
                    output.textures_delta.clear();
                }
                let header = ui.header_layout(&ctx).bounds;
                assert!(header.is_positive());
                let divider_bottom = (header.bottom() + 1.0) * ctx.pixels_per_point();
                assert!(
                    adapter.state.frame_height.get() as f32 > divider_bottom,
                    "maximized={maximized}, zoom={zoom}, frame={}, divider bottom={divider_bottom}",
                    adapter.state.frame_height.get(),
                );
            }
        }
        // SAFETY: the test owns this hidden HWND; destruction removes the subclass.
        unsafe { DestroyWindow(window) };
        assert!(!adapter.state.alive.get());
    }

    #[test]
    fn native_frame_round_trip_retains_styles_hit_testing_and_subclass_lifetime() {
        let window = test_window();
        let mut adapter =
            Titlebar::attach(window, egui::Context::default()).expect("Window subclass");
        let ctx = egui::Context::default();
        let mut ui = daw_ui::DawUi::default();
        let mut native = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: live window and initialized output rect.
        unsafe {
            GetClientRect(window, &mut native);
        }
        let original_style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
        let mut settings = ui.settings();
        settings.titlebar = TitlebarStyle::Custom;
        ui.set_settings(&ctx, settings.clone());
        adapter.prepare(&mut ui, &ctx);
        let mut custom = native;
        unsafe {
            GetClientRect(window, &mut custom);
        }
        assert!(
            custom.bottom > native.bottom,
            "Caption extends the client area"
        );
        assert_eq!(
            unsafe { GetWindowLongPtrW(window, GWL_STYLE) },
            original_style
        );
        *adapter.state.layout.borrow_mut() = HeaderLayout {
            bounds: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 38.0)),
            controls: vec![egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(32.0, 38.0),
            )],
            blocked: false,
            captions: [egui::Rect::NOTHING; 3],
        };
        let hit = |x, y| {
            let mut point = POINT { x, y };
            // SAFETY: convert a client position, then send a synchronous hit-test on
            // this owned window. Packing preserves negative monitor coordinates.
            unsafe {
                windows_sys::Win32::Graphics::Gdi::ClientToScreen(window, &mut point);
            }
            let packed = ((point.y as u16 as u32) << 16) | point.x as u16 as u32;
            unsafe { SendMessageW(window, WM_NCHITTEST, 0, packed as LPARAM) }
        };
        assert_eq!(hit(200, 20), HTCAPTION as LRESULT);
        assert_eq!(hit(15, 20), HTCLIENT as LRESULT);
        assert_eq!(hit(200, 1), HTTOP as LRESULT);
        adapter.state.layout.borrow_mut().captions = std::array::from_fn(|index| {
            egui::Rect::from_min_size(
                egui::pos2(600.0 + index as f32 * 46.0, 0.0),
                egui::vec2(46.0, 38.0),
            )
        });
        // Caption rectangles only reserve UI space. Windows owns their hit testing.
        // A hidden window has no composed caption controls; live coverage uses the
        // isolated GPU preview rather than synthetic caption hit targets here.
        unsafe {
            SetWindowLongPtrW(window, GWL_STYLE, original_style | WS_MAXIMIZE as isize);
            SetWindowPos(
                window,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        assert_eq!(hit(200, 1), HTCAPTION as LRESULT);
        unsafe {
            SetWindowLongPtrW(window, GWL_STYLE, original_style);
            SetWindowPos(
                window,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        adapter.state.layout.borrow_mut().blocked = true;
        assert_eq!(hit(200, 20), HTCLIENT as LRESULT);
        settings.titlebar = TitlebarStyle::Native;
        ui.set_settings(&ctx, settings.clone());
        adapter.prepare(&mut ui, &ctx);
        let mut restored = native;
        unsafe {
            GetClientRect(window, &mut restored);
        }
        assert_eq!(restored.bottom, native.bottom);
        assert_eq!(
            unsafe { GetWindowLongPtrW(window, GWL_STYLE) },
            original_style
        );
        // WM_CLOSE reaches the original window procedure and destroys our window.
        unsafe {
            SendMessageW(window, WM_CLOSE, 0, 0);
        }
        assert!(!adapter.state.alive.get());
        assert_eq!(unsafe { IsWindow(window) }, 0);
        drop(adapter);
    }
}
