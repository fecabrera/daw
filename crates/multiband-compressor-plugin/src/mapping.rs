use crate::{Parameter as P, PluginState};
use daw_unit_core::{MultibandCompressorBand as Band, MultibandCompressorSettings as Core};
/// Control-thread view retains inactive automation slots without putting them in project state.
pub struct Settings {
    pub order: [u8; 64],
    pub core: Core,
    pub slots: [Band; 64],
}
impl std::ops::Deref for Settings {
    type Target = Core;
    fn deref(&self) -> &Core {
        &self.core
    }
}
impl std::ops::DerefMut for Settings {
    fn deref_mut(&mut self) -> &mut Core {
        &mut self.core
    }
}
pub fn band(state: &PluginState, slot: usize) -> Band {
    let base = 1 + 10 * slot;
    Band {
        id: uuid::Uuid::from_u128(slot as u128 + 1),
        enabled: state.get(P::ALL[base + 1]) == 1.0,
        upper_hz: state.get(P::ALL[base + 2]) as f32,
        threshold_db: state.get(P::ALL[base + 3]) as f32,
        ratio: state.get(P::ALL[base + 4]) as f32,
        attack_ms: state.get(P::ALL[base + 5]) as f32,
        release_ms: state.get(P::ALL[base + 6]) as f32,
        knee_db: state.get(P::ALL[base + 7]) as f32,
        level_db: daw_unit_core::gain::to_db_f32(state.get(P::ALL[base + 8])),
        detection: if state.get(P::ALL[base + 9]) == 0.0 {
            daw_unit_core::CompressorDetection::Peak
        } else {
            daw_unit_core::CompressorDetection::Rms
        },
    }
}
pub fn present(state: &PluginState, slot: usize) -> bool {
    state.get(P::ALL[1 + 10 * slot]) == 1.0
}
pub fn to_settings(state: &PluginState) -> Settings {
    let slots = std::array::from_fn(|i| band(state, i));
    let mut bands: Vec<_> = slots
        .iter()
        .enumerate()
        .filter(|(i, _)| present(state, *i))
        .map(|(_, b)| b.clone())
        .collect();
    bands.sort_by(|a, b| a.upper_hz.total_cmp(&b.upper_hz).then(a.id.cmp(&b.id)));
    let count = bands.len();
    let mut lower = 19.0;
    for (i, b) in bands.iter_mut().enumerate() {
        b.upper_hz = if i + 1 == count {
            20000.0
        } else {
            b.upper_hz
                .clamp(lower + 1.0, 20000.0 - (count - i - 1) as f32)
        };
        lower = b.upper_hz;
    }
    let presentation = state
        .presentation()
        .iter()
        .filter_map(|slot| {
            bands
                .iter()
                .find(|b| b.id.as_u128() == u128::from(*slot) + 1)
                .map(|b| b.id)
        })
        .collect();
    Settings {
        order: state.presentation(),
        core: Core {
            bands,
            presentation,
            enabled: state.get(P::Bypass) == 0.0,
            level_db: daw_unit_core::gain::to_db_f32(state.get(P::Level)),
            dry_wet: state.get(P::DryWet) as f32,
            ..Default::default()
        },
        slots,
    }
}
pub fn values(s: &Settings) -> [f64; P::COUNT] {
    std::array::from_fn(|i| {
        if i == 0 {
            return f64::from(!s.enabled);
        }
        if P::ALL[i] == P::Level {
            return f64::from(s.level_db);
        }
        if P::ALL[i] == P::DryWet {
            return f64::from(s.dry_wet);
        }
        let slot = (i - 1) / 10;
        let field = (i - 1) % 10;
        let active = s.bands.iter().find(|b| b.id.as_u128() == slot as u128 + 1);
        let b = active.unwrap_or(&s.slots[slot]);
        match field {
            0 => f64::from(active.is_some()),
            1 => f64::from(b.enabled),
            2 => f64::from(b.upper_hz),
            3 => f64::from(b.threshold_db),
            4 => f64::from(b.ratio),
            5 => f64::from(b.attack_ms),
            6 => f64::from(b.release_ms),
            7 => f64::from(b.knee_db),
            8 => f64::from(b.level_db),
            _ => f64::from(b.detection == daw_unit_core::CompressorDetection::Rms),
        }
    })
}
