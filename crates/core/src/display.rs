//! Saved presentation metadata; this does not change rendering or edit history.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackHeight {
    Compact,
    #[default]
    Regular,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectDisplay {
    pub track_height: TrackHeight,
    pub horizontal_zoom: f32,
    pub horizontal_scroll_seconds: f64,
    pub vertical_scroll_points: f32,
}
impl Default for ProjectDisplay {
    fn default() -> Self {
        Self {
            track_height: TrackHeight::Regular,
            horizontal_zoom: 70.0,
            horizontal_scroll_seconds: 0.0,
            vertical_scroll_points: 0.0,
        }
    }
}
impl ProjectDisplay {
    pub(crate) fn validate(self) -> crate::Result<()> {
        if !self.horizontal_zoom.is_finite()
            || self.horizontal_zoom <= 0.0
            || !self.horizontal_scroll_seconds.is_finite()
            || self.horizontal_scroll_seconds < 0.0
            || !self.vertical_scroll_points.is_finite()
            || self.vertical_scroll_points < 0.0
        {
            return Err(crate::Error("Invalid project display geometry".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_roundtrips_and_legacy_projects_use_regular_defaults() {
        let project = crate::Project {
            display: ProjectDisplay {
                track_height: TrackHeight::Large,
                horizontal_zoom: 160.0,
                horizontal_scroll_seconds: 12.5,
                vertical_scroll_points: 300.0,
            },
            ..Default::default()
        };
        let mut encoded = serde_json::to_value(&project).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::Project>(encoded.clone())
                .unwrap()
                .display,
            project.display
        );
        encoded.as_object_mut().unwrap().remove("display");
        assert_eq!(
            serde_json::from_value::<crate::Project>(encoded)
                .unwrap()
                .display,
            ProjectDisplay::default()
        );
    }
    #[test]
    fn invalid_display_geometry_is_rejected() {
        for value in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            let mut project = crate::Project::default();
            project.display.horizontal_zoom = value;
            assert!(project.validate().is_err());
        }
        let mut project = crate::Project::default();
        project.display.vertical_scroll_points = -1.0;
        assert!(project.validate().is_err());
        project.display.vertical_scroll_points = 0.0;
        project.display.horizontal_scroll_seconds = f64::INFINITY;
        assert!(project.validate().is_err());
    }
}
