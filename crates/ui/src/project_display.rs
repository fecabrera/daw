//! Capture display state only for Save; normal navigation stays outside Undo and audio edits.
use super::*;

impl DawUi {
    pub(super) fn display_save_snapshot(&self) -> daw_project::Result<daw_project::SaveSnapshot> {
        let mut session = self.session.clone();
        session.project.display = self.current_project_display();
        session.save_snapshot(self.history.as_ref())
    }

    pub(super) fn current_project_display(&self) -> daw_core::ProjectDisplay {
        daw_core::ProjectDisplay {
            track_height: daw_core::TrackHeight::from_value(self.timeline_track_size),
            horizontal_zoom: self.zoom,
            horizontal_scroll_seconds: self.scroll,
            vertical_scroll_points: self.track_scroll_offset,
            arrangement_expanded: !self.arrangement.collapsed,
            timeline_editor_height_points: self.timeline_editor.saved_height(),
        }
    }

    pub(super) fn restore_project_display(&mut self) {
        let display = self.session.project.display;
        self.arrangement.collapsed = !display.arrangement_expanded;
        self.timeline_track_size = display.track_height.value();
        self.zoom = display.horizontal_zoom;
        self.scroll = display.horizontal_scroll_seconds;
        self.track_scroll_offset = display.vertical_scroll_points;
        self.track_scroll_restore = Some(display.vertical_scroll_points);
        self.timeline_editor
            .restore_height(display.timeline_editor_height_points);
    }
}
