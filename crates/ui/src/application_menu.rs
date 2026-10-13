//! Application menus and view toolbar shared by Windows and Linux.

#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::toolbars;
use crate::{DawUi, icons, theme};

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
        egui::Popup::menu(&menu).show(|ui| {
            ui.menu_button("File", |ui| self.file_menu(ui));
            ui.menu_button("Edit", |ui| self.edit_menu(ui));
            ui.menu_button("Track", |ui| self.track_menu(ui));
            ui.menu_button("View", |ui| self.view_menu(ui));
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
