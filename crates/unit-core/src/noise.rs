//! Versioned procedural noise source settings.
use crate::{Modulation, ModulationDestinationSettings, SILENT_GAIN_DB};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseType {
    #[default]
    White,
    Pink,
    Brown,
}
impl NoiseType {
    pub const ALL: [Self; 3] = [Self::White, Self::Pink, Self::Brown];
    pub fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Pink => "Pink",
            Self::Brown => "Brown",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseSettings {
    pub id: uuid::Uuid,
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub kind: NoiseType,
    /// Signed spectral tilt, -1 (dark) through +1 (bright).
    pub color: f32,
    /// 0 = mono; 1 = independent stereo.
    pub width: f32,
    pub level_db: f32,
    pub modulation_open: bool,
    pub modulations: Vec<Modulation>,
}
impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            version: 1,
            enabled: true,
            collapsed: false,
            kind: NoiseType::White,
            color: 0.0,
            width: 0.0,
            level_db: -12.0,
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl NoiseSettings {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.version == 1
            && self.color.is_finite()
            && (-1.0..=1.0).contains(&self.color)
            && self.width.is_finite()
            && (0.0..=1.0).contains(&self.width)
            && self.level_db.is_finite()
            && (SILENT_GAIN_DB..=12.0).contains(&self.level_db)
            && self.valid_modulations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InstrumentComponent, InstrumentLayerSettings, ModulationParameter};
    #[test]
    fn noise_settings_roundtrip_validate_conflicts_and_duplicate_owned_routes() {
        let mut layer = InstrumentLayerSettings {
            noise: Some(Default::default()),
            ..Default::default()
        };
        layer.component_order = vec![InstrumentComponent::Noise];
        layer
            .noise
            .as_mut()
            .unwrap()
            .modulations
            .push(Modulation::new(ModulationParameter::NoiseLevel));
        assert!(layer.valid());
        let saved = serde_json::to_value(&layer).unwrap();
        let decoded: InstrumentLayerSettings = serde_json::from_value(saved).unwrap();
        assert_eq!(decoded, layer);
        let copy = layer.duplicate();
        assert_ne!(
            copy.noise.as_ref().unwrap().id,
            layer.noise.as_ref().unwrap().id
        );
        assert_ne!(
            copy.noise.as_ref().unwrap().modulations[0].id,
            layer.noise.as_ref().unwrap().modulations[0].id
        );
        layer.oscillators.push(Default::default());
        assert!(!layer.valid());
        layer.oscillators.clear();
        layer.sampler = Some(Default::default());
        assert!(!layer.valid());
        layer.sampler = None;
        for color in [f32::NAN, -1.01, 1.01] {
            layer.noise.as_mut().unwrap().color = color;
            assert!(!layer.valid());
        }
        layer.clear_sources();
        assert!(layer.noise.is_none() && layer.component_order.is_empty());
    }
}
