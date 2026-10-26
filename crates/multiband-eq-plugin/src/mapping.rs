use crate::{Parameter as P, PluginState};
pub type Settings = daw_unit_core::MultibandEqSettings;
pub fn to_settings(state: &PluginState) -> Settings {
    let mut settings = Settings {
        bands: (0..64)
            .map(|i| daw_unit_core::MultibandEqBand {
                id: uuid::Uuid::from_u128(i + 1),
                mode: daw_unit_core::EqBandMode::Bell,
                enabled: false,
                frequency_hz: 1000.0,
                gain_db: 0.0,
                q: std::f32::consts::FRAC_1_SQRT_2,
            })
            .collect(),
        ..Settings::default()
    };
    update_settings(state, &mut settings);
    settings
}
pub fn update_settings(state: &PluginState, settings: &mut Settings) {
    settings.enabled = state.get(P::Bypass) == 0.0;
    settings.output_gain_db = daw_unit_core::gain::to_db_f32(state.get(P::Level));
    for (i, band) in settings.bands.iter_mut().enumerate() {
        let base = 1 + 5 * i;
        band.enabled = state.get(P::ALL[base]) != 0.0;
        band.mode = daw_unit_core::EqBandMode::ALL[state.get(P::ALL[base + 1]) as usize];
        band.frequency_hz = state.get(P::ALL[base + 2]) as f32;
        band.gain_db = state.get(P::ALL[base + 3]) as f32;
        band.q = state.get(P::ALL[base + 4]) as f32;
    }
}
pub fn values(settings: &Settings) -> [f64; P::COUNT] {
    std::array::from_fn(|i| {
        if i == 0 {
            return f64::from(!settings.enabled);
        }
        if i == 321 {
            return f64::from(settings.output_gain_db);
        }
        let slot = (i - 1) / 5;
        let field = (i - 1) % 5;
        let Some(band) = settings.bands.get(slot) else {
            return P::ALL[i].info().default;
        };
        match field {
            0 => f64::from(band.enabled),
            1 => f64::from(band.mode as u8),
            2 => f64::from(band.frequency_hz),
            3 => f64::from(band.gain_db),
            _ => f64::from(band.q),
        }
    })
}
