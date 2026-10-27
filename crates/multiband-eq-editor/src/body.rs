//! Frozen automation slots are presented by the same variable-band body as the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_core::{MultibandEqBand, MultibandEqSettings};
use daw_unit_ui::{control_capture, layout::*, meters, multiband_eq};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    2.0 * UNIT_STRIP_WIDTH + COMPACT_BUTTON_SIZE + 2.0 * PANEL_MARGIN as f32,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
#[cfg(test)]
pub const SCROLLABLE: bool = true;
pub struct Bounds {
    pub sections: Vec<Rect>,
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
pub fn visible(settings: &MultibandEqSettings, mut slots: u64) -> u64 {
    let mode = daw_unit_core::EqBandMode::ALL[P::ALL[2].info().default as usize];
    let frequency = P::ALL[3].info().default as f32;
    let gain = P::ALL[4].info().default as f32;
    let q = P::ALL[5].info().default as f32;
    for (i, band) in settings.bands.iter().take(64).enumerate() {
        if band.enabled
            || band.mode != mode
            || band.frequency_hz != frequency
            || band.gain_db != gain
            || band.q != q
        {
            slots |= 1 << i;
        }
    }
    slots
}
pub fn view(settings: &MultibandEqSettings, slots: u64) -> (MultibandEqSettings, Vec<usize>) {
    let indices: Vec<_> = (0..settings.bands.len().min(64))
        .filter(|i| slots & (1 << i) != 0)
        .collect();
    (
        MultibandEqSettings {
            enabled: settings.enabled,
            output_gain_db: settings.output_gain_db,
            bands: indices.iter().map(|i| settings.bands[*i].clone()).collect(),
            ..Default::default()
        },
        indices,
    )
}
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    _: bool,
    _: f32,
    _: &[Option<daw_unit_ui::knobs::ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let id = ui.id().with("visible-band-slots");
    let mut slots = ui.data(|data| data.get_temp::<u64>(id)).unwrap_or(1);
    let result = render(ui, settings, level, &mut slots, &[0.0; 64]);
    ui.data_mut(|data| data.insert_temp(id, slots));
    result
}
pub fn render(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    level: meters::OutputLevel<'_>,
    slots: &mut u64,
    levels: &[f32; 64],
) -> Bounds {
    let before = PluginState::from_settings(settings).ok();
    *slots = visible(settings, *slots);
    let (mut view, indices) = self::view(settings, *slots);
    let active_levels: Vec<_> = indices.iter().map(|i| levels[*i]).collect();
    let (b, captured) = control_capture::scope(ui, |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("bands")
            .show(ui, |ui| {
                multiband_eq::show(
                    ui,
                    &mut view,
                    (false, unit_control_height()),
                    multiband_eq::Controls {
                        add_enabled: indices.len() < 64,
                        markers: &[],
                        levels: &active_levels,
                        output_marker: None,
                        output_level: Some(level),
                    },
                )
            })
            .inner
    });
    for (local, slot) in indices.iter().copied().enumerate() {
        settings.bands[slot] = view.bands[local].clone();
    }
    settings.output_gain_db = view.output_gain_db;
    let mut rects = [Rect::NOTHING; P::COUNT - 1];
    for (local, slot) in indices.iter().copied().enumerate() {
        let base = 5 * slot;
        rects[base] = b.power[local];
        rects[base + 1] = b.knobs[local][0];
        rects[base + 2] = b.knobs[local][1];
        rects[base + 3] = b.knobs[local][2];
        rects[base + 4] = b.knobs[local][3];
    }
    rects[P::Level as usize - 101] = b.output_control;
    let mut direct = None;
    if let Some(local) = b.remove {
        let slot = indices[local];
        *slots &= !(1 << slot);
        let id = settings.bands[slot].id;
        settings.bands[slot] = MultibandEqBand {
            id,
            enabled: false,
            ..Default::default()
        };
    }
    if let Some(add) = &b.add
        && add.clicked()
        && let Some(slot) = (0..settings.bands.len().min(64)).find(|i| *slots & (1 << i) == 0)
    {
        *slots |= 1 << slot;
        settings.bands[slot].enabled = true;
        direct = Some((5 * slot, add.clone()));
    }
    let after = PluginState::from_settings(settings).ok();
    let mut controls = std::array::from_fn(|i| {
        control_capture::at(&captured, rects[i]).map(|mut response| {
            if let (Some(before), Some(after)) = (before, after) {
                let p = P::ALL[i + 1];
                if before.get(p) != after.get(p) {
                    response.mark_changed();
                }
            }
            response
        })
    });
    if let Some((slot, mut response)) = direct {
        response.mark_changed();
        controls[slot] = Some(response);
    }
    Bounds {
        sections: b.sections,
        controls,
        readouts: vec![],
    }
}
