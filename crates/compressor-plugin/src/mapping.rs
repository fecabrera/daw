use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::CompressorSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.threshold_db = s.get(P::Threshold) as f32;
    settings.ratio = s.get(P::Ratio) as f32;
    settings.attack_ms = s.get(P::Attack) as f32;
    settings.release_ms = s.get(P::Release) as f32;
    settings.knee_db = s.get(P::Knee) as f32;
    settings.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    settings.dry_wet = s.get(P::DryWet) as f32;
    settings.detection = [
        daw_unit_core::CompressorDetection::Peak,
        daw_unit_core::CompressorDetection::Rms,
    ][s.get(P::Detection) as usize];
}
pub fn values(s: &Settings) -> [f64; 9] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.threshold_db),
        f64::from(s.ratio),
        f64::from(s.attack_ms),
        f64::from(s.release_ms),
        f64::from(s.knee_db),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.detection as u8),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
