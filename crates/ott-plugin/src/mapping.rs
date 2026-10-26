use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::OttSettings;
pub fn to_settings(s: &PluginState) -> Settings {
    let mut settings = Settings::default();
    update_settings(s, &mut settings);
    settings
}
pub fn update_settings(s: &PluginState, settings: &mut Settings) {
    settings.enabled = s.get(P::Bypass) == 0.0;
    settings.low_frequency_hz = s.get(P::LowFrequency) as f32;
    settings.high_frequency_hz = s.get(P::HighFrequency) as f32;
    settings.bands[0].depth = s.get(P::LowDepth) as f32;
    settings.bands[0].input_db = s.get(P::LowInputDb) as f32;
    settings.bands[0].output_db = s.get(P::LowOutputDb) as f32;
    settings.bands[0].attack_ms = s.get(P::LowAttackMs) as f32;
    settings.bands[0].release_ms = s.get(P::LowReleaseMs) as f32;
    settings.bands[0].enabled = s.get(P::LowEnabled) != 0.0;
    settings.bands[1].depth = s.get(P::MidDepth) as f32;
    settings.bands[1].input_db = s.get(P::MidInputDb) as f32;
    settings.bands[1].output_db = s.get(P::MidOutputDb) as f32;
    settings.bands[1].attack_ms = s.get(P::MidAttackMs) as f32;
    settings.bands[1].release_ms = s.get(P::MidReleaseMs) as f32;
    settings.bands[1].enabled = s.get(P::MidEnabled) != 0.0;
    settings.bands[2].depth = s.get(P::HighDepth) as f32;
    settings.bands[2].input_db = s.get(P::HighInputDb) as f32;
    settings.bands[2].output_db = s.get(P::HighOutputDb) as f32;
    settings.bands[2].attack_ms = s.get(P::HighAttackMs) as f32;
    settings.bands[2].release_ms = s.get(P::HighReleaseMs) as f32;
    settings.bands[2].enabled = s.get(P::HighEnabled) != 0.0;
    settings.level_db = daw_unit_core::gain::to_db_f32(s.get(P::Level));
    settings.dry_wet = s.get(P::DryWet) as f32;
    settings.detection = [
        daw_unit_core::CompressorDetection::Peak,
        daw_unit_core::CompressorDetection::Rms,
    ][s.get(P::Detection) as usize];
    settings.low_frequency_hz = settings
        .low_frequency_hz
        .min(settings.high_frequency_hz - 1.0);
}
pub fn values(s: &Settings) -> [f64; 24] {
    let values = [
        f64::from(!s.enabled),
        f64::from(s.low_frequency_hz),
        f64::from(s.high_frequency_hz),
        f64::from(s.bands[0].depth),
        f64::from(s.bands[0].input_db),
        f64::from(s.bands[0].output_db),
        f64::from(s.bands[0].attack_ms),
        f64::from(s.bands[0].release_ms),
        f64::from(s.bands[0].enabled),
        f64::from(s.bands[1].depth),
        f64::from(s.bands[1].input_db),
        f64::from(s.bands[1].output_db),
        f64::from(s.bands[1].attack_ms),
        f64::from(s.bands[1].release_ms),
        f64::from(s.bands[1].enabled),
        f64::from(s.bands[2].depth),
        f64::from(s.bands[2].input_db),
        f64::from(s.bands[2].output_db),
        f64::from(s.bands[2].attack_ms),
        f64::from(s.bands[2].release_ms),
        f64::from(s.bands[2].enabled),
        f64::from(s.level_db),
        f64::from(s.dry_wet),
        f64::from(s.detection as u8),
    ];
    std::array::from_fn(|i| {
        let info = P::ALL[i].info();
        values[i].clamp(info.minimum, info.maximum)
    })
}
