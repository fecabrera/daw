//! Shared EQ graph. DAW hosts stage transient plans; native editors publish bounded host gestures.
use crate::{layout, rows, theme};
use daw_unit_core::{
    FrequencyResponse, MultibandEqBand, MultibandEqSettings, SpectrumReading, spectrum_frequency,
};
use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
pub const WIDTH: f32 = 3.0 * layout::UNIT_STRIP_WIDTH;
const POINTS: usize = 160;
#[derive(Clone, Copy)]
pub struct Input<'a> {
    /// Native editors publish normal host parameter gestures while dragging.
    pub live_edits: bool,
    pub spectrum: Option<&'a SpectrumReading>,
    pub sample_rate_hz: u32,
    pub effective: &'a [Option<[f32; 3]>],
    pub output_gain_db: Option<f32>,
    pub response: fn(&MultibandEqBand, u32) -> FrequencyResponse,
}
#[derive(Clone)]
struct Drag {
    bands: Vec<MultibandEqBand>,
    enabled: bool,
    id: daw_unit_core::ModulationTargetId,
    preview: MultibandEqBand,
    last: Pos2,
    offset: [f64; 2],
    q: bool,
    frame: u64,
}
pub struct Bounds {
    pub section: Rect,
    pub plot: Rect,
    pub nodes: Vec<Rect>,
    /// Held draft for hosts that stage transient audio plans.
    pub preview: Option<MultibandEqBand>,
    /// Band identity, shared Gain/Frequency/Q index, and gesture response.
    pub edits: Vec<(daw_unit_core::ModulationTargetId, usize, Response)>,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            section: Rect::NOTHING,
            plot: Rect::NOTHING,
            nodes: Vec::new(),
            preview: None,
            edits: Vec::new(),
        }
    }
}
fn x(rect: Rect, hz: f32) -> f32 {
    rect.left() + rect.width() * (hz.clamp(20.0, 20_000.0) / 20.0).log10() / 3.0
}
fn y(rect: Rect, db: f32) -> f32 {
    rect.center().y - rect.height() * db.clamp(-24.0, 24.0) / 48.0
}
fn node_radius(index: usize) -> f32 {
    5.5_f32.max((index + 1).to_string().len() as f32 * 2.5 + 1.5)
}
fn point(rect: Rect, b: &MultibandEqBand) -> Pos2 {
    egui::pos2(
        x(rect, b.frequency_hz),
        y(rect, if b.mode.uses_gain() { b.gain_db } else { 0.0 }),
    )
}
pub fn show(
    ui: &mut Ui,
    s: &mut MultibandEqSettings,
    height: f32,
    visible: bool,
    input: Input<'_>,
) -> Bounds {
    let id = ui.make_persistent_id("eq-graph-drag");
    let selection = ui.make_persistent_id("eq-graph-selection");
    let frame = ui.ctx().cumulative_frame_nr();
    let mut drag = ui.data_mut(|d| d.get_temp::<Drag>(id));
    let external_change = drag.as_ref().is_some_and(|d| {
        let expected = if input.live_edits {
            Some(&d.preview)
        } else {
            d.bands.iter().find(|b| b.id == d.id)
        };
        !s.bands
            .iter()
            .map(|b| b.id)
            .eq(d.bands.iter().map(|b| b.id))
            || s.bands.iter().find(|b| b.id == d.id) != expected
            || d.enabled != s.enabled
    });
    let cancelled = !visible
        || !ui.is_enabled()
        || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape))
        || drag.as_ref().is_some_and(|d| frame > d.frame + 1);
    if external_change || cancelled {
        if input.live_edits
            && !external_change
            && let Some(d) = &drag
            && let Some(original) = d.bands.iter().find(|b| b.id == d.id)
            && let Some(b) = s.bands.iter_mut().find(|b| b.id == d.id)
        {
            *b = original.clone();
        }
        drag = None;
    }
    if !visible {
        ui.data_mut(|d| d.remove::<Drag>(id));
        return Bounds::default();
    }
    let mut selected = ui.data(|d| d.get_temp::<daw_unit_core::ModulationTargetId>(selection));
    let mut bounds = Bounds::default();
    let strip = layout::unit_strip_with_width(ui, height, WIDTH, egui::UiBuilder::new(), |ui| {
        let (chart, _) = ui.allocate_exact_size(
            Vec2::new(
                WIDTH - 2.0 * f32::from(layout::PANEL_MARGIN),
                3.0 * crate::knobs::CHOICE_SIZE + 2.0 * f32::from(theme::TOOLBAR_PADDING),
            ),
            Sense::hover(),
        );
        let plot = chart.shrink2(Vec2::new(24.0, 17.0));
        bounds.plot = plot;
        let palette = theme::palette(ui.ctx());
        let accent = theme::accent(ui.ctx());
        let painter = ui.painter_at(chart);
        painter.rect_filled(chart, 2.0, palette.background);
        let grid = Stroke::new(1.0, palette.secondary.gamma_multiply(0.12));
        let font = egui::FontId::proportional(9.0);
        for hz in [
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ] {
            painter.vline(x(plot, hz), plot.y_range(), grid);
            if [20.0, 100.0, 1000.0, 10000.0, 20000.0].contains(&hz) {
                let label = if hz >= 1000.0 {
                    format!("{:.0}k", hz / 1000.0)
                } else {
                    format!("{hz:.0}")
                };
                painter.text(
                    egui::pos2(x(plot, hz), plot.bottom() + 4.0),
                    egui::Align2::CENTER_TOP,
                    label,
                    font.clone(),
                    palette.secondary,
                );
            }
        }
        for db in [-24.0, -12.0, 0.0, 12.0, 24.0] {
            painter.hline(
                plot.x_range(),
                y(plot, db),
                if db == 0.0 {
                    Stroke::new(1.0, palette.secondary.gamma_multiply(0.5))
                } else {
                    grid
                },
            );
            painter.text(
                egui::pos2(plot.left() - 4.0, y(plot, db)),
                egui::Align2::RIGHT_CENTER,
                format!("{db:+.0}"),
                font.clone(),
                palette.secondary,
            );
        }
        let p = ui.painter_at(plot);
        if s.enabled
            && let Some(spectrum) = input.spectrum
            && spectrum.magnitudes.iter().any(|v| *v > 0.0)
        {
            let points: Vec<_> = spectrum
                .magnitudes
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let db = 20.0 * v.max(1e-8).log10();
                    egui::pos2(
                        x(plot, spectrum_frequency(i)),
                        plot.bottom() - plot.height() * ((db + 90.0) / 90.0).clamp(0.0, 1.0),
                    )
                })
                .collect();
            // A triangle fan avoids convex-fill assumptions for a non-convex spectrum skyline.
            let mut mesh = egui::Mesh::default();
            for pair in points.windows(2) {
                let base = mesh.vertices.len() as u32;
                for pos in [
                    egui::pos2(pair[0].x, plot.bottom()),
                    pair[0],
                    pair[1],
                    egui::pos2(pair[1].x, plot.bottom()),
                ] {
                    mesh.colored_vertex(pos, palette.secondary.gamma_multiply(0.12));
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
            p.add(egui::Shape::mesh(mesh));
            p.add(egui::Shape::line(
                points,
                Stroke::new(1.0, palette.secondary.gamma_multiply(0.5)),
            ));
        }
        for (i, b) in s.bands.iter().enumerate() {
            let node = point(
                plot,
                drag.as_ref()
                    .filter(|d| d.id == b.id)
                    .map_or(b, |d| &d.preview),
            );
            let rect = Rect::from_center_size(node, Vec2::splat((node_radius(i) + 3.0) * 2.0));
            bounds.nodes.push(rect);
            let response=ui.interact(rect,ui.id().with(("eq-point",b.id)),Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(format!("Band {} · {}\nDrag: Frequency{}. {}-drag: Q only (up/right increases; down/left decreases). Shift: fine. Live audio preview; release commits; Escape cancels.",i+1,b.mode.label(),if b.mode.uses_gain(){" / Gain"}else{""},if cfg!(target_os="macos"){"Command"}else{"Ctrl"}));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::DragValue,
                    response.enabled(),
                    format!(
                        "EQ band {} point: Frequency {:.0} Hz, Gain {:.1} dB, Q {:.2}",
                        i + 1,
                        b.frequency_hz,
                        b.gain_db,
                        b.q
                    ),
                )
            });
            if response.clicked() || response.drag_started() {
                selected = Some(b.id);
            }
            if response.drag_started()
                && drag.is_none()
                && ui.is_enabled()
                && ui.input(|i| i.focused)
            {
                let last = ui.input(|i| i.pointer.press_origin()).unwrap_or(node);
                drag = Some(Drag {
                    bands: s.bands.clone(),
                    enabled: s.enabled,
                    id: b.id,
                    preview: b.clone(),
                    last,
                    offset: [0.0; 2],
                    q: ui.input(|i| i.modifiers.command),
                    frame,
                });
            }
            if let Some(d) = &mut drag
                && d.id == b.id
            {
                if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                    let delta = (pointer - d.last)
                        * ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
                    d.last = pointer;
                    d.offset[0] += f64::from(delta.x);
                    d.offset[1] += f64::from(delta.y);
                    for value in &mut d.offset {
                        if value.abs() < 1e-6 {
                            *value = 0.0;
                        }
                    }
                    let original = d
                        .bands
                        .iter()
                        .find(|b| b.id == d.id)
                        .expect("validated band identity");
                    if d.q {
                        d.preview.q = (f64::from(original.q)
                            * 100_f64.powf(
                                d.offset[0] / f64::from(plot.width())
                                    - d.offset[1] / f64::from(plot.height()),
                            ))
                        .clamp(0.1, 10.0) as f32;
                    } else {
                        d.preview.frequency_hz = (f64::from(original.frequency_hz)
                            * 1000_f64.powf(d.offset[0] / f64::from(plot.width())))
                        .clamp(20.0, 20_000.0)
                            as f32;
                        if d.preview.mode.uses_gain() {
                            d.preview.gain_db = (f64::from(original.gain_db)
                                - 48.0 * d.offset[1] / f64::from(plot.height()))
                            .clamp(-24.0, 24.0)
                                as f32;
                        }
                    }
                }
                d.frame = frame;
                if input.live_edits || !ui.input(|i| i.pointer.primary_down()) {
                    for (parameter, before, after) in [
                        (1, b.frequency_hz, d.preview.frequency_hz),
                        (0, b.gain_db, d.preview.gain_db),
                        (2, b.q, d.preview.q),
                    ] {
                        if before != after
                            || (input.live_edits
                                && (if d.q {
                                    parameter == 2
                                } else {
                                    parameter == 1 || (parameter == 0 && b.mode.uses_gain())
                                }))
                        {
                            let mut r = response.clone();
                            if before != after {
                                r.mark_changed();
                            }
                            bounds.edits.push((b.id, parameter, r));
                        }
                    }
                }
            }
        }
        let view: Vec<_> = s
            .bands
            .iter()
            .enumerate()
            .map(|(i, b)| {
                if let Some(d) = &drag
                    && d.id == b.id
                {
                    return d.preview.clone();
                }
                let mut b = b.clone();
                if let Some(Some([gain, hz, q])) = input.effective.get(i)
                    && [*gain, *hz, *q].iter().all(|v| v.is_finite())
                {
                    b.gain_db = *gain;
                    b.frequency_hz = *hz;
                    b.q = *q;
                }
                b
            })
            .collect();
        let mut combined = vec![
            if s.enabled {
                input.output_gain_db.unwrap_or(s.output_gain_db).max(-144.0)
            } else {
                0.0
            };
            POINTS
        ];
        for b in &view {
            let response = (input.response)(b, input.sample_rate_hz);
            let values: Vec<_> = (0..POINTS)
                .map(|j| response.db(20.0 * 1000_f32.powf(j as f32 / (POINTS - 1) as f32)))
                .collect();
            if s.enabled && b.enabled {
                for (sum, value) in combined.iter_mut().zip(&values) {
                    *sum += value;
                }
            }
            let color = if selected == Some(b.id) && s.enabled && b.enabled {
                accent.gamma_multiply(0.6)
            } else {
                palette.secondary.gamma_multiply(0.3)
            };
            let points = values
                .iter()
                .enumerate()
                .map(|(j, v)| {
                    egui::pos2(
                        plot.left() + plot.width() * j as f32 / (POINTS - 1) as f32,
                        y(plot, *v),
                    )
                })
                .collect();
            p.add(egui::Shape::line(points, Stroke::new(1.0, color)));
        }
        p.add(egui::Shape::line(
            combined
                .iter()
                .enumerate()
                .map(|(j, v)| {
                    egui::pos2(
                        plot.left() + plot.width() * j as f32 / (POINTS - 1) as f32,
                        y(plot, *v),
                    )
                })
                .collect(),
            Stroke::new(1.5, palette.secondary),
        ));
        for (i, b) in view.iter().enumerate() {
            let base = drag
                .as_ref()
                .filter(|d| d.id == b.id)
                .map_or(&s.bands[i], |d| &d.preview);
            let center = point(plot, base);
            let color = if s.enabled && b.enabled && selected == Some(b.id) {
                accent
            } else {
                palette.secondary
            };
            painter.circle_filled(center, node_radius(i), palette.panel);
            painter.circle_stroke(
                center,
                node_radius(i) + if selected == Some(b.id) { 1.0 } else { 0.0 },
                Stroke::new(1.25, color),
            );
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                egui::FontId::proportional(9.0),
                color,
            );
        }
        if let Some(selected) = selected
            && let Some(base) = s.bands.iter().find(|b| b.id == selected)
        {
            let b = drag
                .as_ref()
                .filter(|d| d.id == selected)
                .map_or(base, |d| &d.preview);
            let index = s.bands.iter().position(|v| v.id == selected).unwrap() + 1;
            let gain = if b.mode.uses_gain() {
                format!(" · {:+.1} dB", b.gain_db)
            } else {
                String::new()
            };
            painter.text(
                chart.right_top() + Vec2::new(-2.0, 1.0),
                egui::Align2::RIGHT_TOP,
                format!(
                    "B{index} · {}{gain} · Q{:.2}",
                    crate::knobs::format_frequency(b.frequency_hz),
                    b.q
                ),
                font.clone(),
                palette.secondary,
            );
        }
        if let Some(d) = &drag
            && ui.input(|i| i.pointer.primary_down())
        {
            bounds.preview = Some(d.preview.clone());
        }
        if let Some(d) = &drag
            && (input.live_edits || !ui.input(|i| i.pointer.primary_down()))
        {
            if let Some(b) = s.bands.iter_mut().find(|b| b.id == d.id) {
                b.frequency_hz = d.preview.frequency_hz;
                b.gain_db = d.preview.gain_db;
                b.q = d.preview.q;
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                drag = None;
            }
        }
        // The normal item gap already separates the graph from its footer.
        rows::centered(ui, layout::COMPACT_BUTTON_SIZE, |ui| {
            rows::unit_label(ui, "Response")
        });
    });
    bounds.section = strip.response.rect;
    ui.data_mut(|data| {
        if let Some(d) = drag {
            data.insert_temp(id, d);
        } else {
            data.remove::<Drag>(id);
        }
        if let Some(s) = selected {
            data.insert_temp(selection, s);
        }
    });
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        s: &mut MultibandEqSettings,
        events: Vec<egui::Event>,
        focus: bool,
        visible: bool,
        command: bool,
    ) -> Bounds {
        let mut result = None;
        let mut events = events;
        events.insert(
            0,
            egui::Event::ModifiersChanged(egui::Modifiers {
                command,
                mac_cmd: command,
                ..Default::default()
            }),
        );
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))),
                events,
                focused: focus,
                ..Default::default()
            },
            |ui| {
                result = Some(show(
                    ui,
                    s,
                    layout::unit_control_height(),
                    visible,
                    Input {
                        live_edits: false,
                        spectrum: None,
                        sample_rate_hz: 48000,
                        effective: &[],
                        output_gain_db: None,
                        response: |_, r| FrequencyResponse::unity(r),
                    },
                ));
            },
        );
        out.textures_delta.clear();
        result.unwrap()
    }
    fn button(p: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn eq_graph_drafts_commit_by_id_on_release_and_cancel_without_mutating_settings() {
        for cancel in ["release", "escape", "focus", "hidden", "external"] {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            let mut s = MultibandEqSettings::default();
            let original = s.clone();
            let b = frame(&ctx, &mut s, vec![], true, true, false);
            assert_eq!(b.section.width(), WIDTH);
            assert!(b.section.contains_rect(b.plot));
            let start = b.nodes[0].center();
            let end = start + Vec2::new(25.0, -20.0);
            frame(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(start), button(start, true)],
                true,
                true,
                false,
            );
            frame(
                &ctx,
                &mut s,
                vec![egui::Event::PointerMoved(end)],
                true,
                true,
                false,
            );
            assert_eq!(s, original);
            if cancel == "external" {
                s.bands[0].q = 2.0;
            }
            let expected = s.clone();
            let events = if cancel == "escape" {
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
            frame(
                &ctx,
                &mut s,
                events,
                cancel != "focus",
                cancel != "hidden",
                false,
            );
            let b = frame(&ctx, &mut s, vec![button(end, false)], true, true, false);
            if cancel == "release" {
                assert_eq!(b.edits.len(), 2);
                assert!(s.bands[0].frequency_hz > original.bands[0].frequency_hz);
                assert!(s.bands[0].gain_db > 0.0);
                assert_eq!(s.bands[0].id, original.bands[0].id);
                assert!(s.valid());
            } else {
                assert_eq!(s, expected, "{cancel}");
                assert!(b.edits.is_empty());
            }
        }
    }
    #[test]
    fn eq_graph_command_drag_changes_q_in_all_four_directions_without_frequency_or_gain() {
        for delta in [
            Vec2::new(20.0, 0.0),
            Vec2::new(0.0, -20.0),
            Vec2::new(-20.0, 0.0),
            Vec2::new(0.0, 20.0),
        ] {
            for mode in [
                daw_unit_core::EqBandMode::Bell,
                daw_unit_core::EqBandMode::HighPass,
            ] {
                let ctx = egui::Context::default();
                theme::configure(&ctx);
                crate::fonts::configure(&ctx);
                let mut s = MultibandEqSettings::default();
                s.bands[0].mode = mode;
                let original = s.clone();
                let start = frame(&ctx, &mut s, vec![], true, true, true).nodes[0].center();
                frame(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                    true,
                    true,
                    true,
                );
                let end = start + delta;
                frame(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(end)],
                    true,
                    true,
                    true,
                );
                frame(&ctx, &mut s, vec![button(end, false)], true, true, true);
                assert_eq!(s.bands[0].frequency_hz, original.bands[0].frequency_hz);
                assert_eq!(s.bands[0].gain_db, original.bands[0].gain_db);
                assert_eq!(s.bands[0].q > original.bands[0].q, delta.x - delta.y > 0.0);
                assert!(s.valid());
            }
        }
    }
}
