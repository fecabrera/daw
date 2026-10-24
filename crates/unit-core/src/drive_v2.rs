use crate::{Modulation, ModulationDestinationSettings, SILENT_GAIN_DB};
use serde::{Deserialize, Serialize};
pub const DRIVE_V2_TYPE_ID: &str = "daw.drive-v2";
pub const DRIVE_V2_STATE_VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveV2ColorShape {
    #[default]
    MidScoop,
    MidHump,
    MidHumpBassCut,
}
impl DriveV2ColorShape {
    pub const ALL: [Self; 3] = [Self::MidScoop, Self::MidHump, Self::MidHumpBassCut];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
    pub fn legacy(self) -> crate::DriveColorShape {
        match self {
            Self::MidScoop => crate::DriveColorShape::MidScoop,
            Self::MidHump => crate::DriveColorShape::MidHump,
            Self::MidHumpBassCut => crate::DriveColorShape::MidHumpBassCut,
        }
    }
    pub fn name(self) -> &'static str {
        self.legacy().name()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveV2Clip {
    #[default]
    Soft,
    Hard,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DriveV2Settings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
    pub input_gain_db: f32,
    pub gain: f32,
    pub tone: f32,
    pub color_enabled: bool,
    pub color_shape: DriveV2ColorShape,
    pub color_strength: f32,
    pub color_frequency_hz: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    pub clip: DriveV2Clip,
}
impl Default for DriveV2Settings {
    fn default() -> Self {
        Self {
            version: DRIVE_V2_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            input_gain_db: 0.0,
            gain: 0.2,
            tone: 0.5,
            color_enabled: false,
            color_shape: DriveV2ColorShape::MidScoop,
            color_strength: 0.5,
            color_frequency_hz: 750.0,
            level_db: 0.0,
            dry_wet: 1.0,
            clip: DriveV2Clip::Soft,
        }
    }
}
impl DriveV2Settings {
    pub fn valid(&self) -> bool {
        self.version == DRIVE_V2_STATE_VERSION
            && [self.input_gain_db, self.level_db]
                .into_iter()
                .all(|v| v.is_finite() && (SILENT_GAIN_DB..=12.0).contains(&v))
            && [self.gain, self.tone, self.color_strength, self.dry_wet]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
            && self.color_frequency_hz.is_finite()
            && (250.0..=2000.0).contains(&self.color_frequency_hz)
            && self.valid_modulations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectInstance, EffectSettings, ModulationParameter as P};
    #[test]
    fn drive_v2_saved_settings_remain_distinct_validate_routes_and_keep_future_payloads() {
        let mut e = EffectInstance::drive_v2();
        let EffectSettings::DriveV2(s) = &mut e.unit else {
            panic!()
        };
        s.color_enabled = true;
        s.color_shape = DriveV2ColorShape::MidHumpBassCut;
        s.clip = DriveV2Clip::Hard;
        s.dry_wet = 0.3;
        s.modulations.push(Modulation::new(P::DriveInput));
        let saved = serde_json::to_value(&e).unwrap();
        let restored: EffectInstance = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(restored, e);
        assert_eq!(saved["unit"]["type_id"], DRIVE_V2_TYPE_ID);
        let copy = e.duplicate();
        assert_ne!(copy.id, e.id);
        assert_ne!(copy.unit.modulations()[0].id, e.unit.modulations()[0].id);
        assert!(e.unit.valid());
        let mut future = saved["unit"].clone();
        future["state"]["version"] = 99.into();
        let missing: EffectSettings = serde_json::from_value(future.clone()).unwrap();
        assert!(matches!(missing, EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(missing).unwrap(), future);
        let old = EffectInstance::drive();
        assert_eq!(
            serde_json::to_value(&old).unwrap()["unit"]["type_id"],
            crate::DRIVE_TYPE_ID
        );
        for value in [f32::NAN, f32::INFINITY, 12.1, SILENT_GAIN_DB - 1.0] {
            let bad = DriveV2Settings {
                input_gain_db: value,
                ..Default::default()
            };
            assert!(!bad.valid());
        }
        let mut bad = DriveV2Settings::default();
        let mut route = Modulation::new(P::Pitch);
        bad.modulations.push(route);
        assert!(!bad.valid());
        route = Modulation::new(P::DriveInput);
        route.target = Some(crate::ModulationTargetId::new_v4());
        bad.modulations[0] = route;
        assert!(!bad.valid());
    }
}
