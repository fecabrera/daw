pub type Settings = daw_unit_core::ShimmerReverbSettings;
use crate::{Parameter as P, PluginState};
pub fn update_settings(s: &PluginState, t: &mut Settings) {
    t.enabled = s.get(P::Bypass) == 0.0;
    t.size = s.get(P::Size) as f32;
    t.predelay_ms = s.get(P::PreDelay) as f32;
    t.sync = s.get(P::Sync) != 0.0;
    t.note = s.get(P::Note) as u8;
    t.vowel = s.get(P::Vowel) as f32;
    t.vowel_amount = s.get(P::VowelAmount) as f32;
    t.pitch_1 = s.get(P::Pitch1) as i8 - 15;
    t.amount_1 = s.get(P::Amount1) as f32;
    t.pitch_2 = s.get(P::Pitch2) as i8 - 15;
    t.amount_2 = s.get(P::Amount2) as f32;
    t.rate = s.get(P::Rate) as f32;
    t.depth = s.get(P::Depth) as f32;
    t.low_damp = s.get(P::LowDamp) as f32;
    t.high_damp = s.get(P::HighDamp) as f32;
    t.warm = s.get(P::Warm) != 0.0;
    t.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    t.dry_wet = s.get(P::DryWet) as f32;
}
pub fn values(s: &Settings) -> [f64; 18] {
    [
        f64::from(!s.enabled),
        f64::from(s.size),
        f64::from(s.predelay_ms),
        f64::from(s.sync),
        f64::from(s.note),
        f64::from(s.vowel),
        f64::from(s.vowel_amount),
        f64::from(s.pitch_1 + 15),
        f64::from(s.amount_1),
        f64::from(s.pitch_2 + 15),
        f64::from(s.amount_2),
        f64::from(s.rate),
        f64::from(s.depth),
        f64::from(s.low_damp),
        f64::from(s.high_damp),
        f64::from(s.warm),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
    ]
}
pub fn to_settings(s: &PluginState) -> Settings {
    let mut t = Settings::default();
    update_settings(s, &mut t);
    t
}
