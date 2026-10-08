use egui::{Align, InnerResponse, Layout, Ui, Vec2};

/// Reserve the tallest control's height before placing vertically centered items.
pub fn centered<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center),
        content,
    )
}

/// Keep row widget identities attached to an item when its position changes.
pub fn centered_with_id<R>(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.push_id(id_salt, |ui| centered(ui, height, content))
        .inner
}

pub fn wrapped<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center).with_main_wrap(true),
        content,
    )
}
