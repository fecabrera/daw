//! Explicit custom-titlebar window drag requests for the macOS desktop adapter.

/// Consume the custom-header drag request before the desktop host applies viewport commands.
pub fn take_window_drag_request(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| data.remove_temp::<bool>(egui::Id::new("custom-header-window-drag")))
        .unwrap_or(false)
}

pub(crate) fn request_window_drag(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("custom-header-window-drag"), true));
    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
}
