use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::HarmonicLiftSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    for (i, b) in settings.bands.iter_mut().enumerate() {
        b.id = daw_unit_core::ModulationTargetId::from_u128(i as u128 + 1);
    }
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.quality = if s.get(P::Quality) == 0.0 {
        daw_unit_core::HarmonicQuality::Normal
    } else {
        daw_unit_core::HarmonicQuality::Hq
    };
    settings.input_db = s.get(P::Input) as f32;
    settings.drive = s.get(P::Drive) as f32;
    settings.push = s.get(P::Push) as f32;
    settings.limit_db = s.get(P::Limit) as f32;
    settings.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    settings.dry_wet = s.get(P::DryWet) as f32;
    settings.bands[0].enabled = s.get(P::Band1Power) != 0.0;
    settings.bands[0].frequency_hz = s.get(P::Band1Frequency) as f32;
    settings.bands[0].gain_db = s.get(P::Band1Gain) as f32;
    settings.bands[1].enabled = s.get(P::Band2Power) != 0.0;
    settings.bands[1].frequency_hz = s.get(P::Band2Frequency) as f32;
    settings.bands[1].gain_db = s.get(P::Band2Gain) as f32;
    settings.bands[2].enabled = s.get(P::Band3Power) != 0.0;
    settings.bands[2].frequency_hz = s.get(P::Band3Frequency) as f32;
    settings.bands[2].gain_db = s.get(P::Band3Gain) as f32;
    settings.bands[3].enabled = s.get(P::Band4Power) != 0.0;
    settings.bands[3].frequency_hz = s.get(P::Band4Frequency) as f32;
    settings.bands[3].gain_db = s.get(P::Band4Gain) as f32;
    settings.bands[4].enabled = s.get(P::Band5Power) != 0.0;
    settings.bands[4].frequency_hz = s.get(P::Band5Frequency) as f32;
    settings.bands[4].gain_db = s.get(P::Band5Gain) as f32;
    settings.bands[5].enabled = s.get(P::Band6Power) != 0.0;
    settings.bands[5].frequency_hz = s.get(P::Band6Frequency) as f32;
    settings.bands[5].gain_db = s.get(P::Band6Gain) as f32;
}
pub fn values(s: &Settings) -> [f64; 26] {
    let v = [
        f64::from(!s.enabled),
        f64::from(s.input_db),
        f64::from(s.quality == daw_unit_core::HarmonicQuality::Hq),
        f64::from(s.bands[0].enabled),
        f64::from(s.bands[0].frequency_hz),
        f64::from(s.bands[0].gain_db),
        f64::from(s.bands[1].enabled),
        f64::from(s.bands[1].frequency_hz),
        f64::from(s.bands[1].gain_db),
        f64::from(s.bands[2].enabled),
        f64::from(s.bands[2].frequency_hz),
        f64::from(s.bands[2].gain_db),
        f64::from(s.bands[3].enabled),
        f64::from(s.bands[3].frequency_hz),
        f64::from(s.bands[3].gain_db),
        f64::from(s.bands[4].enabled),
        f64::from(s.bands[4].frequency_hz),
        f64::from(s.bands[4].gain_db),
        f64::from(s.bands[5].enabled),
        f64::from(s.bands[5].frequency_hz),
        f64::from(s.bands[5].gain_db),
        f64::from(s.drive),
        f64::from(s.push),
        f64::from(s.limit_db),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
    ];
    std::array::from_fn(|i| {
        let p = P::ALL[i].info();
        v[i].clamp(p.minimum, p.maximum)
    })
}
