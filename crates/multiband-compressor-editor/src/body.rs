use daw_drive_plugin::{Parameter as P, PluginState, Settings};
use daw_unit_ui::{control_capture, layout::*, meters, multiband_compressor as shared};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(720.0, unit_control_height() + 2.0 * PANEL_MARGIN as f32);
pub struct Bounds {
    pub sections: Vec<Rect>,
    pub controls: [Option<Response>; P::COUNT - 1],
}
pub fn render(ui: &mut Ui, s: &mut Settings, model: &crate::Model) -> Bounds {
    let before = PluginState::from_settings(s).ok();
    let owner = ui.make_persistent_id("compressor-band-order");
    let reordering = daw_unit_ui::strip_reorder::active(ui, owner);
    let ids: Vec<_> = s.bands.iter().map(|b| b.id).collect();
    let mut solos: Vec<_> = ids
        .iter()
        .map(|id| {
            model.output_meter.solos[(id.as_u128() - 1) as usize]
                .load(std::sync::atomic::Ordering::Relaxed)
        })
        .collect();
    let original: Vec<_> = s
        .presentation
        .iter()
        .filter_map(|id| s.bands.iter().find(|b| b.id == *id).cloned())
        .collect();
    let levels: Vec<_> = ids
        .iter()
        .map(|id| {
            let slot = (id.as_u128() - 1) as usize;
            let meter = &model.output_meter.eq_bands[slot];
            meters::OutputLevel::new(meter.readings()[0], Some(&meter.boost_peaks()[0]))
                .with_reset_mask(&model.output_meter.band_level_resets, 1_u64 << slot)
        })
        .collect();
    let reductions: Vec<_> = ids
        .iter()
        .map(|id| {
            let meter = &model.output_meter.reductions[(id.as_u128() - 1) as usize];
            meters::GainReduction::new(meter.reading(), Some(meter)).for_unit(s.enabled)
        })
        .collect();
    let level = meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let (b, captured) = control_capture::scope(ui, |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("bands")
            .show(ui, |ui| {
                daw_unit_ui::strip_reorder::scroll(ui);
                ui.add_enabled_ui(!reordering, |ui| {
                    shared::show(
                        ui,
                        &mut s.core,
                        (false, unit_control_height()),
                        shared::Controls {
                            solos: &mut solos,
                            band_markers: &[],
                            output_markers: &[None, None],
                            level,
                            levels: &levels,
                            reductions: &reductions,
                            add_enabled: ids.len() < 64,
                        },
                    )
                })
                .inner
            })
            .inner
    });
    for (id, solo) in ids.iter().zip(solos) {
        model.output_meter.solos[(id.as_u128() - 1) as usize].store(
            solo && s.enabled && s.bands.iter().any(|b| b.id == *id),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
    // New UI bands receive an unused fixed automation slot before publication.
    for index in 0..s.core.bands.len() {
        if s.core.bands[index].id.as_u128() > 64 {
            let slot = (0..64)
                .find(|slot| !ids.iter().any(|id| id.as_u128() == *slot as u128 + 1))
                .unwrap();
            let old = s.core.bands[index].id;
            s.core.bands[index].id = s.slots[slot].id;
            let new = s.core.bands[index].id;
            for id in &mut s.core.presentation {
                if *id == old {
                    *id = new;
                }
            }
        }
    }
    let mut rects = [Rect::NOTHING; P::COUNT - 1];
    for (local, band) in original.iter().enumerate() {
        let slot = (band.id.as_u128() - 1) as usize;
        let base = 10 * slot;
        rects[base + 1] = b.power[local];
        for (target, field) in [3, 4, 5, 6, 7, 8].iter().copied().enumerate() {
            rects[base + field] = b.knobs[local][target];
        }
        rects[base + 9] = b.modes[local];
        if control_capture::at(&captured, b.reductions[local]).is_some_and(|r| r.clicked()) {
            model
                .output_meter
                .band_resets
                .fetch_or(1_u64 << slot, std::sync::atomic::Ordering::Release);
        }
    }
    for (index, rect) in b.crossovers.iter().copied().enumerate() {
        let slot = (ids[index].as_u128() - 1) as usize;
        rects[10 * slot + 2] = rect;
    }
    rects[P::Level as usize - 101] = b.output_knobs[0];
    rects[P::DryWet as usize - 101] = b.output_knobs[1];
    let slots: Vec<_> = original
        .iter()
        .zip(&b.band_responses)
        .map(|(band, r)| (egui::Id::new(band.id), r.rect))
        .collect();
    for (i, (band, r)) in original.iter().zip(&b.band_responses).enumerate() {
        let control = ui
            .input(|input| input.pointer.press_origin())
            .is_some_and(|p| {
                meters::unit_monitor_rail(b.bands[i], PANEL_MARGIN).contains(p)
                    || b.power[i].contains(p)
                    || b.solo[i].contains(p)
                    || b.modes[i].contains(p)
                    || b.knobs[i].iter().any(|r| r.contains(p))
            });
        if !control {
            daw_unit_ui::strip_reorder::arm(ui, owner, r, egui::Id::new(band.id), &original);
        }
    }
    let mut order = original.clone();
    if let (Some((_, first)), Some((_, last))) = (slots.first(), slots.last()) {
        daw_unit_ui::strip_reorder::finish(
            ui,
            owner,
            &mut order,
            &slots,
            first.union(*last).intersect(ui.clip_rect()),
        );
    }
    if order.iter().map(|b| b.id).ne(original.iter().map(|b| b.id)) && order.len() == s.bands.len()
    {
        s.core.presentation = order.iter().map(|b| b.id).collect();
    }
    let after = PluginState::from_settings(s).ok();
    let controls = std::array::from_fn(|i| {
        let mut response = control_capture::at(&captured, rects[i]);
        if let (Some(before), Some(after)) = (before, after)
            && before.get(P::ALL[i + 1]) != after.get(P::ALL[i + 1])
        {
            let r = response.get_or_insert_with(|| {
                ui.interact(
                    Rect::NOTHING,
                    ui.id().with(("structural", i)),
                    egui::Sense::hover(),
                )
            });
            r.mark_changed();
        }
        response
    });
    let mut sections = vec![b.ranges];
    sections.extend(b.bands);
    sections.push(b.add);
    sections.push(b.output);
    Bounds { sections, controls }
}
