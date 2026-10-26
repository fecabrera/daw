use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::StereoSpreadSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.lower_intensity = s.get(P::LowerIntensity) as f32;
    settings.lower_frequency_hz = s.get(P::LowerFrequency) as f32;
    settings.upper_intensity = s.get(P::UpperIntensity) as f32;
    settings.upper_frequency_hz = s.get(P::UpperFrequency) as f32;
    settings.order = s.get(P::Order) as u8;
    [settings.lower_frequency_hz, settings.upper_frequency_hz] =
        Settings::constrained_frequencies(settings.lower_frequency_hz, settings.upper_frequency_hz);
}
pub fn values(s: &Settings) -> [f64; 6] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.lower_intensity),
        f64::from(s.lower_frequency_hz),
        f64::from(s.upper_intensity),
        f64::from(s.upper_frequency_hz),
        f64::from(s.order),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
