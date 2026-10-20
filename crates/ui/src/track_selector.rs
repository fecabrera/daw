use super::*;
use daw_core::TrackKind;

impl DawUi {
    pub(super) fn track_control(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(
            !self.midi_editor.dragging()
                && !self.instrument_ui.dragging()
                && self.effects_drag.is_none()
                && self.component_drag.is_none(),
            |ui| {
                let tracks: Vec<_> = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .filter(|t| self.view_mode != ViewMode::Instrument || t.kind == TrackKind::Midi)
                    .filter(|t| self.view_mode != ViewMode::Edit || t.kind != TrackKind::Group)
                    .map(|t| (t.id, t.name.clone()))
                    .collect();
                // Editor follows the active clip when Instrument or Timeline has selected another track.
                let current = if self.view_mode == ViewMode::Edit {
                    self.session
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.clips.iter().any(|c| Some(c.id) == self.selected_clip))
                        .map(|t| t.id)
                        .or(self.selected_track)
                } else {
                    self.selected_track
                }
                .filter(|id| tracks.iter().any(|t| t.0 == *id));
                let prompt = if self.view_mode == ViewMode::Instrument {
                    "Select MIDI track"
                } else {
                    "Select track"
                };
                let label = current
                    .and_then(|id| tracks.iter().find(|t| t.0 == id))
                    .map_or(prompt, |t| t.1.as_str());
                let mut selected = current;
                egui::ComboBox::from_id_salt("source-track")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (id, name) in &tracks {
                            ui.selectable_value(&mut selected, Some(*id), name);
                        }
                    });
                if selected != current
                    && let Some(id) = selected
                {
                    self.select_track(id, false);
                    if self.view_mode == ViewMode::Edit {
                        let clip = self
                            .session
                            .project
                            .tracks
                            .iter()
                            .find(|t| t.id == id)
                            .and_then(|t| t.clips.iter().min_by_key(|c| c.start_frame))
                            .map(|c| c.id);
                        if let Some(clip) = clip {
                            self.select_clip(clip, id, false);
                        } else {
                            self.selected_clip = None;
                            self.selected_clips.clear();
                        }
                        self.midi_editor.cancel();
                    }
                }
            },
        );
    }
}
