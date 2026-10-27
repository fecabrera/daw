//! Application menus and view toolbar shared by Windows and Linux.

#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::toolbars;
use crate::{DawUi, icons, menus, theme};

fn submenu(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::containers::menu::SubMenuButton::from_button(
        egui::Button::new(label)
            .right_text(
                icons::Lucide::ChevronRight
                    .size(icons::TOOLBAR_ICON_SIZE)
                    .image(),
            )
            .image_tint_follows_text_color(true),
    )
    .config(egui::containers::menu::MenuConfig::new().style(menus::popup_style))
    .ui(ui, |ui| menus::layout(ui, menus::MIN_WIDTH, content));
}

#[cfg(test)]
mod tests;

impl DawUi {
    pub(super) fn application_menu(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let menu = ui
            .scope(|ui| {
                let widgets = &mut ui.visuals_mut().widgets;
                for visual in [
                    &mut widgets.noninteractive,
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                    &mut widgets.open,
                ] {
                    visual.bg_stroke = egui::Stroke::NONE;
                }
                icons::button(
                    ui,
                    icons::Lucide::Menu,
                    "Application menu",
                    true,
                    theme::palette(ui.ctx()).text,
                )
            })
            .inner;
        egui::Popup::menu(&menu)
            .style(menus::popup_style)
            .show(|ui| {
                menus::layout(ui, menus::MIN_WIDTH, |ui| {
                    submenu(ui, "File", |ui| self.file_menu(ui));
                    submenu(ui, "Edit", |ui| self.edit_menu(ui));
                    submenu(ui, "Clip", |ui| self.clip_menu(ui));
                    submenu(ui, "Track", |ui| self.track_menu(ui));
                    submenu(ui, "View", |ui| self.view_menu(ui));
                });
            });
        menu
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub(super) fn menu_view_toolbar(&mut self, ui: &mut egui::Ui) {
        toolbars::Toolbar::row(ui, |ui| {
            self.application_menu(ui);
            ui.separator();
            self.view_controls(ui);
        });
    }
}
