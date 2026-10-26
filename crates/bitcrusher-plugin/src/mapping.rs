use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::BitcrusherSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    Settings {
        enabled: s.get(P::Bypass) == 0.0,
        level_db: daw_unit_core::gain::to_db_f32(s.get(P::Level)),
        drive_db: s.get(P::Drive) as f32,
        dry_wet: s.get(P::DryWet) as f32,
        resolution_bits: s.get(P::Resolution) as u8,
        downsampling: s.get(P::Downsampling) as u8,
        ..Default::default()
    }
}
pub fn values(s: &Settings) -> [f64; 6] {
    [
        f64::from(!s.enabled),
        f64::from(s.level_db),
        f64::from(s.drive_db),
        f64::from(s.resolution_bits),
        f64::from(s.downsampling),
        f64::from(s.dry_wet),
    ]
}
