//! Editor revisions stay responsive while a complete playable revision is prepared.
use super::*;
#[derive(Default)]
pub(super) struct State {
    pub(super) worker: Option<daw_project::BackgroundAudio>,
    requested: Option<u64>,
    pub required: bool,
    pub running: bool,
    pub loading: bool,
    pub error: Option<String>,
}
impl DawUi {
    pub(super) fn media_ready(&self) -> bool {
        !self.background_media.required
    }
    pub(super) fn reset_background_media(&mut self) {
        self.background_media = State::default();
    }
    pub(super) fn queue_background_media(&mut self) {
        self.release_optional_memory_for_job();
        if self.background_media.worker.is_none() {
            match daw_project::BackgroundAudio::new() {
                Ok(worker) => self.background_media.worker = Some(worker),
                Err(error) => {
                    self.background_media.error = Some(error.to_string());
                    self.background_media.required = true;
                    return;
                }
            }
        }
        self.background_media
            .worker
            .as_ref()
            .unwrap()
            .submit(self.session.clone());
        self.background_media.requested = Some(self.change_revision);
        self.background_media.required = true;
        self.background_media.running = true;
        self.background_media.error = None;
    }
    pub(super) fn reconcile_background_media(&mut self) -> bool {
        if self.background_media.loading
            || self.session.audio_preparation_needed(&self.session.project)
        {
            if self.background_media.requested != Some(self.change_revision) {
                self.queue_background_media();
            }
            true
        } else {
            if self.background_media.required {
                if let Some(worker) = &self.background_media.worker {
                    worker.cancel();
                }
                self.background_media.required = false;
                self.background_media.running = false;
                self.background_media.error = None;
            }
            false
        }
    }
    pub(super) fn poll_background_media(&mut self) {
        if self.background_media.requested == Some(self.change_revision)
            && let Some(audio) = self
                .background_media
                .worker
                .as_ref()
                .and_then(daw_project::BackgroundAudio::poll_decoded)
        {
            self.session
                .audio
                .extend(audio.into_iter().filter(|(id, _)| {
                    self.session
                        .project
                        .assets
                        .iter()
                        .any(|asset| asset.id == *id && asset.midi_pattern.is_none())
                }));
        }
        let result = self
            .background_media
            .worker
            .as_ref()
            .and_then(daw_project::BackgroundAudio::poll);
        let Some((_, result)) = result else { return };
        if self.background_media.requested != Some(self.change_revision) {
            // The model changed since submission. Requeue its complete requirements.
            self.queue_background_media();
            return;
        }
        self.finish_background_media(result);
    }
    pub(super) fn finish_background_media(&mut self, result: daw_project::Result<Session>) {
        self.background_media.running = false;
        match result {
            Ok(session) => {
                // Merge derived data only. Save relocation, selection, playhead and
                // editor history belong to the current workspace, never this worker.
                self.session.audio = session.audio;
                self.session.stretched_audio = session.stretched_audio;
                self.session.sampler_audio = session.sampler_audio;
                for warning in session.warnings {
                    if !self.session.warnings.contains(&warning) {
                        self.session.warnings.push(warning.clone());
                        self.notices.push(warning);
                    }
                }
                self.background_media.loading = false;
                self.background_media.required = false;
                self.background_media.error = None;
                if let Some(history) = &mut self.history
                    && let Err(error) = history.refresh_audio(&self.session)
                {
                    self.fail(error);
                }
                self.sync_needed = true;
            }
            Err(error) => {
                self.background_media.error = Some(error.to_string());
            }
        }
    }
    pub(super) fn background_media_status(&mut self, ui: &mut egui::Ui) {
        if !self.background_media.required {
            return;
        }
        if self.background_media.running {
            ui.spinner();
            let (done, total) = self
                .background_media
                .worker
                .as_ref()
                .map_or((0, 0), daw_project::BackgroundAudio::progress);
            ui.small(if total == 0 {
                "Preparing project audio".into()
            } else {
                format!("Preparing project audio: {done}/{total}")
            });
            if ui.small_button("Cancel").clicked() {
                if let Some(worker) = &self.background_media.worker {
                    worker.cancel();
                }
                self.background_media.running = false;
            }
        } else {
            ui.small(
                self.background_media
                    .error
                    .as_deref()
                    .unwrap_or("Audio preparation paused"),
            );
            if ui.small_button("Retry").clicked() {
                self.queue_background_media();
            }
        }
        if self.is_playing() {
            ui.small("Playback uses the previous prepared version");
        }
    }
}
