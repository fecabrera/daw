//! Editor adapters for the shared whole-clip rectangle selection gesture.
use super::*;

impl DawUi {
    pub(super) fn editor_clip_marquee(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        lane: Rect,
        track: Id,
    ) {
        let active = self
            .marquee
            .as_ref()
            .is_some_and(|marquee| marquee.pane == response.id);
        if !active {
            if ui
                .input(|input| input.pointer.press_origin())
                .is_some_and(|origin| {
                    self.session
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.id == track)
                        .is_some_and(|track| self.editor_repeat_at(track, lane, origin))
                })
            {
                return;
            }
            if !response.drag_started()
                || ui.input(|input| input.modifiers.command)
                || self.portion_drag.is_some()
            {
                return;
            }
            if ui
                .input(|input| input.pointer.press_origin())
                .is_some_and(|pos| {
                    self.portion
                        .as_ref()
                        .is_some_and(|portion| portion.contains(track, self.ruler_frame(lane, pos)))
                })
            {
                return;
            }
        }
        // Each pane has its own clock and geometry. Restore the overview's lane
        // map immediately so another pane cannot update this rectangle.
        let previous_lanes =
            std::mem::replace(&mut self.lane_bounds, HashMap::from([(track, lane)]));
        self.timeline_selection(ui, response, lane, lane);
        self.lane_bounds = previous_lanes;
        if active
            || self
                .marquee
                .as_ref()
                .is_some_and(|marquee| marquee.pane == response.id)
        {
            self.portion = None;
            self.midi_editor.clear_note_selection();
            self.midi_editor.delete_clips = true;
            self.retain_content_view();
        }
    }
}
