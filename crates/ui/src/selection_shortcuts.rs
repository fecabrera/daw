//! Shared Escape handling for arrangement, instrument layers and beat subtracks.
use super::*;

impl DawUi {
    pub(super) fn selection_shortcut_environment_available(&self, ctx: &egui::Context) -> bool {
        self.edit_shortcuts_available(ctx)
            && self.track_name_edit.is_none()
            && self.tempo_edit.is_none()
            && !self.time_signature_control.active()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
            && self.file_hover.is_none()
    }

    pub(super) fn selection_escape_available(&self, ctx: &egui::Context) -> bool {
        self.selection_shortcut_environment_available(ctx)
            && !self.track_browser.has_gesture()
            && !self.instrument_ui.has_layer_gesture()
            && !strip_reorder::dragging(ctx)
    }

    pub(super) fn selection_escape(&mut self, ctx: &egui::Context) {
        let beat_editor = self.view_mode == ViewMode::Edit
            && self
                .editor_track()
                .is_some_and(|track| track.kind == daw_core::TrackKind::Beat);
        if !(matches!(self.view_mode, ViewMode::Timeline | ViewMode::Instrument) || beat_editor)
            || !self.selection_escape_available(ctx)
            || ctx.text_edit_focused()
            || egui::Popup::is_any_open(ctx)
            || (self.view_mode == ViewMode::Timeline
                && self.clip_settings_visible
                && (self.clip_settings_draft.is_some()
                    || self.clip_settings_change.is_some()
                    || self.clip_transpose_drag.is_some()))
            || ctx.input(|input| input.pointer.any_down())
            || self
                .selection_text_focus
                .is_some_and(|id| ctx.memory(|memory| memory.had_focus_last_frame(id)))
        {
            return;
        }
        // Observe before pane widgets consume Escape. Leave the event available for
        // catalog deselection, while text editors and active gestures keep precedence.
        if !ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(event,
                egui::Event::Key { key: egui::Key::Escape, pressed: true,
                    repeat: false, modifiers, .. } if modifiers.is_none())
            })
        }) {
            return;
        }
        if self.view_mode == ViewMode::Instrument {
            self.deselect_beat_subtracks();
            self.clear_layer_selections_except(None);
        } else if beat_editor {
            self.deselect_beat_subtracks();
        } else if self.selected_clip.is_some()
            || !self.selected_clips.is_empty()
            || self.portion.is_some()
        {
            self.selected_clip = None;
            self.selected_clips.clear();
            self.portion = None;
        } else {
            self.selected_track = None;
            self.selected_tracks.clear();
            self.master_selected = false;
        }
        ctx.request_repaint();
    }
}
