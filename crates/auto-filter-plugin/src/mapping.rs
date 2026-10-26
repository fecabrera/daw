use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::AutoFilterSettings;
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
    settings.lfo_enabled = s.get(P::LfoEnabled) != 0.0;
    settings.wave = daw_unit_core::LfoWave::ALL[s.get(P::Wave) as usize];
    settings.rate_mode = daw_unit_core::LfoRateMode::ALL[s.get(P::RateMode) as usize];
    settings.rate_hz = s.get(P::RateHz) as f32;
    settings.note_rate.0 = s.get(P::NoteRate) as u8;
    settings.strength_octaves = s.get(P::Strength) as f32;
    settings.sync_phase_degrees = s.get(P::SyncPhase) as f32;
    settings.stereo_phase_degrees = s.get(P::StereoPhase) as f32;
}
pub fn values(s: &Settings) -> [f64; 14] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.shape as u8),
        f64::from(s.frequency_hz),
        f64::from(s.q),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.lfo_enabled),
        f64::from(s.wave as u8),
        f64::from(s.rate_mode as u8),
        f64::from(s.rate_hz),
        f64::from(s.note_rate.0),
        f64::from(s.strength_octaves),
        f64::from(s.sync_phase_degrees),
        f64::from(s.stereo_phase_degrees),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
