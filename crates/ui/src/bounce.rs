//! Bounce commands and modal lifetime. Worker completion is not publication.
use super::*;
use daw_core::{BounceMode, BounceRequest};
use daw_project::{BounceError, BounceProgress, PreparedBounce, SessionHistory};
pub(super) struct Status {
    pub(super) progress: BounceProgress,
    pub state: job_report::State,
    pub publishing: bool,
    cleaning: bool,
    focus: Option<egui::Id>,
    detail: String,
    context: egui::Context,
}
pub(super) struct Pending {
    prepared: PreparedBounce,
    history: SessionHistory,
    revision: Option<u64>,
}
impl DawUi {
    fn bounce_request(&self, mode: BounceMode, ids: Option<Vec<Id>>) -> BounceRequest {
        BounceRequest {
            mode,
            ids: ids.unwrap_or_else(|| match mode {
                BounceMode::Clips => self.selected_clip_ids(),
                BounceMode::Tracks => self
                    .session
                    .project
                    .tracks
                    .iter()
                    .filter(|t| self.track_selected(t.id))
                    .map(|t| t.id)
                    .collect(),
            }),
        }
    }
    pub(super) fn bounce_targets_enabled(&self, mode: BounceMode, ids: Vec<Id>) -> bool {
        self.editing_enabled()
            && self.media_ready()
            && self.regular_save.is_none()
            && self.file_hover.is_none()
            && !self.track_browser.has_gesture()
            && !self.instrument_ui.has_layer_gesture()
            && (mode != BounceMode::Clips
                || (self.portion.is_none() && self.view_mode.has_clip_lanes()))
            && self
                .session
                .project
                .bounce_targets(&BounceRequest { mode, ids })
                .is_ok()
    }
    pub(super) fn bounce_request_enabled(&self, mode: BounceMode) -> bool {
        self.bounce_targets_enabled(mode, self.bounce_request(mode, None).ids)
    }
    pub(super) fn start_bounce(
        &mut self,
        mode: BounceMode,
        ids: Option<Vec<Id>>,
        ctx: &egui::Context,
    ) {
        let request = self.bounce_request(mode, ids);
        if !self.bounce_targets_enabled(mode, request.ids.clone()) {
            return;
        }
        self.ensure_history();
        let Some(mut history) = self.history.clone() else {
            return;
        };
        let progress = BounceProgress::default();
        let worker = progress.clone();
        let session = self.session.clone();
        // Install the lifetime guard before starting work. It survives self.job.
        self.bounce_status = Some(Status {
            progress: progress.clone(),
            state: job_report::State::Running,
            publishing: false,
            cleaning: false,
            focus: ctx.memory(|m| m.focused()),
            context: ctx.clone(),
            detail: format!(
                "{} tracks · WAV · Float32 · Stereo · {} kHz",
                self.session
                    .project
                    .bounce_targets(&request)
                    .map_or(0, |s| s.targets.len()),
                self.session.project.sample_rate_hz as f32 / 1000.0
            ),
        });
        self.run_job("Bouncing audio…", move || {
            let result = session.bounce(&request, &worker).and_then(|prepared| {
                if let Err(error) = history.record(&prepared.session) {
                    let cleanup = prepared
                        .discard()
                        .err()
                        .map(|e| format!("\n{e}"))
                        .unwrap_or_default();
                    return Err(BounceError::Failed(format!("{error}{cleanup}")));
                }
                Ok((prepared, history))
            });
            Ok(Job::Bounced(Box::new(result)))
        });
        self.settings_cancel = Some(progress.cancel.clone());
    }
    pub(super) fn finish_bounce(
        &mut self,
        result: Result<(PreparedBounce, SessionHistory), BounceError>,
    ) {
        let Some(status) = &self.bounce_status else {
            return;
        };
        let previous_error = if let job_report::State::Failed(error) = &status.state {
            Some(error.clone())
        } else {
            None
        };
        let (prepared, history) = match result {
            Err(BounceError::Cancelled) => {
                if let Some(error) = previous_error {
                    self.fail_bounce(error);
                } else {
                    self.dismiss_bounce();
                }
                return;
            }
            Err(BounceError::Failed(error)) => {
                self.fail_bounce(error);
                return;
            }
            Ok(prepared) => prepared,
        };
        if let Some(error) = previous_error {
            self.discard_bounce(prepared, Some(error));
            return;
        }
        if status.progress.cancel.load(Ordering::Acquire) {
            self.discard_bounce(prepared, None);
            return;
        }
        let mut expected = prepared.source_project.clone();
        expected.transport = self.session.project.transport.clone();
        expected.display = self.session.project.display;
        if expected != self.session.project {
            self.discard_bounce(
                prepared,
                Some("Bounce source changed before publication".into()),
            );
            return;
        }
        let mut pending = Pending {
            prepared,
            history,
            revision: None,
        };
        pending.prepared.session.project.transport = self.session.project.transport.clone();
        pending.prepared.session.project.display = self.session.project.display;
        // This control-thread acceptance point is serialized with Cancel. Once
        // accepted, only publication/error remains; Cancel is no longer offered.
        if let Some(output) = &mut self.output {
            if let Err(error) = output.sync_reconstructed(pending.prepared.session.plan()) {
                self.discard_bounce(pending.prepared, Some(error));
                return;
            }
            pending.revision = Some(output.requested_revision());
        }
        self.sync_needed = false;
        self.bounce_status.as_mut().unwrap().publishing = true;
        self.bounce_pending = Some(pending);
        self.poll_bounce_publication();
    }
    pub(super) fn poll_bounce_publication(&mut self) {
        if let Some(job_report::State::Failed(error)) =
            self.bounce_status.as_ref().map(|s| &s.state)
        {
            let error = error.clone();
            if let Some(pending) = self.bounce_pending.take() {
                self.discard_bounce(pending.prepared, Some(error));
            }
            return;
        }
        let Some(pending) = &self.bounce_pending else {
            return;
        };
        if let Some(revision) = pending.revision {
            let Some(output) = &self.output else {
                self.fail_bounce("Audio output disappeared during bounce publication".into());
                return;
            };
            if output.status.failed.load(Ordering::Acquire)
                || output.status.source_failed.load(Ordering::Acquire)
            {
                self.fail_bounce("Audio output failed during bounce publication".into());
                return;
            }
            if output.applied_revision() < revision || !output.current_plan_published() {
                return;
            }
        }
        let mut pending = self.bounce_pending.take().unwrap();
        pending.prepared.session.project.transport = self.session.project.transport.clone();
        pending.prepared.session.project.display = self.session.project.display;
        self.notices
            .extend(pending.prepared.warnings.iter().cloned());
        self.session = pending.prepared.session;
        self.history = Some(pending.history);
        self.dirty = true;
        self.change_revision = self.change_revision.wrapping_add(1);
        self.repaint_until = Some(std::time::Instant::now() + Duration::from_millis(250));
        self.dismiss_bounce();
    }
    pub(super) fn fail_bounce(&mut self, error: String) {
        if let Some(pending) = self.bounce_pending.take() {
            if let Some(output) = &mut self.output {
                let _ = output.sync_reconstructed(self.session.plan());
            }
            self.discard_bounce(pending.prepared, Some(error));
            return;
        }
        if let Some(status) = &mut self.bounce_status {
            if self.job.is_some() {
                status.progress.cancel.store(true, Ordering::Release);
            }
            status.state = job_report::State::Failed(error);
            status.publishing = false;
        }
    }
    fn discard_bounce(&mut self, prepared: PreparedBounce, error: Option<String>) {
        if let Some(status) = &mut self.bounce_status {
            status.state = job_report::State::Running;
            status.cleaning = true;
            status.publishing = false;
        }
        self.run_captured_job("Cleaning bounce…", move || {
            let cleanup = prepared.discard().err().map(|e| e.to_string());
            let error = match (error, cleanup) {
                (Some(error), Some(cleanup)) => Some(format!("{error}\n{cleanup}")),
                (error, None) => error,
                (None, cleanup) => cleanup,
            };
            Ok(Job::BounceCleaned(error))
        });
    }
    pub(super) fn finish_bounce_cleanup(&mut self, error: Option<String>) {
        if let Some(error) = error {
            if let Some(status) = &mut self.bounce_status {
                status.cleaning = false;
                status.state = job_report::State::Failed(error);
            }
        } else {
            self.dismiss_bounce();
        }
    }
    fn dismiss_bounce(&mut self) {
        if let Some(status) = self.bounce_status.take() {
            self.bounce_restore_focus = status.focus;
            status.context.input(|input| {
                self.bounce_suppressed_keys
                    .extend(input.keys_down.iter().copied());
                self.bounce_block_pointer = input.pointer.any_down();
            });
        }
        self.bounce_pending = None;
        // Protect the completion frame. Held keys are suppressed individually on
        // subsequent frames; they must never keep the whole workspace disabled.
        self.bounce_input_blocked = true;
    }
    pub(super) fn update_bounce_input_guard(&mut self, ctx: &egui::Context) {
        if let Some(focus) = self.bounce_restore_focus.take() {
            ctx.memory_mut(|m| m.request_focus(focus));
        }
        ctx.input_mut(|input| {
            input.events.retain(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: false,
                    ..
                } => {
                    self.bounce_suppressed_keys.remove(key);
                    true
                }
                egui::Event::Key {
                    key, pressed: true, ..
                } => !self.bounce_suppressed_keys.contains(key),
                _ => true,
            });
            self.bounce_suppressed_keys
                .retain(|key| input.keys_down.contains(key));
            // A held Cancel click must finish over the disabled workspace. New
            // pointer gestures after completion remain available immediately.
            self.bounce_block_pointer &= input.pointer.any_down() || input.pointer.any_released();
        });
        self.bounce_input_blocked = self.bounce_block_pointer;
    }
    pub(super) fn show_bounce(&mut self, ctx: &egui::Context) {
        let Some(status) = &self.bounce_status else {
            return;
        };
        let total = status.progress.total.load(Ordering::Acquire);
        let done = status.progress.done.load(Ordering::Acquire);
        let cancelling = status.progress.cancel.load(Ordering::Acquire);
        let text = if status.cleaning {
            "Cleaning bounce…"
        } else if status.publishing {
            "Publishing bounce…"
        } else if cancelling {
            "Cancelling bounce…"
        } else if total == 0 {
            "Preparing bounce…"
        } else if done >= total {
            "Preparing bounced tracks…"
        } else {
            "Rendering bounce…"
        };
        // An output error can precede the worker's cancellation acknowledgement.
        // Keep the dialog non-dismissable until that worker has released its files.
        let draining_error =
            self.job.is_some() && matches!(status.state, job_report::State::Failed(_));
        let running = job_report::State::Running;
        let detail = if draining_error {
            format!(
                "{}\nWaiting for bounce cleanup…",
                match &status.state {
                    job_report::State::Failed(e) => e.as_str(),
                    _ => "",
                }
            )
        } else {
            status.detail.clone()
        };
        let close = job_report::Report {
            title: "Bounce",
            state: if draining_error {
                &running
            } else {
                &status.state
            },
            running_text: text,
            complete_text: "Bounce complete",
            failed_text: "Bounce failed",
            detail: &detail,
            path: None,
            progress: (total > 0).then_some(done as f32 / total.max(1) as f32),
            warnings: &[],
            cancel: (!status.publishing && !status.cleaning && !draining_error)
                .then_some(&status.progress.cancel),
        }
        .show(ctx);
        if close {
            self.dismiss_bounce();
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
    pub(super) fn bounce_shortcuts(&mut self, ctx: &egui::Context) {
        if self.native_menu {
            return;
        }
        // Consume shifted B first, even when ineligible. Exact matching prevents
        // the unshifted command from interpreting a disabled track command.
        let mut command = None;
        ctx.input_mut(|i| {
            i.events.retain(|event| {
                if let egui::Event::Key {
                    key: egui::Key::B,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                {
                    for (mode, required) in [
                        (
                            BounceMode::Tracks,
                            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                        ),
                        (BounceMode::Clips, egui::Modifiers::COMMAND),
                    ] {
                        if modifiers.matches_exact(required) {
                            if !repeat {
                                command = Some(mode);
                            }
                            return false;
                        }
                    }
                }
                true
            })
        });
        if let Some(mode) = command {
            let enabled = match mode {
                BounceMode::Clips => self.clip_action_enabled(ClipAction::Bounce, ctx),
                BounceMode::Tracks => self.track_action_enabled(TrackAction::Bounce, ctx),
            };
            if enabled && !ctx.input(|i| i.pointer.any_down()) {
                self.start_bounce(mode, None, ctx);
            }
        }
    }
}
