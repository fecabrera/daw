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
            vertical_scroll_points: self.timeline_vertical_scroll,
        }
    }

    pub(super) fn restore_project_display(&mut self) {
        let display = self.session.project.display;
        self.timeline_track_size = display.track_height.value();
        self.zoom = display.horizontal_zoom;
        self.scroll = display.horizontal_scroll_seconds;
        self.timeline_vertical_scroll = display.vertical_scroll_points;
        self.timeline_vertical_restore = Some(display.vertical_scroll_points);
    }
}
