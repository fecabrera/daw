//! Frozen automation slots are presented by the same variable-band body as the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_core::{MultibandEqBand, MultibandEqSettings};
use daw_unit_ui::{control_capture, layout::*, meters, multiband_eq};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    daw_unit_ui::eq_graph::WIDTH
        + 2.0 * UNIT_STRIP_WIDTH
        + COMPACT_BUTTON_SIZE
        + 2.0 * PANEL_MARGIN as f32,
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
    for band in settings.bands.iter().take(64) {
        let i = (band.id.as_u128() - 1) as usize;
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
    let indices: Vec<_> = settings
        .bands
        .iter()
        .map(|b| (b.id.as_u128() - 1) as usize)
        .filter(|i| slots & (1 << i) != 0)
        .collect();
    (
        MultibandEqSettings {
            enabled: settings.enabled,
            output_gain_db: settings.output_gain_db,
            bands: indices
                .iter()
                .map(|i| {
                    settings
                        .bands
                        .iter()
                        .find(|b| b.id.as_u128() == *i as u128 + 1)
                        .unwrap()
                        .clone()
                })
                .collect(),
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
#[cfg(test)]
pub fn render(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    level: meters::OutputLevel<'_>,
    slots: &mut u64,
    levels: &[f32; 64],
) -> Bounds {
    render_audition(
        ui,
        settings,
        level,
        slots,
        &levels.map(Into::into),
        &std::array::from_fn(|_| Default::default()),
    )
}
#[cfg(test)]
pub fn render_audition(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    level: meters::OutputLevel<'_>,
    slots: &mut u64,
    levels: &[meters::OutputLevel<'_>; 64],
    solos: &[std::sync::Arc<std::sync::atomic::AtomicBool>; 64],
) -> Bounds {
    render_visual(ui, settings, level, slots, levels, solos, None)
}
#[allow(clippy::too_many_arguments)]
pub fn render_visual(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    level: meters::OutputLevel<'_>,
    slots: &mut u64,
    levels: &[meters::OutputLevel<'_>; 64],
    solos: &[std::sync::Arc<std::sync::atomic::AtomicBool>; 64],
    spectrum: Option<&daw_unit_core::SpectrumReading>,
) -> Bounds {
    let owner = ui.make_persistent_id("eq-slot-order");
    let reordering = daw_unit_ui::strip_reorder::active(ui, owner);
    let before = PluginState::from_settings(settings).ok();
    *slots = visible(settings, *slots);
    let (mut view, indices) = self::view(settings, *slots);
    let mut selected: Vec<_> = indices
        .iter()
        .map(|i| solos[*i].load(std::sync::atomic::Ordering::Relaxed))
        .collect();
    let original = view.bands.clone();
    let active_levels: Vec<_> = indices.iter().map(|i| levels[*i]).collect();
    let (b, captured) = control_capture::scope(ui, |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("bands")
            .show(ui, |ui| {
                daw_unit_ui::strip_reorder::scroll(ui);
                ui.add_enabled_ui(!reordering, |ui| {
                    multiband_eq::show(
                        ui,
                        &mut view,
                        (false, unit_control_height()),
                        multiband_eq::Controls {
                            graph: daw_unit_ui::eq_graph::Input {
                                live_edits: true,
                                spectrum,
                                sample_rate_hz: spectrum.map_or(48_000, |s| s.sample_rate_hz),
                                effective: &[],
                                output_gain_db: None,
                                response: daw_equalizer::band_response,
                            },
                            solos: &mut selected,
                            add_enabled: indices.len() < 64,
                            markers: &[],
                            levels: &active_levels,
                            output_marker: None,
                            output_level: Some(level),
                        },
                    )
                })
                .inner
            })
            .inner
    });
    for (local, slot) in indices.iter().copied().enumerate() {
        solos[slot].store(
            selected[local] && settings.enabled,
            std::sync::atomic::Ordering::Relaxed,
        );
        *settings
            .bands
            .iter_mut()
            .find(|b| b.id.as_u128() == slot as u128 + 1)
            .unwrap() = view.bands[local].clone();
    }
    settings.output_gain_db = view.output_gain_db;
    let mut order = view.bands.clone();
    let slots_rects: Vec<_> = indices
        .iter()
        .zip(&b.band_responses)
        .map(|(slot, r)| (egui::Id::new(*slot), r.rect))
        .collect();
    for (local, response) in b.band_responses.iter().enumerate() {
        let control = ui
            .input(|i| {
                if i.pointer.any_down() {
                    i.pointer.press_origin()
                } else {
                    i.pointer.hover_pos()
                }
            })
            .is_some_and(|p| {
                b.meters.get(local).is_some_and(|r| r.contains(p))
                    || b.solo[local].contains(p)
                    || b.power[local].contains(p)
                    || b.knobs[local].iter().any(|r| r.contains(p))
            });
        if !control {
            daw_unit_ui::strip_reorder::arm(
                ui,
                owner,
                response,
                egui::Id::new(indices[local]),
                &original,
            );
        }
    }
    if let (Some((_, first)), Some((_, last))) = (slots_rects.first(), slots_rects.last()) {
        daw_unit_ui::strip_reorder::finish(
            ui,
            owner,
            &mut order,
            &slots_rects,
            first.union(*last).intersect(ui.clip_rect()),
        );
    }
    if order
        .iter()
        .map(|v| (v.id.as_u128() - 1) as usize)
        .ne(indices.iter().copied())
    {
        settings
            .bands
            .sort_by_key(|b| order.iter().position(|slot| slot.id == b.id).unwrap_or(64));
    }
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
        solos[slot].store(false, std::sync::atomic::Ordering::Relaxed);
        let index = settings
            .bands
            .iter()
            .position(|b| b.id.as_u128() == slot as u128 + 1)
            .unwrap();
        let id = settings.bands[index].id;
        settings.bands[index] = MultibandEqBand {
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
        settings
            .bands
            .iter_mut()
            .find(|b| b.id.as_u128() == slot as u128 + 1)
            .unwrap()
            .enabled = true;
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
    for (id, parameter, response) in &b.graph.edits {
        let slot = (id.as_u128() - 1) as usize;
        let offset = match parameter {
            0 => 3,
            1 => 2,
            _ => 4,
        };
        controls[5 * slot + offset] = Some(response.clone());
    }
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
