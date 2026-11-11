//! Capture display state only for Save; normal navigation stays outside Undo and audio edits.
use super::*;

impl DawUi {
    pub(super) fn display_save_snapshot(&self) -> daw_project::Result<daw_project::SaveSnapshot> {
        let mut session = self.session.clone();
        session.project.display = daw_core::ProjectDisplay {
            track_height: if self.track_rows_collapsed {
                daw_core::TrackHeight::Compact
            } else if self.timeline_tracks_large {
                daw_core::TrackHeight::Large
            } else {
                daw_core::TrackHeight::Regular
            },
            horizontal_zoom: self.zoom,
            horizontal_scroll_seconds: self.scroll,
            vertical_scroll_points: self.timeline_vertical_scroll,
        };
        session.save_snapshot(self.history.as_ref())
    }

    pub(super) fn restore_project_display(&mut self) {
        let display = self.session.project.display;
        self.track_rows_collapsed = display.track_height == daw_core::TrackHeight::Compact;
        self.timeline_tracks_large = display.track_height == daw_core::TrackHeight::Large;
        self.zoom = display.horizontal_zoom;
        self.scroll = display.horizontal_scroll_seconds;
        self.timeline_vertical_scroll = display.vertical_scroll_points;
        self.timeline_vertical_restore = Some(display.vertical_scroll_points);
    }
}
