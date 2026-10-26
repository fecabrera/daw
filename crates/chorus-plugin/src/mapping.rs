use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::ChorusSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.delay_ms = s.get(P::Delay) as f32;
    settings.depth = s.get(P::Depth) as f32;
    settings.feedback = s.get(P::Feedback) as f32;
    settings.rate_hz = s.get(P::RateHz) as f32;
    settings.stereo_phase_degrees = s.get(P::StereoPhase) as f32;
    settings.width = s.get(P::Width) as f32;
    settings.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    settings.dry_wet = s.get(P::DryWet) as f32;
    settings.voices = s.get(P::Voices) as u8;
    settings.rate_mode = daw_unit_core::LfoRateMode::ALL[s.get(P::RateMode) as usize];
    settings.note_rate.0 = s.get(P::NoteRate) as u8;
}
pub fn values(s: &Settings) -> [f64; 12] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.delay_ms),
        f64::from(s.depth),
        f64::from(s.feedback),
        f64::from(s.rate_hz),
        f64::from(s.stereo_phase_degrees),
        f64::from(s.width),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.voices),
        f64::from(s.rate_mode as u8),
        f64::from(s.note_rate.0),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
