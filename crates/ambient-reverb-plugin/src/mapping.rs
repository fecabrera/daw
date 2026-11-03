pub type Settings = daw_unit_core::AmbientReverbSettings;
use crate::{Parameter as P, PluginState};
pub fn update_settings(s: &PluginState, t: &mut Settings) {
    t.enabled = s.get(P::Bypass) == 0.0;
    t.decay_seconds = s.get(P::Decay) as f32;
    t.bloom = s.get(P::Bloom) as f32;
    t.tone = s.get(P::Tone) as f32;
    t.motion = s.get(P::Motion) as f32;
    t.width = s.get(P::Width) as f32;
    t.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    t.dry_wet = s.get(P::DryWet) as f32;
    t.hq = s.get(P::Quality) != 0.0;
    t.highpass_enabled = s.get(P::HighPassEnabled) != 0.0;
}
pub fn values(s: &Settings) -> [f64; 10] {
    [
        f64::from(!s.enabled),
        f64::from(s.decay_seconds),
        f64::from(s.bloom),
        f64::from(s.tone),
        f64::from(s.motion),
        f64::from(s.width),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.hq),
        f64::from(s.highpass_enabled),
    ]
}

pub fn to_settings(s: &PluginState) -> Settings {
    let mut t = Settings::default();
    update_settings(s, &mut t);
    t
}
