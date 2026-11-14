//! Saved presentation metadata; this does not change rendering or edit history.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TrackHeight {
    Compact,
    #[default]
    Regular,
    Large,
    /// Numeric display size; named variants retain legacy manifest strings.
    Fractional(f32),
}
impl TrackHeight {
    pub fn value(self) -> f32 {
        match self {
            Self::Compact => 1.0,
            Self::Regular => 2.0,
            Self::Large => 3.0,
            Self::Fractional(value) => value,
        }
    }
    pub fn from_value(value: f32) -> Self {
        match value {
            1.0 => Self::Compact,
            2.0 => Self::Regular,
            3.0 => Self::Large,
            _ => Self::Fractional(value),
        }
    }
}
impl Serialize for TrackHeight {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Compact => serializer.serialize_str("compact"),
            Self::Regular => serializer.serialize_str("regular"),
            Self::Large => serializer.serialize_str("large"),
            Self::Fractional(value) => serializer.serialize_f32(*value),
        }
    }
}
impl<'de> Deserialize<'de> for TrackHeight {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Named {
            Compact,
            Regular,
            Large,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Saved {
            Named(Named),
            Numeric(f32),
        }
        Ok(match Saved::deserialize(deserializer)? {
            Saved::Named(Named::Compact) => Self::Compact,
            Saved::Named(Named::Regular) => Self::Regular,
            Saved::Named(Named::Large) => Self::Large,
            Saved::Numeric(value) => Self::Fractional(value),
        })
    }
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
        if !self.track_height.value().is_finite()
            || !(1.0..=4.0).contains(&self.track_height.value())
            || !self.horizontal_zoom.is_finite()
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
    fn fractional_heights_roundtrip_and_named_heights_remain_compatible() {
        for (json, value) in [
            ("\"compact\"", 1.0),
            ("\"regular\"", 2.0),
            ("\"large\"", 3.0),
            ("1.5", 1.5),
            ("2.0", 2.0),
            ("2.75", 2.75),
            ("3.5", 3.5),
            ("4.0", 4.0),
        ] {
            let height: TrackHeight = serde_json::from_str(json).unwrap();
            assert_eq!(height.value(), value);
            let mut project = crate::Project::default();
            project.display.track_height = height;
            project.validate().unwrap();
            let reopened: crate::Project =
                serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
            assert_eq!(reopened.display, project.display);
        }
        for value in [0.99, 4.01, f32::NAN, f32::INFINITY] {
            let mut project = crate::Project::default();
            project.display.track_height = TrackHeight::from_value(value);
            assert!(project.validate().is_err());
        }
        assert!(serde_json::from_str::<TrackHeight>("\"unknown\"").is_err());
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
