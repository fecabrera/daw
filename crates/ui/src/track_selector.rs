use super::*;
use daw_core::TrackKind;

impl DawUi {
    pub(super) fn editor_track(&self) -> Option<&daw_core::Track> {
        self.session
            .project
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| Some(c.id) == self.selected_clip))
            .or_else(|| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|t| Some(t.id) == self.selected_track)
            })
    }

    pub(super) fn active_midi_track(&self) -> Option<Id> {
        let track = if self.view_mode == ViewMode::Edit {
            self.editor_track()
        } else {
            self.session
                .project
                .tracks
                .iter()
                .find(|track| Some(track.id) == self.selected_track)
        };
        track
            .filter(|track| track.kind == TrackKind::Midi)
            .map(|track| track.id)
    }

    pub(super) fn track_control(&mut self, ui: &mut egui::Ui) {
        self.track_dropdown(ui, None);
    }

    pub(super) fn track_dropdown(
        &mut self,
        ui: &mut egui::Ui,
        width: Option<f32>,
    ) -> egui::Response {
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
                    self.editor_track().map(|t| t.id)
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
                let mut combo =
                    crate::menus::Dropdown::from_id_salt("source-track").selected_text(label);
                if let Some(width) = width {
                    combo = combo.width(width).truncate();
                }
                let response = combo.show_ui(ui, |ui| {
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
                response.response
            },
        )
        .inner
    }
}
