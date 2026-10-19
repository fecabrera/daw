//! Idle audio preparation, resource diagnostics and optional-state pressure handling.
use super::*;
impl DawUi {
    pub(super) fn memory_tooltip(&self, ui: &mut egui::Ui) {
        let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
        ui.label("Estimated memory");
        ui.label(format!(
            "Live session model: {:.1} MiB",
            mib(daw_engine::prepared_model_bytes(
                &self.session.project,
                &self.session.audio,
                &self.session.stretched_audio,
                &self.session.sampler_audio
            ))
        ));
        ui.label(format!(
            "Shared audio and scratch: {:.1} MiB / 2048 MiB",
            mib(daw_media::audio_memory_bytes())
        ));
        if let Some(history) = &self.history {
            let m = history.memory_usage();
            ui.label(format!(
                "History current/pending/saved model: {:.1} MiB",
                mib(m.current_model_bytes + m.saved_model_bytes)
            ));
            ui.label(format!(
                "Undo/redo: {:.1} MiB / 128 MiB (model {:.1}, retained audio {:.1})",
                mib(m.optional_bytes()),
                mib(m.retained_model_bytes),
                mib(m.retained_audio_bytes)
            ));
        }
        let stats = self
            .output
            .as_ref()
            .map(|o| o.status.performance.snapshot())
            .or_else(|| {
                self.audio_preparation
                    .as_ref()
                    .map(|(_, _, p)| p.performance())
            });
        if let Some(stats) = stats {
            ui.label(format!(
                "Output renderers, queued and retired: {:.1} MiB",
                mib(stats.output_renderer_bytes)
            ));
            ui.label(format!(
                "Preparation to activation: maximum {:.1} ms",
                stats.max_request_to_activation_ns as f64 / 1_000_000.0
            ));
            let m = stats.cache_memory;
            ui.label(format!(
                "Preparation base: model {:.1}, schedule {:.1}, DSP {:.1} MiB",
                mib(m[0]),
                mib(m[1]),
                mib(m[2])
            ));
            ui.label(format!(
                "Checkpoints: {} / {:.1} MiB; warm cursor: {:.1} MiB",
                m[5],
                mib(m[3]),
                mib(m[4])
            ));
        }
        ui.label("Estimates exclude allocator, native-library and GPU overhead. Shared audio is counted once in its own category.");
    }
    pub(super) fn release_optional_memory_for_job(&mut self) {
        self.audio_preparation = None;
        self.audio_preparation_attempt = None;
        // Keep 256 MiB of the shared PCM budget available for essential preparation.
        if daw_media::audio_memory_bytes()
            > daw_media::AUDIO_MEMORY_BUDGET_BYTES - 256 * 1024 * 1024
            && let Some(history) = &mut self.history
        {
            history.release_optional_memory();
        }
    }
    pub(super) fn prepare_idle_audio(&mut self) {
        if self.output.is_some() || self.session.project.end() == 0 {
            self.audio_preparation = None;
            self.audio_preparation_attempt = None;
            return;
        }
        if self.job.is_some() {
            return;
        }
        let id = self.session.project.project_id;
        let revision = self.change_revision;
        if self
            .audio_preparation
            .as_ref()
            .is_none_or(|(old_id, old_revision, _)| *old_id != id || *old_revision != revision)
        {
            if let Some((old_id, old_revision, preparation)) = &mut self.audio_preparation {
                preparation.update_plan(self.session.plan());
                *old_id = id;
                *old_revision = revision;
            } else if self.audio_preparation_attempt != Some((id, revision)) {
                self.audio_preparation_attempt = Some((id, revision));
                // Worker failure is nonfatal: playback can prepare on demand.
                self.audio_preparation = daw_output::AudioPreparation::new(self.session.plan())
                    .ok()
                    .map(|p| (id, revision, p));
            }
        }
    }
}
