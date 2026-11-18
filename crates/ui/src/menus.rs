//! Shared menu styling and icon submenus.
use crate::icons;
pub use daw_unit_ui::menus::*;

pub(super) fn submenu(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::containers::menu::SubMenuButton::from_button(
        egui::Button::new(label)
            .right_text(
                icons::Lucide::ChevronRight
                    .size(icons::TOOLBAR_ICON_SIZE)
                    .image(),
            )
            .image_tint_follows_text_color(true),
    )
    .config(egui::containers::menu::MenuConfig::new().style(popup_style))
    .ui(ui, |ui| layout(ui, MIN_WIDTH, content));
}
