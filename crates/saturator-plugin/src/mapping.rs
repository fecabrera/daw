use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::SaturatorSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    Settings {
        enabled: s.get(P::Bypass) == 0.0,
        level_db: daw_unit_core::gain::to_db_f32(s.get(P::Level)),
        drive_db: s.get(P::Drive) as f32,
        dry_wet: s.get(P::DryWet) as f32,
        shape: daw_unit_core::SaturatorShape::ALL[s.get(P::Shape) as usize],
        clip_mode: daw_unit_core::SaturatorClipMode::ALL[s.get(P::ClipMode) as usize],
        ..Default::default()
    }
}
pub fn values(s: &Settings) -> [f64; 6] {
    [
        f64::from(!s.enabled),
        f64::from(s.level_db),
        f64::from(s.drive_db),
        daw_unit_core::SaturatorShape::ALL
            .iter()
            .position(|v| *v == s.shape)
            .unwrap_or(0) as f64,
        daw_unit_core::SaturatorClipMode::ALL
            .iter()
            .position(|v| *v == s.clip_mode)
            .unwrap_or(0) as f64,
        f64::from(s.dry_wet),
    ]
}
