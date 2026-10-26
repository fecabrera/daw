use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::LimiterSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.input_db = s.get(P::Input) as f32;
    settings.release_ms = s.get(P::Release) as f32;
    settings.lookahead_ms = s.get(P::Lookahead) as f32;
    settings.ceiling_db = s.get(P::Ceiling) as f32;
    settings.mode = [
        daw_unit_core::LimiterMode::Peak,
        daw_unit_core::LimiterMode::TruePeak,
    ][s.get(P::Mode) as usize];
    settings.routing = [
        daw_unit_core::LimiterRouting::LeftRight,
        daw_unit_core::LimiterRouting::MidSide,
    ][s.get(P::Routing) as usize];
}
pub fn values(s: &Settings) -> [f64; 7] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.input_db),
        f64::from(s.release_ms),
        f64::from(s.lookahead_ms),
        f64::from(s.ceiling_db),
        f64::from(s.mode as u8),
        f64::from(s.routing as u8),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
