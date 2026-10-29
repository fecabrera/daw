//! Desktop source collection dispatch. Filesystem work stays in daw-project workers.
use super::*;
#[cfg(test)]
mod tests;
use daw_project::source_collection::{
    CollectionOutcome, CollectionPhase, CollectionPreflight, CollectionProgress, CollectionStatus,
    SourceCollectionSnapshot,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Checking,
    Review,
    Copying,
    Done,
}
struct Lease {
    current: AtomicBool,
    cancel: Arc<AtomicBool>,
}
pub(super) struct Status {
    ticket: Id,
    snapshot: SourceCollectionSnapshot,
    view: ViewMode,
    focus: Option<egui::Id>,
    navigation_revision: u64,
    lease: Arc<Lease>,
    stage: Stage,
    report: job_report::State,
    preflight: Option<CollectionPreflight>,
    progress: Option<CollectionProgress>,
    receiver: Option<mpsc::Receiver<CollectionProgress>>,
    detail: String,
    warnings: Vec<String>,
    recovery: bool,
    path: PathBuf,
}
impl Drop for Status {
    fn drop(&mut self) {
        self.lease.current.store(false, Ordering::Release);
        self.lease.cancel.store(true, Ordering::Relaxed);
    }
}
impl Status {
    fn show(&self, ctx: &egui::Context) -> Option<bool> {
        if self.stage == Stage::Review {
            let ready = self
                .preflight
                .as_ref()
                .is_some_and(CollectionPreflight::is_ready);
            let actions: &[(&str, bool)] = if ready {
                &[("Collect and save", true), ("Cancel", false)]
            } else {
                &[("Close", false)]
            };
            return dialogs::Dialog::new("Collect project sources", "", actions)
                .show_with_content(ctx, |ui| {
                    ui.add(egui::Label::new(&self.detail).wrap());
                    ui.label("Project folder");
                    ui.add(egui::Label::new(self.path.display().to_string()).wrap());
                    egui::ScrollArea::vertical()
                        .max_height(300.)
                        .show(ui, |ui| {
                            if let Some(report) = &self.preflight {
                                for diagnostic in report.diagnostics() {
                                    ui.colored_label(
                                        theme::palette(ctx).error,
                                        diagnostic.path.display().to_string(),
                                    );
                                    ui.add(egui::Label::new(&diagnostic.message).wrap());
                                    let owners = diagnostic
                                        .owners
                                        .iter()
                                        .map(|&i| &report.snapshot().owners()[i].asset_name)
                                        .cloned()
                                        .collect::<std::collections::BTreeSet<_>>()
                                        .into_iter()
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    ui.add(egui::Label::new(format!("Assets: {owners}")).wrap());
                                }
                            }
                        });
                })
                .or_else(|| {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                        .then_some(false)
                });
        }
        let phase = self.progress.map(|p| p.phase);
        let running = if self.stage == Stage::Checking {
            "Checking project sources…"
        } else {
            match phase {
                Some(CollectionPhase::Validating) => "Checking source revisions…",
                Some(CollectionPhase::Verifying) => "Verifying sources…",
                Some(CollectionPhase::Publishing) => "Saving collected project…",
                _ => "Copying project sources…",
            }
        };
        let progress = self.progress.map(|p| {
            let total = u128::from(p.total_staging_bytes) + u128::from(p.total_copy_bytes);
            let done = u128::from(p.staged_bytes) + u128::from(p.copied_bytes);
            if total == 0 {
                0.
            } else {
                done as f32 / total as f32
            }
        });
        let detail = if let Some(p) = self.progress {
            format!(
                "{}\nCopied {} of {} files · {} of {} bytes",
                self.detail, p.files_copied, p.total_files, p.copied_bytes, p.total_copy_bytes
            )
        } else {
            self.detail.clone()
        };
        job_report::Report {
            title: "Source-collection status",
            state: &self.report,
            running_text: running,
            complete_text: "Project sources collected and saved",
            failed_text: if self.recovery {
                "Collection needs recovery"
            } else {
                "Source collection failed"
            },
            detail: &detail,
            path: &self.path,
            progress,
            warnings: &self.warnings,
            cancel: Some(&self.lease.cancel),
        }
        .show(ctx)
        .then_some(false)
    }
}
impl DawUi {
    pub(super) fn collection_preserves_selection(&self) -> bool {
        self.collection_status.as_ref().is_some_and(|s| {
            s.snapshot.project_id() == self.session.project.project_id && s.view == self.view_mode
        })
    }
    pub(super) fn collection_enabled(&self) -> bool {
        self.session.folder.is_some()
            && self.collection_input_ready
            && self.track_drag.is_none()
            && self.loop_drag.is_none()
            && self.marquee.is_none()
            && self.portion_drag.is_none()
            && self.track_name_edit.is_none()
            && self.tempo_edit.is_none()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
            && self.file_hover.is_none()
            && !self.instrument_ui.editing_name()
            && !self.instrument_ui.has_layer_gesture()
            && !self.separation_models.busy()
    }
    pub(super) fn refresh_collection(&mut self) {
        if let Some(status) = &mut self.collection_status {
            if status.view != self.view_mode
                || status.navigation_revision != self.navigation_revision
                || !status.snapshot.is_current(
                    &self.session,
                    self.history.as_ref(),
                    self.change_revision,
                )
            {
                status.lease.current.store(false, Ordering::Release);
            }
            if let Some(rx) = &status.receiver {
                while let Ok(progress) = rx.try_recv() {
                    status.progress = Some(progress);
                }
            }
        }
    }
    pub(super) fn collect_sources(&mut self) {
        if !self.file_action_enabled(FileAction::CollectSources) {
            return;
        }
        // Release optional owners before capture. The prepared worker path must
        // not change history again after this immutable snapshot is captured.
        self.release_optional_memory_for_job();
        self.ensure_history();
        let snapshot = match self
            .session
            .source_collection_snapshot(self.history.as_ref(), self.change_revision)
        {
            Ok(snapshot) => snapshot,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let ticket = Id::new_v4();
        let lease = Arc::new(Lease {
            current: AtomicBool::new(true),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        self.collection_status = Some(Status {
            ticket,
            snapshot: snapshot.clone(),
            view: self.view_mode,
            focus: self.collection_focus,
            navigation_revision: self.navigation_revision,
            lease: lease.clone(),
            stage: Stage::Checking,
            report: job_report::State::Running,
            preflight: None,
            progress: None,
            receiver: None,
            detail:
                "Original recordings stay unchanged. Collection also saves the current project."
                    .into(),
            warnings: Vec::new(),
            recovery: false,
            path: self.session.folder.clone().unwrap(),
        });
        self.settings_cancel = Some(lease.cancel.clone());
        self.run_captured_job("Checking project sources…", move || {
            Ok(Job::CollectionPreflight(
                ticket,
                snapshot
                    .preflight(None, &|| {
                        lease.cancel.load(Ordering::Relaxed)
                            || !lease.current.load(Ordering::Acquire)
                    })
                    .map_err(|e| e.to_string()),
            ))
        });
    }
    pub(super) fn finish_collection_preflight(
        &mut self,
        ticket: Id,
        result: Result<CollectionPreflight, String>,
    ) {
        let Some(status) = &mut self.collection_status else {
            return;
        };
        if status.ticket != ticket || status.stage != Stage::Checking {
            return;
        }
        if !status.lease.current.load(Ordering::Acquire) {
            Self::collection_error(
                status,
                "Project or view changed during source discovery".into(),
            );
            return;
        }
        match result {
            Ok(report) if !status.lease.cancel.load(Ordering::Relaxed) => {
                let summary = report.summary();
                status.detail = format!(
                    "Files to copy: {} · Already included: {}\nAdditional bytes: {} · History-only sources: {}\n{}\nCollection also saves the current project. Original recordings stay unchanged. Temporary space is also required; free space was not measured.",
                    summary.files_to_copy,
                    summary.files_already_included,
                    summary.additional_bytes,
                    summary.history_only_sources,
                    if !report.is_ready() {
                        "Resolve the listed source problems before collecting."
                    } else if summary.files_to_copy == 0 {
                        "No new copies are required."
                    } else {
                        "All required sources are ready to collect."
                    }
                );
                status.preflight = Some(report);
                status.stage = Stage::Review;
            }
            Ok(_) => {
                status.report = job_report::State::Cancelled;
                status.stage = Stage::Done;
            }
            Err(e) => Self::collection_error(status, e),
        }
    }
    pub(super) fn collection_error(status: &mut Status, error: String) {
        status.stage = Stage::Done;
        status.report = if status.lease.cancel.load(Ordering::Relaxed)
            && status.lease.current.load(Ordering::Acquire)
        {
            job_report::State::Cancelled
        } else {
            job_report::State::Failed(error)
        };
    }
    pub(super) fn confirm_collection(&mut self) {
        if self.job.is_some()
            || self.regular_save.is_some()
            || self.error.is_some()
            || self.unsaved_prompt
            || self.settings_open
            || self.project_settings.is_some()
            || self.project_name_dialog.is_some()
            || self.export_dialog.is_some()
            || self.overwrite.is_some()
            || self.export_status.is_some()
            || self.separation_status.is_some()
        {
            return;
        }
        self.refresh_collection();
        let Some(status) = &mut self.collection_status else {
            return;
        };
        if status.stage != Stage::Review {
            return;
        }
        if !status.lease.current.load(Ordering::Acquire) || !self.collection_input_ready {
            Self::collection_error(
                status,
                "Project or view changed; check sources again before collecting".into(),
            );
            return;
        }
        if !status
            .preflight
            .as_ref()
            .is_some_and(CollectionPreflight::is_ready)
        {
            return;
        }
        let plan = match status.preflight.take().unwrap().into_plan() {
            Ok(plan) => plan,
            Err(e) => {
                Self::collection_error(status, e.to_string());
                return;
            }
        };
        let ticket = status.ticket;
        let lease = status.lease.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        status.receiver = Some(rx);
        status.stage = Stage::Copying;
        status.detail = "Original recordings stay unchanged.".into();
        self.settings_cancel = Some(lease.cancel.clone());
        self.run_captured_job("Collecting project sources…", move || {
            let result = plan
                .execute(
                    &|_| lease.current.load(Ordering::Acquire),
                    &|| lease.cancel.load(Ordering::Relaxed),
                    &mut |progress| {
                        let _ = tx.try_send(progress);
                        Ok(())
                    },
                )
                .map_err(|e| e.to_string());
            Ok(Job::Collected(ticket, result))
        });
    }
    pub(super) fn finish_collection(
        &mut self,
        ticket: Id,
        result: Result<CollectionOutcome, String>,
    ) {
        let Some(mut status) = self.collection_status.take() else {
            return;
        };
        if status.ticket != ticket || status.stage != Stage::Copying {
            self.collection_status = Some(status);
            return;
        }
        match result {
            Ok(outcome) => {
                status.progress = Some(outcome.progress());
                status.warnings = outcome.warnings().to_vec();
                status.stage = Stage::Done;
                if outcome.status() == CollectionStatus::RecoveryRequired {
                    status.recovery = true;
                    status.report = job_report::State::Failed(format!(
                        "Manifest commit could not be confirmed. Keep the project files and candidate manifest {} for recovery.",
                        outcome
                            .recovery_manifest()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    ));
                } else if !status.lease.current.load(Ordering::Acquire) {
                    status.recovery = true;
                    status.report=job_report::State::Failed("Sources were saved on disk, but the active project or view changed. Active state was retained. Reopen the saved project to use the collected sources.".into());
                } else {
                    match outcome.apply(
                        &mut self.session,
                        self.history.as_mut(),
                        self.change_revision,
                    ) {
                        Ok(()) => {
                            self.saved_navigation_revision = status.navigation_revision;
                            self.dirty = self.history.as_ref().is_some_and(|h| !h.is_saved())
                                || self.navigation_revision != self.saved_navigation_revision;
                            status.report = job_report::State::Complete;
                        }
                        Err(e) => {
                            status.recovery = true;
                            status.report = job_report::State::Failed(format!(
                                "Sources were saved on disk, but active state was retained: {e}. Reopen the saved project to use collected sources."
                            ));
                        }
                    }
                }
            }
            Err(e) => Self::collection_error(&mut status, e),
        }
        self.collection_status = Some(status);
    }
    pub(super) fn show_collection(&mut self, ctx: &egui::Context) {
        let action = self.collection_status.as_ref().and_then(|s| s.show(ctx));
        match action {
            Some(true) => self.confirm_collection(),
            Some(false) => {
                let restore = self.collection_preserves_selection();
                if let Some(status) = self.collection_status.take()
                    && restore
                    && let Some(focus) = status.focus
                {
                    crate::unit_power::schedule_focus_id(ctx, focus);
                }
            }
            None => {}
        }
    }
}
