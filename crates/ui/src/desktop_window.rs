//! Shared contract for the secondary windows and native choosers in the desktop hosts.
use crate::{
    DawUi, SettingsPicker, dialogs,
    project_prompt::{ProjectPrompt, ProjectPromptKind},
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DesktopWindow {
    Settings(u64),
    Project(ProjectPrompt),
}

impl DesktopWindow {
    pub fn title(self) -> &'static str {
        match self {
            Self::Settings(_) => "Settings",
            Self::Project(prompt) => match prompt.kind {
                ProjectPromptKind::Unsaved => "Unsaved changes",
                ProjectPromptKind::Save => "Save project",
                ProjectPromptKind::SaveAs => "Save project as",
                ProjectPromptKind::Export => "Export audio",
                ProjectPromptKind::ReplaceExport => "Replace export?",
                ProjectPromptKind::ExportStatus => "Export status",
                ProjectPromptKind::StretchUpgrade => "Time stretch upgrade",
            },
        }
    }

    pub fn size(self) -> [u32; 2] {
        match self {
            Self::Settings(_) => [680, 510],
            Self::Project(prompt) => [
                352,
                match prompt.kind {
                    ProjectPromptKind::Unsaved | ProjectPromptKind::ReplaceExport => 64,
                    ProjectPromptKind::Save | ProjectPromptKind::SaveAs => 132,
                    ProjectPromptKind::Export => 168,
                    ProjectPromptKind::ExportStatus => 224,
                    ProjectPromptKind::StretchUpgrade => 128,
                },
            ],
        }
    }

    pub fn resizable(self) -> bool {
        matches!(self, Self::Settings(_))
    }
}

#[derive(Clone, Copy)]
pub enum DesktopPicker {
    Settings(SettingsPicker),
    ProjectLocation(ProjectPrompt),
    ExportFile(ProjectPrompt, daw_media::ExportFormat),
}

impl DesktopPicker {
    pub fn owner(self) -> DesktopWindow {
        match self {
            Self::Settings(request) => DesktopWindow::Settings(request.generation),
            Self::ProjectLocation(request) | Self::ExportFile(request, _) => {
                DesktopWindow::Project(request)
            }
        }
    }
}

impl DawUi {
    pub fn desktop_window(&self) -> Option<DesktopWindow> {
        self.native_project_prompt()
            .map(DesktopWindow::Project)
            .or_else(|| {
                self.native_settings_generation()
                    .map(DesktopWindow::Settings)
            })
    }

    pub fn desktop_window_size(&self, window: DesktopWindow) -> [u32; 2] {
        let [width, default_height] = window.size();
        let height = match window {
            DesktopWindow::Project(_) => {
                self.project_prompt_window.height.unwrap_or(default_height)
            }
            DesktopWindow::Settings(_) => default_height,
        };
        [width, height]
    }

    pub fn desktop_window_blocked(&self, window: DesktopWindow) -> bool {
        match window {
            DesktopWindow::Settings(_) => self.settings_root_prompt(),
            DesktopWindow::Project(_) => self.project_prompt_blocked(),
        }
    }

    pub fn take_desktop_focus(&mut self, window: DesktopWindow) -> bool {
        match window {
            DesktopWindow::Settings(_) => self.take_settings_focus(),
            DesktopWindow::Project(_) => {
                std::mem::take(&mut self.project_prompt_window.focus_requested)
            }
        }
    }

    pub fn fail_desktop_window(&mut self, window: DesktopWindow) {
        match window {
            DesktopWindow::Settings(generation) => self.fail_native_settings(generation),
            DesktopWindow::Project(prompt) => self.fail_native_project_prompt(prompt),
        }
    }

    pub fn close_desktop_window(&mut self, window: DesktopWindow) {
        if self.desktop_window() == Some(window) {
            match window {
                DesktopWindow::Settings(_) => self.close_application_settings(),
                DesktopWindow::Project(_) => self.cancel_project_prompt(),
            }
        }
    }

    pub fn show_desktop_window(
        &mut self,
        window: DesktopWindow,
        root: &egui::Context,
        ui: &mut egui::Ui,
    ) {
        if self.desktop_window() != Some(window) {
            return;
        }
        match window {
            DesktopWindow::Settings(_) => self.show_native_settings(root, ui),
            DesktopWindow::Project(_) => {
                let ctx = ui.ctx().clone();
                self.sync_settings_theme(&ctx);
                if ctx.input(|input| input.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.close_desktop_window(window);
                    root.request_repaint_of(egui::ViewportId::ROOT);
                    return;
                }
                let pending_picker = self.project_prompt_window.picking();
                let height =
                    dialogs::native_client(ui, |ui| self.draw_project_prompt(&ctx, Some(ui)));
                let resized = height > 0
                    && !pending_picker
                    && !self.project_prompt_window.picking()
                    && !self.project_prompt_blocked()
                    && self.desktop_window() == Some(window)
                    && self.project_prompt_window.height != Some(height);
                if resized {
                    self.project_prompt_window.height = Some(height);
                }
                if resized
                    || self.desktop_window() != Some(window)
                    || pending_picker != self.project_prompt_window.picking()
                    || self.job.is_some()
                    || self.error.is_some()
                {
                    root.request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        }
    }

    pub fn take_desktop_picker(&mut self) -> Option<DesktopPicker> {
        self.take_project_picker()
            .or_else(|| self.take_settings_picker().map(DesktopPicker::Settings))
    }

    pub fn complete_desktop_picker(
        &mut self,
        request: DesktopPicker,
        result: Result<Option<PathBuf>, String>,
    ) {
        match request {
            DesktopPicker::Settings(request) => self.complete_settings_picker(request, result),
            DesktopPicker::ProjectLocation(request) | DesktopPicker::ExportFile(request, _) => {
                self.complete_project_picker(request, result)
            }
        }
    }
}
