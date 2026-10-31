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
    Ready,
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
    fn show_error(&self, ctx: &egui::Context) -> bool {
        if !matches!(self.report, job_report::State::Failed(_)) {
            return false;
        }
        job_report::Report {
            title: "Collect project sources",
            state: &self.report,
            running_text: "",
            complete_text: "",
            failed_text: if self.recovery {
                "Collection needs recovery"
            } else {
                "Source collection failed"
            },
            detail: &self.detail,
            path: &self.path,
            progress: None,
            warnings: &self.warnings,
            cancel: None,
        }
        .show(ctx)
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
            && self.media_ready()
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
        self.collection_message = None;
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
            self.update_collection_message();
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
                if report.is_ready() {
                    status.preflight = Some(report);
                    status.stage = Stage::Ready;
                } else {
                    for diagnostic in report.diagnostics() {
                        let owners = diagnostic
                            .owners
                            .iter()
                            .map(|&i| report.snapshot().owners()[i].asset_name.as_str())
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join(", ");
                        status.detail.push_str(&format!(
                            "\n\n{}\n{}\nAssets: {owners}",
                            diagnostic.path.display(),
                            diagnostic.message
                        ));
                    }
                    Self::collection_error(
                        status,
                        "Required sources could not be verified. No sources were collected.".into(),
                    );
                }
            }
            Ok(_) => {
                status.report = job_report::State::Cancelled;
                status.stage = Stage::Done;
            }
            Err(e) => Self::collection_error(status, e),
        }
        self.start_collection();
        self.update_collection_message();
    }
    pub(super) fn collection_error(status: &mut Status, error: String) {
        status.stage = Stage::Done;
        status.report = if status.lease.cancel.load(Ordering::Relaxed)
            && status.lease.current.load(Ordering::Acquire)
        {
            // Worker errors cross this boundary as text. Preserve the backend's
            // cleanup diagnostics even when ordinary cancellation needs no dialog.
            if error.contains("; cleanup warnings: ") {
                status.warnings.push(error);
            }
            job_report::State::Cancelled
        } else {
            job_report::State::Failed(error)
        };
    }
    fn start_collection(&mut self) {
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
            if let Some(status) = &mut self.collection_status {
                Self::collection_error(status, "Source collection could not continue because another operation or dialog is active.".into());
            }
            return;
        }
        self.refresh_collection();
        let Some(status) = &mut self.collection_status else {
            return;
        };
        if status.stage != Stage::Ready {
            return;
        }
        if !status.lease.current.load(Ordering::Acquire) {
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
        self.update_collection_message();
    }
    pub(super) fn update_collection_message(&mut self) {
        let Some(status) = &self.collection_status else {
            return;
        };
        if matches!(
            status.report,
            job_report::State::Complete | job_report::State::Cancelled
        ) {
            for warning in &status.warnings {
                if !self.notices.contains(warning) {
                    self.notices.push(warning.clone());
                }
            }
        }
        self.collection_message = match &status.report {
            job_report::State::Complete => {
                let progress = status.progress.unwrap();
                Some(format!(
                    "Project sources collected and saved · {} files · {} bytes",
                    progress.files_copied, progress.copied_bytes
                ))
            }
            job_report::State::Cancelled => Some("Source collection cancelled".into()),
            job_report::State::Failed(_) => Some("Source collection failed".into()),
            job_report::State::Running => None,
        };
    }
    pub(super) fn collection_status_text(&self) -> Option<String> {
        self.collection_status
            .as_ref()
            .filter(|s| s.report == job_report::State::Running)
            .map(|s| {
                if s.lease.cancel.load(Ordering::Relaxed) {
                    return "Cancelling source collection…".into();
                }
                let phase = match s.progress.map(|p| p.phase) {
                    Some(CollectionPhase::Validating) => "Checking source revisions…",
                    Some(CollectionPhase::Verifying) => "Verifying sources…",
                    Some(CollectionPhase::Publishing) => "Saving collected project…",
                    _ if s.stage == Stage::Checking => "Checking project sources…",
                    _ => "Copying project sources…",
                };
                if let Some(p) = s.progress {
                    format!(
                        "{phase} {} of {} files · {} of {} bytes",
                        p.files_copied, p.total_files, p.copied_bytes, p.total_copy_bytes
                    )
                } else {
                    phase.into()
                }
            })
            .or_else(|| self.collection_message.clone())
    }
    pub(super) fn show_collection(&mut self, ctx: &egui::Context) {
        let dismiss = self.collection_status.as_ref().is_some_and(|s| {
            s.stage == Stage::Done
                && (!matches!(s.report, job_report::State::Failed(_)) || s.show_error(ctx))
        });
        if dismiss {
            let restore = self.collection_preserves_selection();
            if let Some(status) = self.collection_status.take()
                && restore
                && let Some(focus) = status.focus
            {
                crate::unit_power::schedule_focus_id(ctx, focus);
            }
        }
    }
}
