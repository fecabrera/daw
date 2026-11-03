pub type Settings = daw_unit_core::PlateReverbSettings;
use crate::{Parameter as P, PluginState};
pub fn update_settings(s: &PluginState, t: &mut Settings) {
    t.enabled = s.get(P::Bypass) == 0.0;
    t.predelay_ms = s.get(P::PreDelay) as f32;
    t.decay_seconds = s.get(P::Decay) as f32;
    t.highpass_hz = s.get(P::HighPass) as f32;
    t.lowpass_hz = s.get(P::LowPass) as f32;
    t.drive_db = s.get(P::Drive) as f32;
    t.width = s.get(P::Width) as f32;
    t.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    t.dry_wet = s.get(P::DryWet) as f32;
    t.gated = s.get(P::Gate) != 0.0;
    t.dark = s.get(P::Color) != 0.0;
    t.drive_enabled = s.get(P::DriveEnabled) != 0.0;
    t.hq = s.get(P::Quality) != 0.0;
}
pub fn values(s: &Settings) -> [f64; 13] {
    [
        f64::from(!s.enabled),
        f64::from(s.predelay_ms),
        f64::from(s.decay_seconds),
        f64::from(s.highpass_hz),
        f64::from(s.lowpass_hz),
        f64::from(s.drive_db),
        f64::from(s.width),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.gated),
        f64::from(s.dark),
        f64::from(s.drive_enabled),
        f64::from(s.hq),
    ]
}

pub fn to_settings(s: &PluginState) -> Settings {
    let mut t = Settings::default();
    update_settings(s, &mut t);
    t
}
