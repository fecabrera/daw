//! Variable-band compression; audition state belongs to the live host, never the project.
use crate::{Modulation, ModulationDestinationSettings, ModulationParameter as P, ModulationScope};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const MULTIBAND_COMPRESSOR_TYPE_ID: &str = "daw.multiband-compressor";
pub const MULTIBAND_COMPRESSOR_STATE_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultibandCompressorBand {
    pub id: Uuid,
    pub enabled: bool,
    /// Shared boundary with the next range. The last range reaches Nyquist.
    pub upper_hz: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub level_db: f32,
    pub detection: crate::CompressorDetection,
}
impl Default for MultibandCompressorBand {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            upper_hz: 20_000.0,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            level_db: 0.0,
            detection: crate::CompressorDetection::Peak,
        }
    }
}
impl MultibandCompressorBand {
    pub fn compressor_settings(&self) -> crate::CompressorSettings {
        crate::CompressorSettings {
            enabled: self.enabled,
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
            knee_db: self.knee_db,
            level_db: self.level_db,
            detection: self.detection,
            dry_wet: 1.0,
            ..Default::default()
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.upper_hz.is_finite()
            && (20.0..=20_000.0).contains(&self.upper_hz)
            && self.compressor_settings().valid()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultibandCompressorSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub bands: Vec<MultibandCompressorBand>,
    #[serde(default)]
    pub presentation: Vec<Uuid>,
    pub level_db: f32,
    pub dry_wet: f32,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<Modulation>,
}
impl Default for MultibandCompressorSettings {
    fn default() -> Self {
        Self {
            version: MULTIBAND_COMPRESSOR_STATE_VERSION,
            enabled: true,
            collapsed: false,
            bands: [200.0, 2000.0, 20_000.0]
                .map(|upper_hz| MultibandCompressorBand {
                    upper_hz,
                    ..Default::default()
                })
                .into(),
            presentation: Vec::new(),
            level_db: 0.0,
            dry_wet: 1.0,
            modulation_open: false,
            modulations: Vec::new(),
        }
    }
}
impl MultibandCompressorSettings {
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.version == MULTIBAND_COMPRESSOR_STATE_VERSION
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
            && (self.presentation.is_empty()
                || (self.presentation.len() == self.bands.len()
                    && self
                        .presentation
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        == ids))
            && self.valid_modulations()
    }
    /// Split the widest logarithmic range; existing band and route identities stay stable.
    pub fn add_band(&mut self) -> Option<Uuid> {
        if self.bands.is_empty() {
            let band = MultibandCompressorBand::default();
            let id = band.id;
            self.bands.push(band);
            self.presentation.clear();
            return Some(id);
        }
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
        if !self.presentation.is_empty() {
            self.presentation.push(id);
        }
        Some(id)
    }
    pub fn remove_band(&mut self, id: Uuid) {
        if self.bands.len() == 1 {
            return;
        }
        if let Some(i) = self.bands.iter().position(|b| b.id == id) {
            let removed = self.bands.remove(i);
            self.presentation.retain(|b| *b != id);
            if i > 0 {
                self.bands[i - 1].upper_hz = removed.upper_hz;
            }
            self.modulations.retain(|r| r.target != Some(id));
        }
    }
}
impl ModulationDestinationSettings for MultibandCompressorSettings {
    const MODULATION_PARAMETERS: &'static [P] = &[
        P::CompressorThreshold,
        P::CompressorRatio,
        P::CompressorAttack,
        P::CompressorRelease,
        P::CompressorKnee,
        P::Level,
        P::DryWet,
    ];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        crate::valid_mixed_modulations(
            &self.modulations,
            &P::COMPRESSOR[..6],
            &self.bands.iter().map(|b| b.id).collect::<Vec<_>>(),
            &[P::Level, P::DryWet],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bands_routes_and_visual_order_preserve_identity_through_persistence_and_copy() {
        let mut effect = crate::EffectInstance::multiband_compressor();
        let crate::EffectSettings::MultibandCompressor(s) = &mut effect.unit else {
            unreachable!()
        };
        let first = s.bands[0].id;
        s.presentation = s.bands.iter().rev().map(|b| b.id).collect();
        let mut route = Modulation::new(P::CompressorThreshold);
        route.target = Some(first);
        s.modulations.push(route);
        for _ in 0..12 {
            s.add_band();
            assert!(s.valid());
        }
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(saved.clone()).unwrap(),
            effect
        );
        let copy = effect.duplicate();
        assert!(copy.unit.valid());
        assert!(
            copy.unit
                .band_ids()
                .all(|id| !effect.unit.band_ids().any(|old| old == id))
        );
        let crate::EffectSettings::MultibandCompressor(s) = &mut effect.unit else {
            unreachable!()
        };
        s.remove_band(first);
        assert!(s.valid());
        assert!(s.modulations.is_empty());
        s.bands[0].upper_hz = s.bands[1].upper_hz;
        assert!(!s.valid());
        let mut future = saved;
        future["unit"]["state"]["version"] = 999.into();
        let restored = serde_json::from_value::<crate::EffectInstance>(future.clone()).unwrap();
        assert!(matches!(
            restored.unit,
            crate::EffectSettings::Missing { .. }
        ));
        assert_eq!(serde_json::to_value(restored).unwrap(), future);
    }
}
