use super::*;
use daw_project::SeparationProgress;
pub(super) struct Status {
    pub progress: SeparationProgress,
    pub state: job_report::State,
    pub warnings: Vec<String>,
    path: PathBuf,
}
impl Status {
    pub fn show(&self, ctx: &egui::Context) -> bool {
        use std::sync::atomic::Ordering;
        let total = self.progress.total.load(Ordering::Relaxed);
        let done = self.progress.done.load(Ordering::Relaxed);
        job_report::Report {
            title: "Stem-separation status",
            state: &self.state,
            running_text: if total == 0 {
                "Preparing audio and loading model (CoreML may compile)…"
            } else if done >= total {
                "Preparing stem tracks…"
            } else {
                "Separating audio…"
            },
            complete_text: "Stem separation complete",
            failed_text: "Stem separation failed",
            detail: "Charon · HTDemucs · Stereo · Drums, Bass, Other, Vocals",
            path: &self.path,
            progress: (total > 0).then_some(done as f32 / total.max(1) as f32),
            warnings: &self.warnings,
            cancel: Some(&self.progress.cancel),
        }
        .show(ctx)
    }
}
impl DawUi {
    pub(super) fn separate_clip(&mut self, clip: Id) {
        if !self.file_action_enabled(FileAction::Import) || self.separation_status.is_some() {
            return;
        }
        let progress = SeparationProgress::default();
        let session = self.session.clone();
        let models = self.settings.separation_models.clone();
        let worker_progress = progress.clone();
        self.run_job("Separating stems…", move || {
            session
                .separate_clip(clip, &models, &worker_progress)
                .map(Job::Separated)
                .map_err(|e| e.to_string())
        });
        self.settings_cancel = Some(progress.cancel.clone());
        self.separation_status = Some(Status {
            progress,
            state: job_report::State::Running,
            warnings: Vec::new(),
            path: self
                .session
                .folder
                .clone()
                .unwrap_or_default()
                .join("stems"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_stays_in_status_and_guards_project_actions() {
        let mut app = DawUi::default();
        app.session.folder = Some(std::env::temp_dir());
        app.settings.separation_models = std::sync::Arc::new(daw_project::SeparationModels {
            cpu: Some(std::env::temp_dir().join("daw-missing-test-model.onnx")),
            coreml: None,
        });
        let before = app.session.project.clone();
        app.separate_clip(Id::new_v4());
        assert!(app.separation_status.is_some());
        assert!(!app.file_action_enabled(FileAction::Save));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            app.poll();
            if app.job.is_none() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Separation job timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.job.is_none());
        assert!(matches!(
            app.separation_status.as_ref().unwrap().state,
            job_report::State::Failed(_)
        ));
        assert!(app.error.is_none());
        assert_eq!(app.session.project, before);
        assert!(!app.file_action_enabled(FileAction::Open));
        app.separation_status = None;
        assert!(app.file_action_enabled(FileAction::Open));
    }
    #[test]
    fn model_paths_round_trip_and_legacy_settings_need_no_models() {
        let mut settings = settings::AppSettings::default();
        std::sync::Arc::make_mut(&mut settings.separation_models).cpu =
            Some(PathBuf::from("models/cpu.onnx"));
        std::sync::Arc::make_mut(&mut settings.separation_models).coreml =
            Some(PathBuf::from("models/split.onnx"));
        assert_eq!(
            settings::AppSettings::from_json(Some(&settings.to_json())),
            settings
        );
        assert!(
            settings::AppSettings::from_json(Some("{} "))
                .separation_models
                .cpu
                .is_none()
        );
    }
}
