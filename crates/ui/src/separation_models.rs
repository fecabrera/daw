//! Worker validation and publication of machine-local model selections.
use super::*;
use daw_project::{
    SeparationModelInfo, SeparationModelKind as Kind, SeparationModels, SeparationProgress,
};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub(super) struct State {
    pub validated: SeparationModels,
    pub info: HashMap<Kind, SeparationModelInfo>,
    queue: VecDeque<(Kind, PathBuf)>,
    pending: Option<Pending>,
}

struct Pending {
    kind: Kind,
    path: PathBuf,
    previous_path: Option<PathBuf>,
    progress: SeparationProgress,
}

impl State {
    pub fn busy(&self) -> bool {
        self.pending.is_some() || !self.queue.is_empty()
    }

    pub fn running(&self) -> bool {
        self.pending.is_some()
    }
}

impl DawUi {
    pub(super) fn revalidate_separation_models(&mut self) {
        if let Some(pending) = &self.separation_models.pending {
            pending.progress.cancel.store(true, Ordering::Relaxed);
        }
        self.separation_models.queue.clear();
        for kind in Kind::ALL {
            let path = self.settings.separation_models.path(kind);
            if self.separation_models.validated.path(kind) != path {
                self.separation_models.validated.set(kind, None);
                self.separation_models.info.remove(&kind);
            }
            if kind.available()
                && self.separation_models.validated.path(kind).is_none()
                && let Some(path) = path
            {
                self.separation_models
                    .queue
                    .push_back((kind, path.to_owned()));
            }
        }
    }

    pub(super) fn change_separation_model(&mut self, action: settings::ModelAction) {
        if self.job.is_some() || self.separation_models.busy() || self.error.is_some() {
            return;
        }
        let settings::ModelAction { kind, path } = action;
        if !kind.available() {
            return;
        }
        if let Some(path) = path {
            // Keep the accepted path visible and persisted until this one passes.
            self.separation_models.queue.push_back((kind, path));
        } else {
            std::sync::Arc::make_mut(&mut self.settings.separation_models).set(kind, None);
            self.separation_models.validated.set(kind, None);
            self.separation_models.info.remove(&kind);
        }
    }

    pub(super) fn start_next_model_validation(&mut self) {
        if self.job.is_some()
            || self.regular_save.is_some()
            || self.error.is_some()
            || self.export_status.is_some()
            || self.separation_status.is_some()
            || self.collection_status.is_some()
            || self.project_settings.is_some()
            || self.project_name_dialog.is_some()
            || self.unsaved_prompt
            || self.overwrite.is_some()
            || self.export_dialog.is_some()
        {
            return;
        }
        if let Some((kind, path)) = self.separation_models.queue.pop_front() {
            self.start_model_validation(kind, path, |kind, path, progress| {
                daw_project::validate_separation_model(kind, path, progress)
                    .map_err(|e| e.to_string())
            });
        }
    }

    pub(super) fn start_model_validation(
        &mut self,
        kind: Kind,
        path: PathBuf,
        validate: impl FnOnce(
            Kind,
            &std::path::Path,
            &SeparationProgress,
        ) -> Result<SeparationModelInfo, String>
        + Send
        + 'static,
    ) {
        let progress = SeparationProgress::default();
        self.separation_models.pending = Some(Pending {
            kind,
            path: path.clone(),
            previous_path: self
                .settings
                .separation_models
                .path(kind)
                .map(PathBuf::from),
            progress: progress.clone(),
        });
        let worker_progress = progress.clone();
        self.run_job(&format!("Validating {}…", kind.name()), move || {
            Ok(Job::ModelValidated(validate(kind, &path, &worker_progress)))
        });
        self.settings_cancel = Some(progress.cancel);
    }

    pub(super) fn finish_model_validation(&mut self, result: Result<SeparationModelInfo, String>) {
        let Some(pending) = self.separation_models.pending.take() else {
            return;
        };
        // A programmatic settings replacement can supersede an active check.
        if self.settings.separation_models.path(pending.kind) != pending.previous_path.as_deref() {
            return;
        }
        let cancelled = pending.progress.cancel.load(Ordering::Relaxed);
        if let Ok(info) = result.as_ref()
            && !cancelled
        {
            std::sync::Arc::make_mut(&mut self.settings.separation_models)
                .set(pending.kind, Some(pending.path.clone()));
            self.separation_models
                .validated
                .set(pending.kind, Some(pending.path));
            self.separation_models.info.insert(pending.kind, *info);
        } else {
            // A file can change in place after its earlier validation.
            if result.is_err()
                && !cancelled
                && self.separation_models.validated.path(pending.kind)
                    == Some(pending.path.as_path())
            {
                self.separation_models.validated.set(pending.kind, None);
                self.separation_models.info.remove(&pending.kind);
            }
            // A failed startup check rejects the saved path too. Validation
            // stamps are session-only, so stored settings cannot bypass a check.
            let previous = self
                .separation_models
                .validated
                .path(pending.kind)
                .map(PathBuf::from);
            std::sync::Arc::make_mut(&mut self.settings.separation_models)
                .set(pending.kind, previous);
            if !cancelled && let Err(error) = result {
                self.fail(format!(
                    "{} validation failed: {error}",
                    pending.kind.name()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_info() -> SeparationModelInfo {
        SeparationModelInfo {
            stems: daw_project::SeparationStemSet::Six,
            file_size_bytes: 258_000_000,
        }
    }

    fn finish(app: &mut DawUi) {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.job.is_some() || app.separation_models.busy() {
            app.poll();
            assert!(
                std::time::Instant::now() < deadline,
                "Model validation timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn accepts_each_model_only_after_validation_and_rejects_replacements() {
        let mut app = DawUi::default();
        let before = app.session.project.clone();
        for kind in Kind::ALL.into_iter().filter(|kind| kind.available()) {
            let path = PathBuf::from(format!("accepted-{kind:?}.onnx"));
            let (release, wait) = mpsc::channel();
            app.start_model_validation(kind, path.clone(), move |_, _, _| wait.recv().unwrap());
            assert!(app.settings.separation_models.path(kind).is_none());
            assert!(app.separation_models.validated.path(kind).is_none());
            assert!(!app.separation_models.info.contains_key(&kind));
            assert!(!app.separation_enabled());
            assert!(!app.settings().to_json().contains(path.to_str().unwrap()));
            assert!(release.send(Ok(test_info())).is_ok());
            finish(&mut app);
            assert_eq!(
                app.settings.separation_models.path(kind),
                Some(path.as_path())
            );
            assert_eq!(
                app.separation_models.validated.path(kind),
                Some(path.as_path())
            );
            assert!(app.separation_enabled());
            assert_eq!(app.separation_models.info.get(&kind), Some(&test_info()));

            app.start_model_validation(kind, PathBuf::from("invalid.onnx"), |_, _, _| {
                Err("wrong tensor contract".into())
            });
            finish(&mut app);
            assert_eq!(
                app.settings.separation_models.path(kind),
                Some(path.as_path())
            );
            assert_eq!(
                app.separation_models.validated.path(kind),
                Some(path.as_path())
            );
            assert_eq!(app.separation_models.info.get(&kind), Some(&test_info()));
            assert!(!app.settings().to_json().contains("invalid.onnx"));
            assert!(app.error.take().unwrap().contains("validation failed"));
            assert!(app.separation_enabled());
        }
        for kind in Kind::ALL.into_iter().filter(|kind| kind.available()) {
            app.change_separation_model(settings::ModelAction { kind, path: None });
            assert!(app.settings.separation_models.path(kind).is_none());
            assert!(app.separation_models.validated.path(kind).is_none());
            assert!(!app.separation_models.info.contains_key(&kind));
            assert_eq!(
                app.separation_enabled(),
                app.separation_models.validated.is_configured()
            );
        }
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }

    #[test]
    fn failed_recheck_rejects_a_previously_validated_path() {
        let mut app = DawUi::default();
        let path = PathBuf::from("replaced-in-place.onnx");
        app.start_model_validation(Kind::Cpu, path.clone(), |_, _, _| Ok(test_info()));
        finish(&mut app);
        app.start_model_validation(Kind::Cpu, path, |_, _, _| Err("damaged model".into()));
        finish(&mut app);
        assert!(app.settings.separation_models.cpu.is_none());
        assert!(app.separation_models.validated.cpu.is_none());
        assert!(app.separation_models.info.is_empty());
        assert!(!app.separation_enabled());
        assert!(app.error.take().unwrap().contains("damaged model"));
        assert!(!app.dirty);
    }

    #[test]
    fn saved_paths_are_revalidated_and_invalid_paths_are_rejected() {
        if !daw_project::SEPARATION_AVAILABLE {
            return;
        }
        let mut app = DawUi::default();
        let mut settings = app.settings();
        for kind in Kind::ALL.into_iter().filter(|kind| kind.available()) {
            std::sync::Arc::make_mut(&mut settings.separation_models).set(
                kind,
                Some(std::env::temp_dir().join(format!("missing-model-{}.onnx", Id::new_v4()))),
            );
        }
        let ctx = egui::Context::default();
        app.set_settings(&ctx, settings);
        assert!(!app.separation_enabled());
        assert!(!app.separation_models.validated.is_configured());
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut failures = 0;
        while app.job.is_some() || app.separation_models.busy() {
            app.poll();
            failures += usize::from(app.error.take().is_some());
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            failures,
            Kind::ALL
                .into_iter()
                .filter(|kind| kind.available())
                .count()
        );
        assert!(!app.settings.separation_models.is_configured());
        assert!(!app.separation_enabled());
        assert!(!app.dirty);
    }

    #[test]
    fn superseded_worker_cannot_accept_an_old_candidate() {
        if !Kind::Cpu.available() {
            return;
        }
        let mut app = DawUi::default();
        let (release, wait) = mpsc::channel();
        app.start_model_validation(
            Kind::Cpu,
            PathBuf::from("old-candidate.onnx"),
            move |_, _, _| wait.recv().unwrap(),
        );
        let model = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/charon-audio/tests/fixtures/identity_4stems.onnx");
        let mut settings = app.settings();
        std::sync::Arc::make_mut(&mut settings.separation_models).cpu = Some(model.clone());
        app.set_settings(&egui::Context::default(), settings);
        assert!(release.send(Ok(test_info())).is_ok());
        finish(&mut app);
        assert_eq!(app.separation_models.validated.cpu, Some(model));
        assert_eq!(
            app.separation_models.info[&Kind::Cpu].stems,
            daw_project::SeparationStemSet::Four
        );
        assert!(app.separation_enabled());
        assert!(app.error.is_none());
        assert!(!app.dirty);
    }
}
