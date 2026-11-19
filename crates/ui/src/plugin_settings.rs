//! Machine-local plugin discovery controls in Application Settings.
use crate::settings::{AppSettings, AudioControls};
use daw_plugin_host::{PluginFormat, ScanMode, SearchRoot};
#[derive(Clone, Default)]
pub(super) struct ScanStatus {
    pub roots: Vec<SearchRoot>,
    pub scanning: bool,
    pub available: bool,
    pub error: Option<String>,
    pub inventory: Vec<(String, String, String)>,
}
#[derive(Clone)]
pub(super) struct FolderDraft {
    format: PluginFormat,
    pub(super) path: String,
}
impl Default for FolderDraft {
    fn default() -> Self {
        Self {
            format: PluginFormat::Vst3,
            path: String::new(),
        }
    }
}
pub(super) fn show(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    controls: &mut AudioControls<'_>,
) {
    let status = controls.plugins;
    let scanning = status.is_some_and(|s| s.scanning);
    let available = status.is_some_and(|s| s.available);
    ui.label("Default folders are scanned at startup. Additional folders apply on this computer.");
    ui.horizontal_wrapped(|ui| {
        for (label, mode) in [
            ("Scan folders", ScanMode::Changed),
            ("Rescan all", ScanMode::Rescan),
            ("Retry failed", ScanMode::RetryFailed),
        ] {
            if ui
                .add_enabled(available && !scanning, egui::Button::new(label))
                .clicked()
            {
                controls.plugin_action = Some(mode);
            }
        }
        if scanning && ui.button("Cancel scan").clicked() {
            controls.plugin_cancel_requested = true;
        }
        if scanning {
            ui.spinner();
            ui.label("Scanning…");
        }
    });
    if !available {
        ui.label("Native plugin services are unavailable.");
    }
    if let Some(error) = status.and_then(|s| s.error.as_ref()) {
        ui.label(error);
    }
    ui.separator();
    ui.strong("Default folders");
    if let Some(status) = status {
        for root in &status.roots {
            ui.label(format!(
                "{} · {}",
                root.format.extension().to_ascii_uppercase(),
                root.path.display()
            ));
        }
    }
    ui.separator();
    ui.strong("Additional folders");
    let mut remove = None;
    for (index, root) in settings.plugin_folders.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!scanning, egui::Button::new("Remove"))
                .clicked()
            {
                remove = Some(index);
            }
            ui.label(format!(
                "{} · {}",
                root.format.extension().to_ascii_uppercase(),
                root.path.display()
            ));
        });
    }
    if let Some(index) = remove {
        settings.plugin_folders.remove(index);
    }
    let key = egui::Id::new("plugin-folder-draft");
    let mut draft = controls
        .folder_draft
        .as_deref()
        .cloned()
        .unwrap_or_else(|| {
            ui.ctx()
                .data(|d| d.get_temp::<FolderDraft>(key))
                .unwrap_or_default()
        });
    ui.add_enabled_ui(!scanning, |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("plugin-folder-format")
                .selected_text(draft.format.extension().to_ascii_uppercase())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.format, PluginFormat::Vst3, "VST3");
                    ui.selectable_value(&mut draft.format, PluginFormat::Clap, "CLAP");
                });
            if ui.button("Browse…").clicked() {
                if controls.host_picker {
                    controls.picker = Some(crate::settings::PickerKind::PluginFolder);
                } else if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    draft.path = path.to_string_lossy().into_owned();
                }
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut draft.path)
                .hint_text("Absolute plugin folder path")
                .desired_width(ui.available_width()),
        );
        let path = std::path::PathBuf::from(draft.path.trim());
        let root = SearchRoot {
            format: draft.format,
            path,
        };
        let valid = root.path.is_absolute()
            && !settings.plugin_folders.contains(&root)
            && settings.plugin_folders.len() < 64;
        if ui
            .add_enabled(valid, egui::Button::new("Add folder"))
            .clicked()
        {
            settings.plugin_folders.push(root);
            draft.path.clear();
        }
    });
    if let Some(state) = controls.folder_draft.as_deref_mut() {
        *state = draft;
    } else {
        ui.ctx().data_mut(|d| d.insert_temp(key, draft));
    }
    ui.separator();
    ui.strong("Plugin status");
    if let Some(status) = status {
        ui.label(format!("{} catalog entries", status.inventory.len()));
        egui::ScrollArea::vertical()
            .id_salt("plugin-status-list")
            .max_height(180.0)
            .show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y,
                status.inventory.len(),
                |ui, rows| {
                    for index in rows {
                        let (name, state, path) = &status.inventory[index];
                        ui.add(egui::Label::new(format!("{name} · {state}")).truncate())
                            .on_hover_text(format!("{path}\n{state}"));
                    }
                },
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        settings: &mut AppSettings,
        status: &ScanStatus,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, Option<ScanMode>, bool) {
        let mut controls = AudioControls {
            plugins: Some(status),
            ..Default::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(680.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| show(ui, settings, &mut controls),
        );
        output.textures_delta.clear();
        (
            output,
            controls.plugin_action,
            controls.plugin_cancel_requested,
        )
    }
    fn click(
        ctx: &egui::Context,
        settings: &mut AppSettings,
        status: &ScanStatus,
        label: &str,
    ) -> (Option<ScanMode>, bool) {
        frame(ctx, settings, status, vec![]);
        let output = frame(ctx, settings, status, vec![]).0;
        let position = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing {label}"));
        let event = |pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            ctx,
            settings,
            status,
            vec![egui::Event::PointerMoved(position), event(true)],
        );
        let (_, action, cancel) = frame(ctx, settings, status, vec![event(false)]);
        (action, cancel)
    }
    #[test]
    fn scan_and_cancel_are_explicit_actions_and_do_not_change_folder_preferences() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings::default();
        let before = settings.clone();
        let mut status = ScanStatus {
            available: true,
            ..Default::default()
        };
        assert_eq!(
            click(&ctx, &mut settings, &status, "Rescan all"),
            (Some(ScanMode::Rescan), false)
        );
        status.scanning = true;
        assert_eq!(
            click(&ctx, &mut settings, &status, "Cancel scan"),
            (None, true)
        );
        assert_eq!(settings, before);
    }
    #[test]
    fn adding_a_folder_uses_the_selected_format_and_retains_the_other_preferences() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let path = std::env::temp_dir().join("owned-plugin-folder");
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("plugin-folder-draft"),
                FolderDraft {
                    format: PluginFormat::Clap,
                    path: path.display().to_string(),
                },
            )
        });
        let mut settings = AppSettings::default();
        let before = settings.clone();
        let status = ScanStatus {
            available: true,
            ..Default::default()
        };
        click(&ctx, &mut settings, &status, "Add folder");
        assert_eq!(
            settings.plugin_folders,
            vec![SearchRoot {
                format: PluginFormat::Clap,
                path
            }]
        );
        settings.plugin_folders.clear();
        assert_eq!(settings, before);
    }
}
