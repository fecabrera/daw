use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::FilterSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.shape = daw_unit_core::FilterShape::ALL[s.get(P::Shape) as usize];
    settings.frequency_hz = s.get(P::Frequency) as f32;
    settings.q = s.get(P::Q) as f32;
    settings.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    settings.dry_wet = s.get(P::DryWet) as f32;
}
pub fn values(s: &Settings) -> [f64; 6] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.shape as u8),
        f64::from(s.frequency_hz),
        f64::from(s.q),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
