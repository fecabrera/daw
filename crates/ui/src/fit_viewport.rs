//! One-shot horizontal fitting for Timeline and the Editor track overview.
use super::*;

impl DawUi {
    pub(super) fn fit_all_range(&self) -> Option<(u64, u64)> {
        let track = self
            .view_mode
            .has_track_overview()
            .then(|| self.editor_track().map(|track| track.id));
        self.session
            .project
            .tracks
            .iter()
            .filter(|candidate| track.is_none_or(|id| id == Some(candidate.id)))
            .flat_map(|track| &track.clips)
            .map(|clip| (clip.start_frame, clip.end()))
            .reduce(|(start, end), (next_start, next_end)| {
                (start.min(next_start), end.max(next_end))
            })
    }

    pub(super) fn apply_fit_all(&mut self, width: f32) {
        if !std::mem::take(&mut self.fit_all_requested) || width <= 0.0 {
            return;
        }
        let Some((start, end)) = self.fit_all_range() else {
            return;
        };
        // Use the current layout width, rather than last frame's lane bounds.
        // Allow an exact fit outside the manual zoom slider's usual limits.
        let zoom = (f64::from(width) / self.session.project.seconds(end - start)) as f32;
        if zoom.is_finite() && zoom > 0.0 {
            self.set_view_zoom(zoom);
            self.set_view_scroll(self.session.project.seconds(start));
        }
    }
}
