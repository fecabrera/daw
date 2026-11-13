//! Crossover and threshold gestures share one draft in DAW hosts and bounded live plugin edits.
use crate::{layout, rows, spectrum_graph as graph, theme};
use daw_unit_core::{ModulationTargetId as BandId, MultibandCompressorSettings, SpectrumReading};
use egui::{Rect, Response, Ui, Vec2};
#[derive(Clone)]
struct Drag {
    frequencies: Vec<(BandId, f32)>,
    threshold: bool,
    enabled: bool,
    band: BandId,
    original: f32,
    preview: f32,
    position: f64,
    last_coordinate: f32,
    frame: u64,
    cancelled: bool,
}
pub(crate) struct Bounds {
    pub section: Rect,
    pub plot: Rect,
    pub markers: Vec<Rect>,
    pub preview: Option<(BandId, f32)>,
    pub edits: Vec<(BandId, Response)>,
    pub thresholds: Vec<Rect>,
    pub threshold_preview: Option<(BandId, f32)>,
    pub threshold_edits: Vec<(BandId, Response)>,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            section: Rect::NOTHING,
            plot: Rect::NOTHING,
            markers: Vec::new(),
            preview: None,
            edits: Vec::new(),
            thresholds: Vec::new(),
            threshold_preview: None,
            threshold_edits: Vec::new(),
        }
    }
}
pub(crate) fn show(
    ui: &mut Ui,
    s: &mut MultibandCompressorSettings,
    dimensions: (bool, f32),
    reading: Option<&SpectrumReading>,
    live: bool,
) -> Bounds {
    let id = ui.id().with("compressor-graph-drag");
    let spectrum_id = ui.id().with("compressor-spectrum");
    let selection_id = ui.id().with("compressor-graph-selection");
    let mut selected = ui
        .data(|d| d.get_temp::<(BandId, bool)>(selection_id))
        .filter(|(id, threshold)| {
            s.bands
                .iter()
                .enumerate()
                .any(|(i, b)| b.id == *id && (*threshold || i + 1 < s.bands.len()))
        });
    let frame = ui.ctx().cumulative_frame_nr();
    let mut drag = ui.data_mut(|d| d.get_temp::<Drag>(id));
    let external = drag.as_ref().is_some_and(|d| {
        d.enabled != s.enabled
            || s.bands.len() != d.frequencies.len()
            || s.bands
                .iter()
                .zip(&d.frequencies)
                .any(|(band, (saved, hz))| {
                    if band.id != *saved {
                        return true;
                    }
                    if band.id == d.band && live && !d.threshold {
                        band.upper_hz != d.preview && (!d.cancelled || band.upper_hz != d.original)
                    } else {
                        band.upper_hz != *hz
                    }
                })
            || (d.threshold
                && s.bands.iter().find(|b| b.id == d.band).is_some_and(|b| {
                    b.threshold_db != if live { d.preview } else { d.original }
                        && (!d.cancelled || b.threshold_db != d.original)
                }))
    });
    let cancel = dimensions.0
        || !ui.is_enabled()
        || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape))
        || drag.as_ref().is_some_and(|d| frame > d.frame + 1);
    if external {
        drag = None;
    } else if let Some(d) = drag.as_mut() {
        if d.cancelled
            && s.bands.iter().any(|b| {
                b.id == d.band
                    && (if d.threshold {
                        b.threshold_db
                    } else {
                        b.upper_hz
                    }) == d.original
            })
        {
            drag = None;
        } else if cancel || d.cancelled {
            if live {
                // Retain a cancelled restoration until the host accepts its value.
                // Bounded editor queues can temporarily prevent publication.
                if let Some(b) = s.bands.iter_mut().find(|b| b.id == d.band) {
                    if d.threshold {
                        b.threshold_db = d.original;
                    } else {
                        b.upper_hz = d.original;
                    }
                }
                d.cancelled = true;
                d.frame = frame;
            } else {
                drag = None;
            }
        }
    }
    let mut bounds = Bounds {
        section: Rect::NOTHING,
        plot: Rect::NOTHING,
        ..Default::default()
    };
    if dimensions.0 {
        crate::eq_spectrum::clear(ui, spectrum_id);
        ui.data_mut(|data| {
            if let Some(d) = drag {
                data.insert_temp(id, d);
            } else {
                data.remove::<Drag>(id);
            }
        });
        return bounds;
    }
    let section = layout::unit_strip_with_width(
        ui,
        dimensions.1,
        graph::WIDTH,
        egui::UiBuilder::new(),
        |ui| {
            let start = ui.cursor().top();
            let (chart, _) = ui.allocate_exact_size(
                Vec2::new(
                    graph::WIDTH - 2.0 * f32::from(layout::PANEL_MARGIN),
                    3.0 * crate::knobs::CHOICE_SIZE + 2.0 * f32::from(theme::TOOLBAR_PADDING),
                ),
                egui::Sense::hover(),
            );
            let plot = graph::axes(ui, chart, (-90.0, 0.0), &[-90.0, -60.0, -30.0, 0.0]);
            bounds.plot = plot;
            graph::paint(ui, spectrum_id, plot, reading, s.enabled);
            for threshold in [false, true] {
                for index in 0..s.bands.len().saturating_sub(usize::from(!threshold)) {
                    let band = s.bands[index].id;
                    let hz = drag
                        .as_ref()
                        .filter(|d| d.band == band && d.threshold == threshold && !d.cancelled)
                        .map_or(
                            if threshold {
                                s.bands[index].threshold_db
                            } else {
                                s.bands[index].upper_hz
                            },
                            |d| d.preview,
                        );
                    let x = if threshold {
                        plot.left()
                    } else {
                        graph::frequency_x(plot, hz)
                    };
                    // Split overlapping hit regions at their midpoint so a nearby
                    // later crossover cannot steal a press from this line.
                    let displayed = |i: usize| {
                        drag.as_ref()
                            .filter(|d| d.band == s.bands[i].id && !d.threshold && !d.cancelled)
                            .map_or(s.bands[i].upper_hz, |d| d.preview)
                    };
                    let left = if index > 0 {
                        (graph::frequency_x(plot, displayed(index - 1)) + x) * 0.5
                    } else {
                        plot.left()
                    };
                    let right = if index + 2 < s.bands.len() {
                        (graph::frequency_x(plot, displayed(index + 1)) + x) * 0.5
                    } else {
                        plot.right()
                    };
                    let hit = if threshold {
                        let left = if index == 0 {
                            plot.left()
                        } else {
                            graph::frequency_x(plot, displayed(index - 1))
                        };
                        let right = if index + 1 == s.bands.len() {
                            plot.right()
                        } else {
                            graph::frequency_x(plot, displayed(index))
                        };
                        let inset = 5.0_f32.min((right - left) * 0.25);
                        let y = plot.bottom() - (hz + 90.0) / 90.0 * plot.height();
                        Rect::from_x_y_ranges(left + inset..=right - inset, y - 8.5..=y + 8.5)
                            .intersect(chart)
                    } else {
                        Rect::from_x_y_ranges(
                            (x - 4.0).max(left)..=(x + 4.0).min(right),
                            plot.y_range(),
                        )
                    };
                    let mut response = ui
                        .interact(
                            hit,
                            id.with((band, threshold)),
                            egui::Sense::click_and_drag(),
                        )
                        .on_hover_cursor(if threshold {
                            egui::CursorIcon::ResizeVertical
                        } else {
                            egui::CursorIcon::ResizeHorizontal
                        });
                    if response.enabled()
                        && (response.clicked()
                            || response.drag_started()
                            || (response.is_pointer_button_down_on()
                                && ui.input(|i| i.pointer.primary_pressed())))
                    {
                        selected = Some((band, threshold));
                    }
                    if ui.is_enabled()
                        && ui.input(|i| i.focused)
                        && !cancel
                        && drag.is_none()
                        && (response.drag_started()
                            || (response.is_pointer_button_down_on()
                                && ui.input(|i| i.pointer.primary_pressed())))
                    {
                        response.request_focus();
                        let start = ui
                            .input(|i| i.pointer.press_origin())
                            .unwrap_or(hit.center());
                        drag = Some(Drag {
                            frequencies: s.bands.iter().map(|b| (b.id, b.upper_hz)).collect(),
                            threshold,
                            enabled: s.enabled,
                            band,
                            original: hz,
                            preview: hz,
                            position: if threshold {
                                f64::from(hz)
                            } else {
                                f64::from(hz).ln()
                            },
                            last_coordinate: if threshold { start.y } else { start.x },
                            frame,
                            cancelled: false,
                        });
                    }
                    if let Some(d) = drag
                        .as_mut()
                        .filter(|d| d.band == band && d.threshold == threshold && !d.cancelled)
                    {
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            let current = if threshold { pos.y } else { pos.x };
                            let delta = current - d.last_coordinate;
                            d.last_coordinate = current;
                            if delta != 0.0 {
                                let fine = ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
                                if threshold {
                                    d.position = (d.position
                                        - f64::from(delta * fine * 90.0 / plot.height()))
                                    .clamp(-60.0, 0.0);
                                    d.preview = d.position as f32;
                                } else {
                                    let min = if index == 0 {
                                        20.0
                                    } else {
                                        s.bands[index - 1].upper_hz + 1.0
                                    };
                                    let max = s.bands[index + 1].upper_hz - 1.0;
                                    d.position = (d.position
                                        + f64::from(delta * fine / plot.width()) * 1000.0_f64.ln())
                                    .clamp(f64::from(min).ln(), f64::from(max).ln());
                                    d.preview = (d.position.exp() as f32).clamp(min, max);
                                }
                            }
                        }
                        d.frame = frame;
                        if threshold {
                            bounds.threshold_preview = Some((band, d.preview));
                        } else {
                            bounds.preview = Some((band, d.preview));
                        }
                        if live || !ui.input(|i| i.pointer.primary_down()) {
                            let value = if threshold {
                                &mut s.bands[index].threshold_db
                            } else {
                                &mut s.bands[index].upper_hz
                            };
                            if *value != d.preview {
                                *value = d.preview;
                                response.mark_changed();
                            }
                            if threshold {
                                bounds.threshold_edits.push((band, response.clone()));
                            } else {
                                bounds.edits.push((band, response.clone()));
                            }
                        }
                    }
                    let drawn = drag
                        .as_ref()
                        .filter(|d| d.band == band && d.threshold == threshold && !d.cancelled)
                        .map_or(
                            if threshold {
                                s.bands[index].threshold_db
                            } else {
                                s.bands[index].upper_hz
                            },
                            |d| d.preview,
                        );
                    let label = if threshold {
                        let displayed = |i: usize| {
                            drag.as_ref()
                                .filter(|d| d.band == s.bands[i].id && !d.threshold && !d.cancelled)
                                .map_or(s.bands[i].upper_hz, |d| d.preview)
                        };
                        let left = if index == 0 {
                            plot.left()
                        } else {
                            graph::frequency_x(plot, displayed(index - 1))
                        };
                        let right = if index + 1 == s.bands.len() {
                            plot.right()
                        } else {
                            graph::frequency_x(plot, displayed(index))
                        };
                        let y = plot.bottom() - (drawn + 90.0) / 90.0 * plot.height();
                        bounds
                            .thresholds
                            .push(Rect::from_x_y_ranges(left..=right, y - 4.0..=y + 4.0));
                        format!(
                            "Band {} threshold: {:.1} dBFS\nDrag vertically. Shift: fine. Escape cancels.",
                            index + 1,
                            drawn
                        )
                    } else {
                        let x = graph::frequency_x(plot, drawn);
                        bounds
                            .markers
                            .push(Rect::from_x_y_ranges(x - 4.0..=x + 4.0, plot.y_range()));
                        format!(
                            "Crossover {}: {}\nDrag horizontally. Shift: fine. Escape cancels.",
                            index + 1,
                            crate::knobs::format_frequency(drawn)
                        )
                    };
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::DragValue,
                            response.enabled(),
                            &label,
                        )
                    });
                    let response = response.on_hover_text(label);
                    crate::control_capture::record(ui, &response);
                }
            }
            // Resolve selection before painting so only one graph control is accented.
            for (index, marker) in bounds.markers.iter().enumerate() {
                let active = selected == Some((s.bands[index].id, false));
                let color = if active && s.enabled {
                    theme::accent(ui.ctx()).gamma_multiply(0.65)
                } else {
                    theme::palette(ui.ctx()).secondary.gamma_multiply(0.3)
                };
                ui.painter_at(plot).vline(
                    marker.center().x,
                    plot.y_range(),
                    egui::Stroke::new(1.0, color),
                );
            }
            // Resolve selection for every band before painting any handle.
            // Switching to a later band must not leave the previous one accented.
            for (index, marker) in bounds.thresholds.iter().enumerate() {
                let active = selected == Some((s.bands[index].id, true));
                let line_color = if active && s.enabled && s.bands[index].enabled {
                    theme::accent(ui.ctx()).gamma_multiply(0.65)
                } else {
                    theme::palette(ui.ctx()).secondary.gamma_multiply(0.3)
                };
                let inset = 1.0_f32.min(marker.width() * 0.25);
                ui.painter_at(plot).hline(
                    marker.left() + inset..=marker.right() - inset,
                    marker.center().y,
                    egui::Stroke::new(1.0, line_color),
                );
                graph::band_handle(
                    ui,
                    &ui.painter_at(chart),
                    marker.center(),
                    5.5,
                    active,
                    s.enabled && s.bands[index].enabled,
                );
            }
            if drag.as_ref().is_some_and(|d| !d.cancelled)
                && !ui.input(|i| i.pointer.primary_down())
            {
                drag = None;
                bounds.preview = None;
                bounds.threshold_preview = None;
            }
            crate::multiband_compressor::footer_space(ui, start, dimensions.1);
            rows::centered(ui, 18.0, |ui| rows::unit_label(ui, "Response"));
        },
    );
    bounds.section = section.response.rect;
    ui.data_mut(|data| {
        if let Some(d) = drag {
            data.insert_temp(id, d);
        } else {
            data.remove::<Drag>(id);
        }
    });
    ui.data_mut(|data| {
        if let Some(band) = selected {
            data.insert_temp(selection_id, band);
        } else {
            data.remove::<(BandId, bool)>(selection_id);
        }
    });
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pointer(p: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn draw(
        ctx: &egui::Context,
        s: &mut MultibandCompressorSettings,
        events: Vec<egui::Event>,
        live: bool,
        focused: bool,
        enabled: bool,
        compact: bool,
    ) -> Bounds {
        let mut b = Bounds::default();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500.0, 300.0),
                )),
                focused,
                events,
                ..Default::default()
            },
            |ui| {
                b = ui
                    .add_enabled_ui(enabled, |ui| {
                        show(ui, s, (compact, layout::unit_control_height()), None, live)
                    })
                    .inner;
            },
        );
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("graph-test-shapes"), out.shapes.clone()));
        out.textures_delta.clear();
        b
    }
    #[test]
    fn crossover_drag_previews_commits_by_id_and_clamps_between_neighbors() {
        for live in [false, true] {
            let ctx = egui::Context::default();
            let mut s = MultibandCompressorSettings::default();
            let before = s.clone();
            let b = draw(&ctx, &mut s, vec![], live, true, true, false);
            let start = b.markers[0].center();
            let end = start + Vec2::new(20.0, 0.0);
            draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(start), pointer(start, true)],
                live,
                true,
                true,
                false,
            );
            let b = draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(end)],
                live,
                true,
                true,
                false,
            );
            let (id, hz) = b.preview.unwrap();
            assert_eq!(id, before.bands[0].id);
            assert!(hz > before.bands[0].upper_hz);
            assert_eq!(
                s.bands[0].upper_hz,
                if live { hz } else { before.bands[0].upper_hz }
            );
            if live {
                assert!(
                    b.edits
                        .iter()
                        .any(|(band, r)| *band == id && r.changed() && r.dragged())
                );
            }
            let far = end + Vec2::new(2000.0, 0.0);
            let b = draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(far)],
                live,
                true,
                true,
                false,
            );
            assert_eq!(b.preview.unwrap().1, before.bands[1].upper_hz - 1.0);
            draw(
                &ctx,
                &mut s,
                vec![pointer(far, false)],
                live,
                true,
                true,
                false,
            );
            assert_eq!(s.bands[0].upper_hz, before.bands[1].upper_hz - 1.0);
            assert_eq!(s.bands[1..], before.bands[1..]);
        }
    }
    #[test]
    fn crossover_drag_cancels_on_escape_focus_disable_collapse_and_external_changes() {
        for live in [false, true] {
            for mode in 0..5 {
                let ctx = egui::Context::default();
                let mut s = MultibandCompressorSettings::default();
                let before = s.clone();
                let b = draw(&ctx, &mut s, vec![], live, true, true, false);
                let start = b.markers[0].center();
                let end = start + Vec2::new(20.0, 0.0);
                draw(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(start), pointer(start, true)],
                    live,
                    true,
                    true,
                    false,
                );
                draw(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(end)],
                    live,
                    true,
                    true,
                    false,
                );
                if mode == 4 {
                    s.bands[0].upper_hz = 500.0;
                }
                let events = if mode == 0 {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }]
                } else {
                    vec![]
                };
                let b = draw(&ctx, &mut s, events, live, mode != 1, mode != 2, mode == 3);
                assert!(b.preview.is_none());
                draw(
                    &ctx,
                    &mut s,
                    vec![pointer(end, false)],
                    live,
                    true,
                    true,
                    false,
                );
                assert_eq!(
                    s.bands[0].upper_hz,
                    if mode == 4 {
                        500.0
                    } else {
                        before.bands[0].upper_hz
                    }
                );
            }
        }
    }
    #[test]
    fn adjacent_crossover_lines_remain_individually_grabbable() {
        let ctx = egui::Context::default();
        let mut s = MultibandCompressorSettings::default();
        s.bands[0].upper_hz = 999.0;
        s.bands[1].upper_hz = 1000.0;
        let b = draw(&ctx, &mut s, vec![], false, true, true, false);
        let start = b.markers[0].center();
        draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
            false,
            true,
            true,
            false,
        );
        let end = start - Vec2::new(20.0, 0.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(end)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap().0, s.bands[0].id);
        draw(
            &ctx,
            &mut s,
            vec![pointer(end, false)],
            false,
            true,
            true,
            false,
        );
        assert!(s.bands[0].upper_hz < 999.0);
        assert_eq!(s.bands[1].upper_hz, 1000.0);
    }
    #[test]
    fn crossover_drag_has_no_press_jump_and_shift_changes_only_future_movement() {
        let ctx = egui::Context::default();
        let mut s = MultibandCompressorSettings::default();
        let before = s.clone();
        let b = draw(&ctx, &mut s, vec![], false, true, true, false);
        let start = b.markers[0].center() + Vec2::new(3.0, 0.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap().1, before.bands[0].upper_hz);
        let fine = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(fine)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap().1, before.bands[0].upper_hz);
        let end = start + Vec2::new(20.0, 0.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(end)],
            false,
            true,
            true,
            false,
        );
        let hz = b.preview.unwrap().1;
        assert!(hz > 200.0 && hz < 220.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(Default::default())],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap().1, hz);
        draw(
            &ctx,
            &mut s,
            vec![pointer(end, false)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(s.bands[0].upper_hz, hz);
    }
    #[test]
    fn threshold_drag_previews_commits_by_id_and_clamps_to_zero() {
        for live in [false, true] {
            let ctx = egui::Context::default();
            let mut s = MultibandCompressorSettings::default();
            let before = s.clone();
            let b = draw(&ctx, &mut s, vec![], live, true, true, false);
            let start = b.thresholds[0].center();
            let end = start + Vec2::new(0.0, -15.0);
            draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(start), pointer(start, true)],
                live,
                true,
                true,
                false,
            );
            let b = draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(end)],
                live,
                true,
                true,
                false,
            );
            let (id, hz) = b.threshold_preview.unwrap();
            assert_eq!(id, before.bands[0].id);
            assert!(hz > before.bands[0].threshold_db);
            assert_eq!(
                s.bands[0].threshold_db,
                if live {
                    hz
                } else {
                    before.bands[0].threshold_db
                }
            );
            if live {
                assert!(
                    b.threshold_edits
                        .iter()
                        .any(|(band, r)| *band == id && r.changed() && r.dragged())
                );
            }
            let far = end + Vec2::new(0.0, -2000.0);
            let b = draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(far)],
                live,
                true,
                true,
                false,
            );
            assert_eq!(b.threshold_preview.unwrap().1, 0.0);
            draw(
                &ctx,
                &mut s,
                vec![pointer(far, false)],
                live,
                true,
                true,
                false,
            );
            assert_eq!(s.bands[0].threshold_db, 0.0);
            assert_eq!(s.bands[1..], before.bands[1..]);
        }
    }
    #[test]
    fn threshold_drag_cancels_on_escape_focus_disable_collapse_and_external_changes() {
        for live in [false, true] {
            for mode in 0..5 {
                let ctx = egui::Context::default();
                let mut s = MultibandCompressorSettings::default();
                let before = s.clone();
                let b = draw(&ctx, &mut s, vec![], live, true, true, false);
                let start = b.thresholds[0].center();
                let end = start + Vec2::new(0.0, -15.0);
                draw(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(start), pointer(start, true)],
                    live,
                    true,
                    true,
                    false,
                );
                draw(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(end)],
                    live,
                    true,
                    true,
                    false,
                );
                if mode == 4 {
                    s.bands[0].threshold_db = -32.0;
                }
                let events = if mode == 0 {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }]
                } else {
                    vec![]
                };
                let b = draw(&ctx, &mut s, events, live, mode != 1, mode != 2, mode == 3);
                assert!(b.threshold_preview.is_none());
                draw(
                    &ctx,
                    &mut s,
                    vec![pointer(end, false)],
                    live,
                    true,
                    true,
                    false,
                );
                assert_eq!(
                    s.bands[0].threshold_db,
                    if mode == 4 {
                        -32.0
                    } else {
                        before.bands[0].threshold_db
                    }
                );
            }
        }
    }
    #[test]
    fn threshold_drag_has_no_press_jump_and_shift_changes_only_future_movement() {
        let ctx = egui::Context::default();
        let mut s = MultibandCompressorSettings::default();
        let before = s.clone();
        let b = draw(&ctx, &mut s, vec![], false, true, true, false);
        let start = b.thresholds[0].center() + Vec2::new(0.0, 3.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.threshold_preview.unwrap().1, before.bands[0].threshold_db);
        let fine = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(fine)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.threshold_preview.unwrap().1, before.bands[0].threshold_db);
        let end = start + Vec2::new(0.0, -15.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(end)],
            false,
            true,
            true,
            false,
        );
        let hz = b.threshold_preview.unwrap().1;
        assert!(hz > -18.0 && hz < -16.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(Default::default())],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.threshold_preview.unwrap().1, hz);
        draw(
            &ctx,
            &mut s,
            vec![pointer(end, false)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(s.bands[0].threshold_db, hz);
    }
    #[test]
    fn threshold_lower_bound_and_crossover_intersection_keep_band_identity() {
        for threshold in [false, true] {
            let ctx = egui::Context::default();
            let mut s = MultibandCompressorSettings::default();
            let before = s.clone();
            let b = draw(&ctx, &mut s, vec![], false, true, true, false);
            let start = if threshold {
                b.thresholds[1].center()
            } else {
                egui::pos2(b.markers[0].center().x, b.thresholds[0].center().y)
            };
            draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(start), pointer(start, true)],
                false,
                true,
                true,
                false,
            );
            let end = start
                + if threshold {
                    Vec2::new(0.0, 2000.0)
                } else {
                    Vec2::new(-20.0, 0.0)
                };
            let b = draw(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(end)],
                false,
                true,
                true,
                false,
            );
            if threshold {
                assert_eq!(b.threshold_preview, Some((s.bands[1].id, -60.0)));
                assert!(b.preview.is_none());
            } else {
                assert_eq!(b.preview.unwrap().0, s.bands[0].id);
                assert!(b.threshold_preview.is_none());
            }
            draw(
                &ctx,
                &mut s,
                vec![pointer(end, false)],
                false,
                true,
                true,
                false,
            );
            if threshold {
                let mut expected = before;
                expected.bands[1].threshold_db = -60.0;
                assert_eq!(s, expected);
            } else {
                assert!(s.bands[0].upper_hz < before.bands[0].upper_hz);
                assert_eq!(s.bands[0].threshold_db, before.bands[0].threshold_db);
                assert_eq!(s.bands[1..], before.bands[1..]);
            }
        }
    }
    #[test]
    fn threshold_handles_share_eq_style_and_keep_only_the_selected_band_accented() {
        for (mode, palette) in [
            (egui::Theme::Dark, theme::LIGHT),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            for scale in [1.0, 1.25, 2.0] {
                let ctx = egui::Context::default();
                crate::fonts::configure(&ctx);
                theme::configure(&ctx);
                theme::apply(&ctx, mode, palette, theme::ACCENT);
                ctx.set_pixels_per_point(scale);
                let mut s = MultibandCompressorSettings::default();
                let before = s.clone();
                let check = |bounds: &Bounds, selected: Option<(usize, bool)>| {
                    let shapes = ctx
                        .data(|d| {
                            d.get_temp::<Vec<egui::epaint::ClippedShape>>(egui::Id::new(
                                "graph-test-shapes",
                            ))
                        })
                        .unwrap();
                    for (index, marker) in bounds.thresholds.iter().enumerate() {
                        let active = selected == Some((index, true));
                        let color = if active {
                            theme::accent(&ctx)
                        } else {
                            theme::palette(&ctx).secondary
                        };
                        let circle = shapes
                            .iter()
                            .find_map(|p| match &p.shape {
                                egui::Shape::Circle(c)
                                    if c.center.distance(marker.center()) < 0.01
                                        && c.stroke.width == 1.25 =>
                                {
                                    Some(c)
                                }
                                _ => None,
                            })
                            .expect("threshold handle missing");
                        assert_eq!(circle.stroke.color, color);
                        assert_eq!(circle.radius, if active { 6.5 } else { 5.5 });
                        let line_color = if active {
                            color.gamma_multiply(0.65)
                        } else {
                            color.gamma_multiply(0.3)
                        };
                        assert!(shapes.iter().any(|p| matches!(&p.shape,egui::Shape::LineSegment { points, stroke } if points[0].y == marker.center().y && points[1].y == marker.center().y && stroke.color == line_color)));
                    }
                    for (index, marker) in bounds.markers.iter().enumerate() {
                        let color = if selected == Some((index, false)) {
                            theme::accent(&ctx).gamma_multiply(0.65)
                        } else {
                            theme::palette(&ctx).secondary.gamma_multiply(0.3)
                        };
                        assert!(shapes.iter().any(|p| matches!(&p.shape,egui::Shape::LineSegment { points, stroke } if points[0].x == marker.center().x && points[1].x == marker.center().x && points[0].y == bounds.plot.top() && points[1].y == bounds.plot.bottom() && stroke.color == color)));
                    }
                    assert!(!shapes.iter().any(|p| matches!(&p.shape,egui::Shape::Text(t) if bounds.plot.contains(t.pos))), "threshold handles must not contain numbers");
                };
                let b = draw(&ctx, &mut s, vec![], false, true, true, false);
                check(&b, None);
                for (selected, threshold) in [
                    (0, true),
                    (1, true),
                    (2, true),
                    (0, false),
                    (1, false),
                    (0, true),
                ] {
                    let point = if threshold {
                        b.thresholds[selected].center()
                    } else {
                        b.markers[selected].center()
                    };
                    let pressed = draw(
                        &ctx,
                        &mut s,
                        vec![egui::Event::PointerMoved(point), pointer(point, true)],
                        false,
                        true,
                        true,
                        false,
                    );
                    check(&pressed, Some((selected, threshold)));
                    let b = draw(
                        &ctx,
                        &mut s,
                        vec![pointer(point, false)],
                        false,
                        true,
                        true,
                        false,
                    );
                    check(&b, Some((selected, threshold)));
                    assert_eq!(s, before, "selection must not edit settings");
                    let b = draw(&ctx, &mut s, vec![], false, true, true, false);
                    check(&b, Some((selected, threshold)));
                }
            }
        }
    }
}
