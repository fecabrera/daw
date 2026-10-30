//! Variable-band excitation; audition state belongs to the live host, never the project.
use crate::{Modulation, ModulationDestinationSettings, ModulationParameter as P, ModulationScope};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const EXCITER_TYPE_ID: &str = "daw.exciter";
pub const EXCITER_STATE_VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExciterMode {
    #[default]
    Soft,
    Tube,
    Tape,
    Hard,
}
impl ExciterMode {
    pub const ALL: [Self; 4] = [Self::Soft, Self::Tube, Self::Tape, Self::Hard];
    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Tube => "Tube",
            Self::Tape => "Tape",
            Self::Hard => "Hard",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExciterBand {
    pub id: Uuid,
    pub enabled: bool,
    pub mode: ExciterMode,
    /// Shared boundary with the next band; last band always extends to Nyquist.
    pub upper_hz: f32,
    pub drive_db: f32,
    pub mix: f32,
    pub level_db: f32,
}
impl Default for ExciterBand {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            mode: ExciterMode::Soft,
            upper_hz: 20_000.0,
            drive_db: 0.0,
            mix: 0.0,
            level_db: 0.0,
        }
    }
}
impl ExciterBand {
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.upper_hz.is_finite()
            && (20.0..=20_000.0).contains(&self.upper_hz)
            && self.drive_db.is_finite()
            && (-36.0..=36.0).contains(&self.drive_db)
            && self.mix.is_finite()
            && (0.0..=1.0).contains(&self.mix)
            && self.level_db.is_finite()
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.level_db)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExciterSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub bands: Vec<ExciterBand>,
    pub level_db: f32,
    pub dry_wet: f32,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
}
impl Default for ExciterSettings {
    fn default() -> Self {
        Self {
            version: EXCITER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            bands: [150.0, 2000.0, 6000.0, 20_000.0]
                .map(|upper_hz| ExciterBand {
                    upper_hz,
                    ..Default::default()
                })
                .into(),
            level_db: 0.0,
            dry_wet: 1.0,
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl ExciterSettings {
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.version == EXCITER_STATE_VERSION
            && !self.bands.is_empty()
            && self.bands.iter().all(|b| b.valid() && ids.insert(b.id))
            && self
                .bands
                .windows(2)
                .all(|b| b[1].upper_hz - b[0].upper_hz >= 1.0)
            && self.bands.last().is_some_and(|b| b.upper_hz == 20_000.0)
            && self.level_db.is_finite()
            && (crate::SILENT_GAIN_DB..=12.0).contains(&self.level_db)
            && self.dry_wet.is_finite()
            && (0.0..=1.0).contains(&self.dry_wet)
            && self.valid_modulations()
    }
    /// Split the widest logarithmic range; existing band and route identities stay stable.
    pub fn add_band(&mut self) -> Option<Uuid> {
        let mut lower = 20.0_f32;
        let mut best = None;
        for (i, b) in self.bands.iter().enumerate() {
            if b.upper_hz - lower >= 2.0 {
                let width = (b.upper_hz / lower).ln();
                if best.is_none_or(|(_, w, _)| width > w) {
                    best = Some((i, width, lower));
                }
            }
            lower = b.upper_hz;
        }
        let (i, _, lower) = best?;
        let mut right = self.bands[i].clone();
        right.id = Uuid::new_v4();
        self.bands[i].upper_hz = (lower * right.upper_hz)
            .sqrt()
            .clamp(lower + 1.0, right.upper_hz - 1.0);
        let id = right.id;
        self.bands.insert(i + 1, right);
        Some(id)
    }
    pub fn remove_band(&mut self, id: Uuid) {
        if self.bands.len() == 1 {
            return;
        }
        if let Some(i) = self.bands.iter().position(|b| b.id == id) {
            let removed = self.bands.remove(i);
            if i > 0 {
                self.bands[i - 1].upper_hz = removed.upper_hz;
            }
            self.modulations.retain(|r| r.target != Some(id));
        }
    }
}
impl ModulationDestinationSettings for ExciterSettings {
    const MODULATION_PARAMETERS: &'static [P] = &P::SATURATOR;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        crate::valid_mixed_modulations(
            &self.modulations,
            &P::SATURATOR,
            &self.bands.iter().map(|b| b.id).collect::<Vec<_>>(),
            &[P::Level, P::DryWet],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectInstance, EffectSettings};
    #[test]
    fn identities_routes_edits_and_future_versions_round_trip() {
        let mut effect = EffectInstance::exciter();
        let EffectSettings::Exciter(s) = &mut effect.unit else {
            unreachable!()
        };
        let id = s.bands[1].id;
        let mut route = Modulation::new(P::SaturatorDrive);
        route.target = Some(id);
        s.modulations.push(route);
        for _ in 0..20 {
            assert!(s.add_band().is_some());
            assert!(s.valid());
        }
        s.remove_band(s.bands[0].id);
        assert!(s.valid());
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<EffectInstance>(saved.clone()).unwrap(),
            effect
        );
        let copy = effect.duplicate();
        assert!(copy.unit.valid());
        assert!(
            effect
                .unit
                .band_ids()
                .all(|id| !copy.unit.band_ids().any(|new| id == new))
        );
        assert_ne!(
            effect.unit.modulations()[0].id,
            copy.unit.modulations()[0].id
        );
        let EffectSettings::Exciter(s) = &mut effect.unit else {
            unreachable!()
        };
        s.remove_band(id);
        assert!(s.modulations.is_empty());
        assert!(s.valid());
        for _ in 0..30 {
            s.remove_band(s.bands[0].id);
        }
        assert_eq!(s.bands.len(), 1);
        assert!(s.valid());
        let mut future = saved;
        future["unit"]["state"]["version"] = 999.into();
        let opaque: EffectInstance = serde_json::from_value(future.clone()).unwrap();
        assert!(matches!(opaque.unit, EffectSettings::Missing { .. }));
        assert!(opaque.unit.valid());
        assert_eq!(serde_json::to_value(opaque).unwrap(), future);
    }
    #[test]
    fn invalid_settings_reject_bad_crossovers_duplicate_ids_and_targets() {
        let good = ExciterSettings::default();
        assert!(good.valid());
        for case in 0..9 {
            let mut s = good.clone();
            match case {
                0 => s.bands.clear(),
                1 => s.bands[0].upper_hz = s.bands[1].upper_hz,
                2 => s.bands[1].id = s.bands[0].id,
                3 => s.bands[0].mix = f32::NAN,
                4 => s.bands[0].drive_db = 36.1,
                5 => s.level_db = f32::INFINITY,
                6 => s.bands.last_mut().unwrap().upper_hz = 19000.0,
                7 => {
                    let mut r = Modulation::new(P::SaturatorDrive);
                    r.target = Some(Uuid::new_v4());
                    s.modulations.push(r);
                }
                _ => s.modulations.push(Modulation::new(P::SaturatorDrive)),
            }
            assert!(!s.valid(), "case {case}");
        }
    }
}
