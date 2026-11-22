use crate::{Parameter as P, PluginState};
use daw_unit_core::TransientShaperClipMode;
pub type Settings = daw_unit_core::TransientShaperSettings;

pub fn to_settings(state: &PluginState) -> Settings {
    Settings {
        enabled: state.get(P::Bypass) == 0.0,
        level_db: daw_unit_core::gain::to_db_f32(state.get(P::Level)),
        attack_db: state.get(P::Attack) as f32,
        pump_db: state.get(P::Pump) as f32,
        sustain_db: state.get(P::Sustain) as f32,
        speed: state.get(P::Speed) as f32,
        dry_wet: state.get(P::DryWet) as f32,
        clip_mode: TransientShaperClipMode::ALL[state.get(P::ClipMode) as usize],
        ..Default::default()
    }
}

pub fn values(settings: &Settings) -> [f64; P::COUNT] {
    [
        f64::from(!settings.enabled),
        f64::from(settings.level_db),
        f64::from(settings.attack_db),
        f64::from(settings.pump_db),
        f64::from(settings.sustain_db),
        f64::from(settings.speed),
        f64::from(settings.dry_wet),
        TransientShaperClipMode::ALL
            .iter()
            .position(|mode| *mode == settings.clip_mode)
            .unwrap() as f64,
    ]
}
