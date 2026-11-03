pub type Settings = daw_unit_core::LoudnessSettings;
use crate::{Parameter as P, PluginState};
pub fn update_settings(s: &PluginState, t: &mut Settings) {
    t.enabled = s.get(P::Bypass) == 0.0;
    t.target = s.get(P::Target) as u8;
    t.hq = s.get(P::Quality) != 0.0;
    t.continuous = s.get(P::Clock) != 0.0;
}
pub fn values(s: &Settings) -> [f64; 4] {
    [
        f64::from(!s.enabled),
        s.target as f64,
        f64::from(s.hq),
        f64::from(s.continuous),
    ]
}
pub fn to_settings(s: &PluginState) -> Settings {
    let mut t = Settings::default();
    update_settings(s, &mut t);
    t
}
