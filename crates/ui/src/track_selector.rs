use super::*;
use daw_core::TrackKind;

impl DawUi {
    pub(super) fn editor_track(&self) -> Option<&daw_core::Track> {
        if self.timeline_editor_visible() {
            let track_id = self
                .beat_sample_editor_context()
                .map_or_else(|| self.track_context(), |(track, _)| Some(track));
            return self.session.project.tracks.iter().find(|track| {
                Some(track.id) == track_id
                    && matches!(
                        track.kind,
                        TrackKind::Audio | TrackKind::Midi | TrackKind::Beat
                    )
            });
        }
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
                    .find(|t| Some(t.id) == self.track_context())
            })
    }

    pub(super) fn active_midi_track(&self) -> Option<Id> {
        let track = if self.view_mode.has_track_overview() {
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

    /// Editor navigation selects the earliest clip and clears a stale clip on empty tracks.
    pub(super) fn select_editor_track(&mut self, id: Id) {
        self.select_track(id, false);
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
