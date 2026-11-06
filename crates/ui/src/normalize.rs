//! Normalization options and status-bar-only worker lifecycle.
use super::*;
use daw_project::{NormalizeMode, NormalizeOptions, NormalizeProgress};

pub(super) struct Draft {
    clips: Vec<Id>,
    options: NormalizeOptions,
    target: String,
}
impl Draft {
    pub(super) fn new(clips: Vec<Id>) -> Self {
        Self {
            clips,
            options: NormalizeOptions::default(),
            target: "-1.0".into(),
        }
    }
    fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        let mut action = None;
        let message = format!(
            "Normalize {} audio clip(s) individually. Adjusts dry clip Gain before effects. Source files stay unchanged.",
            self.clips.len()
        );
        dialogs::Dialog::<bool>::new("Normalize gain", &message, &[]).show_with_content(ctx, |ui| {
            let before = self.options.mode;
            egui::Grid::new("normalize-options").show(ui, |ui| {
                ui.label("Measure");
                menus::Dropdown::from_id_salt("normalize-mode").selected_text(self.options.mode.label()).show_ui(ui, |ui| {
                    for mode in NormalizeMode::ALL {
                        ui.selectable_value(&mut self.options.mode, mode, mode.label());
                    }
                });
                ui.end_row();
                if before != self.options.mode { self.target = format!("{:.1}", self.options.mode.default_target()); }
                ui.label(format!("Target ({})", self.options.mode.unit()));
                ui.add(egui::TextEdit::singleline(&mut self.target).desired_width(90.0));
                ui.end_row();
            });
            ui.checkbox(&mut self.options.allow_increase, "Allow gain increase");
            if self.options.mode == NormalizeMode::Integrated {
                ui.label("Measures integrated loudness. Clips need at least 400 ms. Gain changes do not limit peaks.");
            }
            self.options.target = self.target.trim().replace('−', "-").parse().unwrap_or(f32::NAN);
            let valid = self.options.validate();
            if let Err(error) = &valid { ui.label(error.to_string()); }
            rows::wrapped(ui, toolbars::CONTROL_HEIGHT, |ui| {
                if ui.add_enabled(valid.is_ok(), egui::Button::new("Normalize")).clicked() { action = Some(true); }
                if ui.button("Cancel").clicked() { action = Some(false); }
            });
        });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            action = Some(false);
        }
        action
    }
}
impl DawUi {
    pub(super) fn normalization_targets_are_audio(&self, ids: &[Id]) -> bool {
        !ids.is_empty()
            && ids.iter().all(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .find(|c| c.id == *id)
                    .is_some_and(|c| {
                        self.session
                            .project
                            .assets
                            .iter()
                            .any(|a| a.id == c.asset_id && a.is_audio())
                    })
            })
    }
    pub(super) fn show_normalization(&mut self, ctx: &egui::Context) {
        if let Some(mut draft) = self.normalize_dialog.take() {
            match draft.show(ctx) {
                Some(true) => self.start_normalization(draft.clips, draft.options),
                Some(false) => (),
                None => self.normalize_dialog = Some(draft),
            }
        }
    }
    pub(super) fn start_normalization(&mut self, clips: Vec<Id>, options: NormalizeOptions) {
        let session = self.session.clone();
        let progress = NormalizeProgress::default();
        let worker_progress = progress.clone();
        self.run_job("Normalizing gain…", move || {
            session
                .normalize_clips(&clips, options, &worker_progress)
                .map(Job::Normalized)
                .map_err(|e| e.to_string())
        });
        self.settings_cancel = Some(progress.cancel.clone());
        self.normalize_progress = Some(progress);
    }
    pub(super) fn normalization_status_text(&self) -> Option<String> {
        let progress = self.normalize_progress.as_ref()?;
        if progress.cancel.load(Ordering::Relaxed) {
            return Some("Cancelling normalization…".into());
        }
        let done = progress.done.load(Ordering::Relaxed);
        let total = progress.total.load(Ordering::Relaxed);
        Some(if total == 0 {
            "Preparing normalization…".into()
        } else {
            format!(
                "Normalizing gain… {:.0}%",
                100.0 * done as f64 / total as f64
            )
        })
    }
    pub(super) fn finish_normalization(&mut self, result: Result<Vec<Edit>, String>) {
        let Some(progress) = self.normalize_progress.take() else {
            return;
        };
        if progress.cancel.load(Ordering::Relaxed) {
            return;
        }
        match result {
            Ok(edits) if !edits.is_empty() => self.edit(Edit::Batch(edits)),
            Ok(_) => (),
            Err(error) => self.fail(error),
        }
    }
}
