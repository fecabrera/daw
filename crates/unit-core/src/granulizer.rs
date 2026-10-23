//! Versioned granular source controls. Positions are fractions of the selected input region.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrainWindow {
    #[default]
    Hann,
    Triangle,
    Trapezoid,
}
impl GrainWindow {
    pub const ALL: [Self; 3] = [Self::Hann, Self::Triangle, Self::Trapezoid];
    pub fn label(self) -> &'static str {
        match self {
            Self::Hann => "Hann",
            Self::Triangle => "Triangle",
            Self::Trapezoid => "Trapezoid",
        }
    }
    /// Zero at both endpoints. Trapezoid reserves one quarter for each fade.
    pub fn amplitude(self, phase: f32) -> f32 {
        let p = phase.clamp(0.0, 1.0);
        match self {
            Self::Hann => (1.0 - (std::f32::consts::TAU * p).cos()) * 0.5,
            Self::Triangle => (2.0 * p.min(1.0 - p)).max(0.0),
            Self::Trapezoid => (4.0 * p.min(1.0 - p)).clamp(0.0, 1.0),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GranulizerSettings {
    pub version: u32,
    pub size_ms: f32,
    pub overlap: f32,
    pub position: f32,
    pub speed: f32,
    pub pitch: f32,
    pub position_jitter: f32,
    pub pitch_jitter: f32,
    pub pan: f32,
    pub window: GrainWindow,
    pub looping: bool,
    /// Repeatable grain randomness, retained across save, copy and export.
    pub seed: u32,
}
impl Default for GranulizerSettings {
    fn default() -> Self {
        Self {
            version: 1,
            size_ms: 80.0,
            overlap: 4.0,
            position: 0.0,
            speed: 1.0,
            pitch: 0.0,
            position_jitter: 0.0,
            pitch_jitter: 0.0,
            pan: 0.0,
            window: GrainWindow::Hann,
            looping: true,
            seed: 1,
        }
    }
}
impl GranulizerSettings {
    pub fn values(self) -> [f32; 8] {
        [
            self.size_ms,
            self.overlap,
            self.position,
            self.speed,
            self.pitch,
            self.position_jitter,
            self.pitch_jitter,
            self.pan,
        ]
    }
    pub fn bounds() -> [(f32, f32); 8] {
        [
            (10.0, 500.0),
            (0.25, 8.0),
            (0.0, 1.0),
            (-4.0, 4.0),
            (-48.0, 48.0),
            (0.0, 1.0),
            (0.0, 48.0),
            (0.0, 1.0),
        ]
    }
    pub fn valid(self) -> bool {
        self.version == 1
            && self
                .values()
                .into_iter()
                .zip(Self::bounds())
                .all(|(v, (min, max))| v.is_finite() && (min..=max).contains(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Modulation, ModulationParameter as P, SamplerSettings};
    #[test]
    fn saved_granular_settings_round_trip_reject_invalid_data_and_preserve_legacy_sampler() {
        let mut settings = SamplerSettings::granular();
        settings.modulations = P::GRANULIZER.into_iter().map(Modulation::new).collect();
        assert!(settings.valid());
        assert_eq!(
            serde_json::from_value::<SamplerSettings>(serde_json::to_value(&settings).unwrap())
                .unwrap(),
            settings
        );
        for field in [
            "size_ms",
            "overlap",
            "position",
            "speed",
            "pitch",
            "position_jitter",
            "pitch_jitter",
            "pan",
        ] {
            let mut bad = serde_json::to_value(&settings).unwrap();
            bad["granulizer"][field] = serde_json::json!(10000.0);
            assert!(
                !serde_json::from_value::<SamplerSettings>(bad)
                    .unwrap()
                    .valid(),
                "{field}"
            );
        }
        let mut bad = settings.clone();
        bad.version = 1;
        assert!(
            !bad.valid(),
            "Older readers must not silently play granular sources as Sampler"
        );
        bad = settings.clone();
        bad.granulizer.as_mut().unwrap().version = 2;
        assert!(!bad.valid());
        bad = settings.clone();
        bad.mode = crate::SamplerMode::Time;
        assert!(!bad.valid());
        bad = settings;
        bad.granulizer = None;
        assert!(!bad.valid());
        let legacy = SamplerSettings::default();
        let json = serde_json::to_value(&legacy).unwrap();
        assert!(json.get("granulizer").is_none());
        assert_eq!(
            serde_json::from_value::<SamplerSettings>(json).unwrap(),
            legacy
        );
        for window in GrainWindow::ALL {
            assert_eq!(window.amplitude(0.0), 0.0);
            assert_eq!(window.amplitude(1.0), 0.0);
            assert_eq!(window.amplitude(0.5), 1.0);
        }
    }
}
