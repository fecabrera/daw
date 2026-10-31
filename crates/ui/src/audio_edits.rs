//! Shared worker transaction for clip transforms and instrument sample preparation.
use super::*;
impl DawUi {
    /// MIDI edits enter logical history immediately; source/transform transactions
    /// retain their existing complete-session publication while audio is ready.
    pub(super) fn prepare_audio_edit(
        &mut self,
        project: &daw_core::Project,
        placement: Option<(Id, Vec<Id>)>,
    ) -> bool {
        if !self.session.audio_preparation_needed(project) {
            return false;
        }
        if self.background_media.required || self.session.midi_only_preparation_change(project) {
            self.session.project = project.clone();
            if let Some((track, clips)) = placement {
                self.select_track(track, false);
                self.selected_clip = clips.first().copied();
                self.selected_clips = clips.into_iter().collect();
            }
            self.changed();
            self.queue_background_media();
            return true;
        }
        let mut session = self.session.clone();
        session.project = project.clone();
        let layer_selection = self
            .instrument_ui
            .track
            .zip(self.instrument_ui.selected_layer)
            .filter(|(track, layer)| {
                project
                    .tracks
                    .iter()
                    .find(|t| t.id == *track)
                    .and_then(|t| t.instrument.as_ref())
                    .is_some_and(|i| i.main.id == *layer || i.layers.iter().any(|l| l.id == *layer))
            })
            .map(|(track, layer)| (track, layer, self.instrument_ui.selected_layers.clone()));
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = cancel.clone();
        self.run_job("Preparing audio", move || {
            session
                .prepare_stretches_cancellable(&|| flag.load(std::sync::atomic::Ordering::Relaxed))
                .map_err(|e| e.to_string())?;
            Ok(Job::PreparedEdit {
                session,
                placement,
                layer_selection,
            })
        });
        self.settings_cancel = Some(cancel);
        true
    }
}
