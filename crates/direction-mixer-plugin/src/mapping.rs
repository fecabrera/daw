use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::DirectionMixerSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.input = [
        daw_unit_core::DirectionMixerInput::LeftRight,
        daw_unit_core::DirectionMixerInput::MidSide,
    ][s.get(P::Input) as usize];
    settings.direction_degrees = s.get(P::Direction) as f32;
    settings.spread = s.get(P::Spread) as f32;
}
pub fn values(s: &Settings) -> [f64; 4] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.input as u8),
        f64::from(s.direction_degrees),
        f64::from(s.spread),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
