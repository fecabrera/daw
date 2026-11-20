//! One logical Application Settings session shared by the desktop surfaces.
use crate::{
    Action, DawUi, PluginRequest, dialogs, midi_service,
    settings::{self, PickerKind},
};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsPicker {
    pub generation: u64,
    pub kind: PickerKind,
}

#[derive(Default)]
pub(super) struct SettingsWindow {
    host: bool,
    fallback: bool,
    retiring: bool,
    generation: u64,
    pub focus_requested: bool,
    navigation: settings::Navigation,
    folder_draft: crate::plugin_settings::FolderDraft,
    quality_draft: Option<daw_core::AudioQuality>,
    picker: Option<SettingsPicker>,
    picker_started: bool,
    close_after_picker: bool,
    root_close: bool,
    message: Option<String>,
}

impl SettingsWindow {
    pub fn begin(&mut self) {
        self.retiring = false;
        self.generation = self.generation.wrapping_add(1);
        self.fallback = false;
        self.focus_requested = true;
        self.message = None;
    }

    pub fn take_root_close(&mut self) -> bool {
        self.picker.is_none() && std::mem::take(&mut self.root_close)
    }
}

impl DawUi {
    pub fn enable_native_windows(&mut self) {
        self.settings_window.host = true;
    }

    pub(super) fn native_desktop_windows_enabled(&self) -> bool {
        self.settings_window.host
    }

    pub fn application_settings_open(&self) -> bool {
        self.settings_open
    }

    pub(super) fn settings_modal(&self) -> bool {
        self.settings_open || self.settings_window.retiring
    }

    pub fn finish_settings_retirement(&mut self) {
        self.settings_window.retiring = false;
    }

    pub(super) fn sync_settings_theme(&self, ctx: &egui::Context) {
        if self.settings_window.host {
            self.themes.sync_settings_surface(ctx, &self.settings);
        }
    }

    pub fn native_settings_visible(&self) -> bool {
        self.settings_open && self.settings_window.host && !self.settings_window.fallback
    }

    pub fn native_settings_generation(&self) -> Option<u64> {
        self.native_settings_visible()
            .then_some(self.settings_window.generation)
    }

    pub fn settings_root_prompt(&self) -> bool {
        self.midi.recovery_prompt()
            || self.project_prompt_kind().is_some()
            || self.diagnostic_details.is_some()
    }

    pub fn take_settings_focus(&mut self) -> bool {
        std::mem::take(&mut self.settings_window.focus_requested)
    }

    pub fn fail_native_settings(&mut self, generation: u64) {
        if self.native_settings_generation() == Some(generation) {
            self.settings_window.fallback = true;
            self.settings_window.message =
                Some("The native Settings window could not open.".into());
        }
    }

    pub fn take_settings_picker(&mut self) -> Option<SettingsPicker> {
        let picker = self.settings_window.picker?;
        if std::mem::replace(&mut self.settings_window.picker_started, true) {
            return None;
        }
        Some(picker)
    }

    pub fn complete_settings_picker(
        &mut self,
        request: SettingsPicker,
        result: Result<Option<PathBuf>, String>,
    ) {
        if self.settings_window.picker != Some(request) {
            return;
        }
        self.settings_window.picker = None;
        if self.settings_window.root_close {
            self.settings_window.close_after_picker = false;
            return;
        }
        if std::mem::take(&mut self.settings_window.close_after_picker) {
            self.close_application_settings();
            return;
        }
        match result {
            Ok(Some(path)) => match request.kind {
                PickerKind::Model(kind) => self.change_separation_model(settings::ModelAction {
                    kind,
                    path: Some(path),
                }),
                PickerKind::PluginFolder => {
                    self.settings_window.folder_draft.path = path.to_string_lossy().into_owned()
                }
            },
            Ok(None) => {}
            Err(message) => self.settings_window.message = Some(message),
        }
        self.settings_window.focus_requested = true;
    }

    pub fn close_application_settings(&mut self) {
        if self.settings_window.picker.is_some() {
            self.settings_window.close_after_picker = true;
            return;
        }
        self.settings_window.retiring |= self.native_settings_visible();
        self.settings_open = false;
        self.settings_window.quality_draft = None;
        self.settings_window.close_after_picker = false;
        self.settings_window.focus_requested = false;
    }

    pub fn request_desktop_close(&mut self) {
        if self.native_desktop_windows_enabled() && self.project_prompt_kind().is_some() {
            if !self.project_prompt_window.picking() {
                self.project_prompt_window.focus_requested = true;
            }
            return;
        }
        if self.settings_window.picker.is_some() {
            self.settings_window.root_close = true;
        } else if self.job.is_none()
            && self.regular_save.is_none()
            && self.collection_status.is_none()
            && self.bounce_status.is_none()
            && !self.bounce_input_blocked
        {
            self.project_name_dialog = None;
            self.close_application_settings();
            self.project_settings = None;
            self.request(Action::CloseWindow);
        }
    }

    pub fn show_native_settings(&mut self, root: &egui::Context, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.themes.sync_settings_surface(&ctx, &self.settings);
        ui.painter()
            .rect_filled(ctx.content_rect(), 0.0, crate::theme::palette(&ctx).panel);
        if ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_application_settings();
            root.request_repaint_of(egui::ViewportId::ROOT);
        }
        if !self.native_settings_visible() {
            return;
        }
        if self.settings_root_prompt() {
            return;
        }
        if ctx.input_mut(|input| input.consume_shortcut(&Self::settings_shortcut())) {
            self.open_settings();
        }
        let before = self.settings.clone();
        self.draw_application_settings(root, Some(ui));
        if self.settings != before || !self.settings_open || self.settings_window.picker.is_some() {
            root.request_repaint_of(egui::ViewportId::ROOT);
        }
    }

    pub(super) fn draw_application_settings(
        &mut self,
        root: &egui::Context,
        mut surface: Option<&mut egui::Ui>,
    ) {
        let ctx = surface
            .as_ref()
            .map_or_else(|| root.clone(), |ui| ui.ctx().clone());
        let native = surface.is_some();
        if self.settings_window.picker.is_some() {
            if let Some(ui) = surface.as_mut() {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.label("Choosing a file…");
                });
            }
            return;
        }
        if native && let Some(error) = self.error.clone() {
            if dialogs::Dialog::new("Error", &error, &[("OK", ())])
                .show(&ctx)
                .is_some()
            {
                self.error = None;
                root.request_repaint_of(egui::ViewportId::ROOT);
            }
            return;
        }
        let mut model_action = None;
        let enabled = self.job.is_none()
            && !self.separation_models.busy()
            && self.error.is_none()
            && self.settings_window.picker.is_none();
        let mut audio = settings::AudioControls {
            host_picker: self.settings_window.host,
            navigation: Some(&mut self.settings_window.navigation),
            folder_draft: Some(&mut self.settings_window.folder_draft),
            plugins: Some(&self.hosting.scan),
            themes: Some(&self.themes),
            midi_devices: Some(&self.midi.devices),
            midi_enabled: enabled && !self.midi.busy(),
            midi_connected: self.midi.connected,
            midi_take_retained: self.midi.take_retained,
            devices: self.audio_devices.catalog.as_ref(),
            quality_draft: self.settings_window.quality_draft,
            error: self.audio_devices.error.as_deref(),
            enabled: enabled && !self.audio_devices.busy(),
            ..Default::default()
        };
        let close = settings::show_surface(
            &ctx,
            surface,
            &mut self.settings,
            &self.separation_models.validated,
            &self.separation_models.info,
            enabled,
            &mut model_action,
            &mut audio,
        );
        let picker = audio.picker;
        if audio.plugin_cancel_requested {
            self.hosting.requests.push(PluginRequest::CancelScan);
        }
        if let Some(mode) = audio.plugin_action.take() {
            self.hosting.requests.push(PluginRequest::Scan(mode));
        }
        let midi_panic_requested = audio.midi_panic_requested;
        let midi_retry_requested = audio.midi_retry_requested;
        let selection = audio.selection.take();
        self.settings_window.quality_draft = audio.quality_draft;
        let refresh_requested = audio.refresh_requested;
        if let Some(action) = audio.theme_action {
            self.themes.start(root, action);
        }
        if midi_retry_requested {
            self.midi.finalizing = true;
            self.midi_send(midi_service::Command::Retry);
        }
        if midi_panic_requested && let Some(service) = &self.midi.service {
            service.panic();
        }
        if let Some(draft) = selection {
            self.apply_audio_device_selection(draft);
        }
        if refresh_requested {
            self.refresh_audio_devices();
        }
        if let Some(action) = model_action {
            self.change_separation_model(action);
        }
        if let Some(kind) = picker {
            self.settings_window.picker = Some(SettingsPicker {
                generation: self.settings_window.generation,
                kind,
            });
            self.settings_window.picker_started = false;
        }
        if close && self.error.is_none() {
            self.close_application_settings();
        }
        if native
            && self.separation_models.running()
            && let Some(cancel) = &self.settings_cancel
            && dialogs::Dialog::new("Model validation", &self.busy, &[("Cancel", ())])
                .actions_enabled(!cancel.load(Ordering::Relaxed))
                .show(&ctx)
                .is_some()
        {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(message) = self.settings_window.message.as_deref()
            && dialogs::Dialog::new("Settings window", message, &[("OK", ())])
                .show(&ctx)
                .is_some()
        {
            self.settings_window.message = None;
        }
        if self.audio_devices.busy()
            || self.themes.busy()
            || self.separation_models.busy()
            || self.hosting.scan.scanning
            || self.job.is_some()
            || self.midi.busy()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_client_keeps_sidebar_and_close_inside_the_minimum_size() {
        let ctx = egui::Context::default();
        let root = egui::Context::default();
        let mut daw = DawUi::default();
        daw.enable_native_windows();
        daw.open_settings();
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 320.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| daw.show_native_settings(&root, ui),
        );
        output.textures_delta.clear();
        let padding = f32::from(crate::dialogs::PADDING);
        let footer_top = bounds.bottom() - crate::toolbars::CONTROL_HEIGHT - 2.0 * padding;
        for label in ["Appearance", "Plugins", "Theme", "Close"] {
            let text = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.text() == label
                    {
                        Some(text.galley.rect.translate(text.pos.to_vec2()))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("Missing Settings control {label}"));
            assert!(
                bounds.contains_rect(text),
                "{label} is outside the Settings client area"
            );
            if label == "Close" {
                assert!(
                    (text.center().y - (footer_top + bounds.bottom()) * 0.5).abs() <= 2.0,
                    "Close must be vertically centered in the footer"
                );
            }
        }
        assert!(
            output.shapes.iter().any(|shape| {
                if let egui::Shape::LineSegment { points, .. } = &shape.shape {
                    (points[0].y - footer_top - 0.5).abs() <= 1.0
                        && (points[0].y - points[1].y).abs() <= 0.5
                        && points[0].x <= bounds.left() + 1.0
                        && points[1].x >= bounds.right() - 1.0
                } else {
                    false
                }
            }),
            "the footer divider must span the bottom panel"
        );
    }

    #[test]
    fn settings_fallback_and_native_retirement_keep_the_edit_guard() {
        let mut ui = DawUi::default();
        ui.enable_native_windows();
        ui.open_settings();
        let generation = ui.native_settings_generation().unwrap();
        assert!(!ui.edit_environment_available());
        assert!(ui.take_settings_focus());
        ui.open_settings();
        assert!(ui.take_settings_focus());
        assert_eq!(ui.native_settings_generation(), Some(generation));
        ui.fail_native_settings(generation.wrapping_add(1));
        assert_eq!(ui.native_settings_generation(), Some(generation));
        ui.close_application_settings();
        assert!(!ui.edit_environment_available());
        ui.finish_settings_retirement();
        assert!(ui.edit_environment_available());
        ui.open_settings();
        let generation = ui.native_settings_generation().unwrap();
        ui.fail_native_settings(generation);
        assert!(ui.application_settings_open());
        assert!(ui.native_settings_generation().is_none());
        assert!(!ui.edit_environment_available());
        ui.close_application_settings();
        assert!(ui.edit_environment_available());
    }

    #[test]
    fn chooser_results_keep_the_draft_and_cannot_reopen_retired_settings() {
        let mut ui = DawUi::default();
        ui.enable_native_windows();
        ui.open_settings();
        let request = SettingsPicker {
            generation: ui.native_settings_generation().unwrap(),
            kind: PickerKind::PluginFolder,
        };
        ui.settings_window.picker = Some(request);
        assert_eq!(ui.take_settings_picker(), Some(request));
        assert!(ui.take_settings_picker().is_none());
        ui.complete_settings_picker(request, Ok(Some(PathBuf::from("/plugin-fixture"))));
        assert_eq!(ui.settings_window.folder_draft.path, "/plugin-fixture");
        assert!(ui.settings.plugin_folders.is_empty());
        ui.settings_window.picker = Some(request);
        ui.close_application_settings();
        assert!(ui.application_settings_open());
        ui.complete_settings_picker(request, Ok(Some(PathBuf::from("/stale-fixture"))));
        assert!(!ui.application_settings_open());
        assert_eq!(ui.settings_window.folder_draft.path, "/plugin-fixture");
        ui.complete_settings_picker(request, Ok(Some(PathBuf::from("/retired-fixture"))));
        assert_eq!(ui.settings_window.folder_draft.path, "/plugin-fixture");
    }
}
