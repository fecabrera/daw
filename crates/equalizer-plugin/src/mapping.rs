use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::EqSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.bands[0].gain_db = s.get(P::LowGain) as f32;
    settings.bands[0].frequency_hz = s.get(P::LowFrequency) as f32;
    settings.bands[0].q = s.get(P::LowQ) as f32;
    settings.bands[0].enabled = s.get(P::LowEnabled) != 0.0;
    settings.bands[1].gain_db = s.get(P::MidGain) as f32;
    settings.bands[1].enabled = s.get(P::MidEnabled) != 0.0;
    settings.bands[2].gain_db = s.get(P::HighGain) as f32;
    settings.bands[2].frequency_hz = s.get(P::HighFrequency) as f32;
    settings.bands[2].q = s.get(P::HighQ) as f32;
    settings.bands[2].enabled = s.get(P::HighEnabled) != 0.0;
}
pub fn values(s: &Settings) -> [f64; 11] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.bands[0].gain_db),
        f64::from(s.bands[0].frequency_hz),
        f64::from(s.bands[0].q),
        f64::from(s.bands[0].enabled),
        f64::from(s.bands[1].gain_db),
        f64::from(s.bands[1].enabled),
        f64::from(s.bands[2].gain_db),
        f64::from(s.bands[2].frequency_hz),
        f64::from(s.bands[2].q),
        f64::from(s.bands[2].enabled),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
