//! Compressor transfer curve, detector marker and one atomic two-parameter draft.
use crate::{control_capture, layout, meters, rows, spectrum_graph, theme};
use daw_unit_core::{CompressorSettings, compressor_shaped_gain_reduction_db};
use egui::{Pos2, Rect, Response, Ui, Vec2};
// Include the lowest possible knee edge: -60 dB threshold - 24/2 dB knee.
const MIN: f32 = -72.0;
const MAX: f32 = 12.0;
#[derive(Clone)]
struct Drag {
    before: CompressorSettings,
    preview: [f32; 2],
    position: [f64; 2],
    last: Pos2,
    frame: u64,
    cancelled: bool,
}
#[derive(Clone, Copy)]
pub struct Input {
    pub live_edits: bool,
    /// Actual detector input amplitude with the shared level release.
    pub detector_level: f32,
}
pub struct Bounds {
    pub section: Rect,
    pub plot: Rect,
    pub handle: Rect,
    pub reduction_meter: Rect,
    pub preview: Option<[f32; 2]>,
    pub edits: Vec<(usize, Response)>,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            section: Rect::NOTHING,
            plot: Rect::NOTHING,
            handle: Rect::NOTHING,
            reduction_meter: Rect::NOTHING,
            preview: None,
            edits: Vec::new(),
        }
    }
}
fn point(plot: Rect, input: f32, output: f32) -> Pos2 {
    egui::pos2(
        plot.left() + plot.width() * (input - MIN) / (MAX - MIN),
        plot.bottom() - plot.height() * (output - MIN) / (MAX - MIN),
    )
}
fn output(input: f32, parameters: [f32; 2], knee: f32, curve: f32) -> f32 {
    input
        - compressor_shaped_gain_reduction_db(
            input,
            parameters[0],
            1.0 - parameters[1].recip(),
            knee,
            curve,
        )
}
fn parameters(s: &CompressorSettings) -> [f32; 2] {
    [s.threshold_db, s.ratio]
}
fn apply(s: &mut CompressorSettings, p: [f32; 2]) {
    s.threshold_db = p[0];
    s.ratio = p[1];
}
pub fn show(
    ui: &mut Ui,
    s: &mut CompressorSettings,
    dimensions: (bool, f32),
    input: Input,
    reduction: meters::GainReduction<'_>,
) -> Bounds {
    let id = ui.id().with("compressor-curve-drag");
    let selection = ui.id().with("compressor-curve-selection");
    let frame = ui.ctx().cumulative_frame_nr();
    let mut selected = ui.data(|d| d.get_temp::<bool>(selection)).unwrap_or(false);
    let mut drag = ui.data_mut(|d| d.get_temp::<Drag>(id));
    let external = drag.as_ref().is_some_and(|d| {
        let mut expected = d.before.clone();
        if input.live_edits {
            apply(&mut expected, d.preview);
        }
        // Ignore only our own in-flight values; all other controls invalidate a draft.
        *s != expected && (!d.cancelled || *s != d.before)
    });
    let cancel = dimensions.0
        || !ui.is_enabled()
        || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape))
        || drag.as_ref().is_some_and(|d| frame > d.frame + 1);
    if external {
        drag = None;
    } else if let Some(d) = drag.as_mut() {
        if d.cancelled && parameters(s) == parameters(&d.before) {
            drag = None;
        } else if cancel || d.cancelled {
            if input.live_edits {
                apply(s, parameters(&d.before));
                d.cancelled = true;
                d.frame = frame;
            } else {
                drag = None;
            }
        }
    }
    let mut bounds = Bounds::default();
    if !dimensions.0 {
        bounds.section = layout::unit_strip_with_width(
            ui,
            dimensions.1,
            spectrum_graph::WIDTH,
            egui::UiBuilder::new(),
            |ui| {
                let start = ui.cursor().top();
                let (chart, _) = ui.allocate_exact_size(
                    Vec2::new(
                        spectrum_graph::WIDTH - 2.0 * f32::from(layout::PANEL_MARGIN),
                        3.0 * crate::knobs::CHOICE_SIZE + 2.0 * f32::from(theme::TOOLBAR_PADDING),
                    ),
                    egui::Sense::hover(),
                );
                let mut plot = chart.shrink2(Vec2::new(24.0, 17.0));
                plot.max.x -= 24.0;
                bounds.plot = plot;
                let palette = theme::palette(ui.ctx());
                let p = ui.painter_at(chart);
                p.rect_filled(chart, 2.0, palette.background);
                bounds.reduction_meter =
                    Rect::from_x_y_ranges(chart.right() - 28.0..=chart.right(), chart.y_range());
                meters::gain_reduction_vertical(
                    ui,
                    bounds.reduction_meter,
                    reduction.for_unit(s.enabled),
                );
                let grid = egui::Stroke::new(1.0, palette.secondary.gamma_multiply(0.12));
                for db in [-72.0, -60.0, -48.0, -36.0, -24.0, -12.0, 0.0, 12.0] {
                    let pos = point(plot, db, db);
                    p.vline(pos.x, plot.y_range(), grid);
                    p.hline(plot.x_range(), pos.y, grid);
                    if [-72.0, -24.0, 0.0, 12.0].contains(&db) {
                        let text = format!("{db:+.0}");
                        p.text(
                            egui::pos2(pos.x, plot.bottom() + 4.0),
                            egui::Align2::CENTER_TOP,
                            &text,
                            egui::FontId::proportional(9.0),
                            palette.secondary,
                        );
                        p.text(
                            egui::pos2(plot.left() - 4.0, pos.y),
                            egui::Align2::RIGHT_CENTER,
                            text,
                            egui::FontId::proportional(9.0),
                            palette.secondary,
                        );
                    }
                }
                let view = drag
                    .as_ref()
                    .filter(|d| !d.cancelled)
                    .map_or(parameters(s), |d| d.preview);
                // Threshold is the input midpoint of the soft knee (the hard-knee corner).
                let node = point(plot, view[0], output(view[0], view, s.knee_db, s.curve));
                let response = ui
                    .interact(
                        Rect::from_center_size(node, Vec2::splat(17.0)),
                        id,
                        egui::Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if response.clicked() || response.is_pointer_button_down_on() {
                    selected = true;
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
                    let before = s.clone();
                    let values = parameters(s);
                    drag = Some(Drag {
                        before,
                        preview: values,
                        position: [f64::from(values[0]), f64::from(values[1]).ln()],
                        last: ui.input(|i| i.pointer.press_origin()).unwrap_or(node),
                        frame,
                        cancelled: false,
                    });
                }
                if let Some(d) = drag.as_mut().filter(|d| !d.cancelled) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        let delta = (pos - d.last)
                            * ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
                        d.last = pos;
                        let modifiers = ui.input(|i| i.modifiers);
                        // Consume both pointer axes even when locked so changing
                        // modifiers cannot apply motion accumulated in another mode.
                        if !modifiers.command {
                            d.position[0] = (d.position[0]
                                + f64::from(delta.x * (MAX - MIN) / plot.width()))
                            .clamp(-60.0, 0.0);
                        }
                        // Up opens the curve (less compression); down raises Ratio.
                        if !modifiers.alt {
                            d.position[1] = (d.position[1]
                                + f64::from(delta.y / plot.height()) * 20.0_f64.ln())
                            .clamp(0.0, 20.0_f64.ln());
                        }
                        d.preview = [d.position[0] as f32, d.position[1].exp() as f32];
                    }
                    d.frame = frame;
                    bounds.preview = Some(d.preview);
                    if input.live_edits || !ui.input(|i| i.pointer.primary_down()) {
                        let before = parameters(s);
                        apply(s, d.preview);
                        for (index, value) in before.into_iter().enumerate() {
                            let mut edit = response.clone();
                            if value != d.preview[index] {
                                edit.mark_changed();
                            }
                            bounds.edits.push((index, edit));
                        }
                    }
                }
                let view = drag
                    .as_ref()
                    .filter(|d| !d.cancelled)
                    .map_or(parameters(s), |d| d.preview);
                let painter = ui.painter_at(plot);
                painter.line_segment(
                    [point(plot, MIN, MIN), point(plot, MAX, MAX)],
                    egui::Stroke::new(1.0, palette.secondary.gamma_multiply(0.15)),
                );
                painter.add(egui::Shape::line(
                    (0..161)
                        .map(|i| {
                            let x = MIN + (MAX - MIN) * i as f32 / 160.0;
                            point(plot, x, output(x, view, s.knee_db, s.curve))
                        })
                        .collect(),
                    egui::Stroke::new(
                        1.5,
                        if selected && s.enabled {
                            theme::accent(ui.ctx()).gamma_multiply(0.65)
                        } else {
                            palette.secondary
                        },
                    ),
                ));
                if s.enabled && input.detector_level.is_finite() && input.detector_level > 1e-3 {
                    let db = (20.0 * input.detector_level.log10()).clamp(MIN, MAX);
                    painter.circle_filled(
                        point(plot, db, output(db, view, s.knee_db, s.curve)),
                        2.5,
                        theme::accent(ui.ctx()),
                    );
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(16));
                }
                let node = point(plot, view[0], output(view[0], view, s.knee_db, s.curve));
                bounds.handle = Rect::from_center_size(node, Vec2::splat(17.0));
                spectrum_graph::band_handle(ui, &p, node, 5.5, selected, s.enabled);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::DragValue,
                        response.enabled(),
                        format!(
                            "Compressor curve: Threshold {:.1} dBFS, Ratio {:.1}:1",
                            view[0], view[1]
                        ),
                    )
                });
                control_capture::record(
                    ui,
                    &response.clone().on_hover_text(format!(
                        concat!(
                            "Drag left/right: Threshold. Up/down: lower/higher Ratio.\n",
                            "{}: Ratio only. {}: Threshold only. Both: hold values.\n",
                            "Switch modifiers while dragging. Shift: fine. Escape cancels."
                        ),
                        if cfg!(target_os = "macos") {
                            "Command"
                        } else {
                            "Ctrl"
                        },
                        if cfg!(target_os = "macos") {
                            "Option"
                        } else {
                            "Alt"
                        }
                    )),
                );
                if drag.as_ref().is_some_and(|d| !d.cancelled)
                    && !ui.input(|i| i.pointer.primary_down())
                {
                    drag = None;
                    bounds.preview = None;
                }
                ui.add_space(
                    (dimensions.1 - layout::COMPACT_BUTTON_SIZE - (ui.cursor().top() - start))
                        .max(0.0),
                );
                rows::centered(ui, layout::COMPACT_BUTTON_SIZE, |ui| {
                    rows::unit_label(ui, "Curve")
                        .on_hover_text("Input → output in dBFS, before Dry/Wet and Level.")
                });
            },
        )
        .response
        .rect;
    }
    ui.data_mut(|data| {
        if let Some(d) = drag {
            data.insert_temp(id, d);
        } else {
            data.remove::<Drag>(id);
        }
        data.insert_temp(selection, selected);
    });
    bounds
}
#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        ctx
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }
    }
    fn draw(
        ctx: &egui::Context,
        s: &mut CompressorSettings,
        events: Vec<egui::Event>,
        live: bool,
        focused: bool,
        enabled: bool,
        compact: bool,
    ) -> Bounds {
        let mut b = Bounds::default();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(550.0, 350.0))),
                events,
                focused,
                ..Default::default()
            },
            |ui| {
                b = ui
                    .add_enabled_ui(enabled, |ui| {
                        show(
                            ui,
                            s,
                            (compact, layout::unit_control_height()),
                            Input {
                                live_edits: live,
                                detector_level: 0.5,
                            },
                            0.0.into(),
                        )
                    })
                    .inner;
            },
        );
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("compressor-curve-shapes-test"),
                out.shapes.clone(),
            )
        });
        out.textures_delta.clear();
        b
    }
    #[test]
    fn curve_drag_previews_both_controls_and_commits_or_cancels_as_one_draft() {
        for live in [false, true] {
            for cancel in 0..6 {
                let ctx = context();
                let mut s = CompressorSettings::default();
                let before = s.clone();
                let b = draw(&ctx, &mut s, vec![], live, true, true, false);
                let start = b.handle.center();
                let end = start + Vec2::new(-15.0, 20.0);
                draw(
                    &ctx,
                    &mut s,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
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
                let preview = b.preview.unwrap();
                assert!(preview[0] < before.threshold_db);
                assert!(preview[1] > before.ratio);
                assert_eq!(
                    parameters(&s),
                    if live { preview } else { parameters(&before) }
                );
                if cancel == 5 {
                    s.knee_db = 12.0;
                }
                if cancel != 0 {
                    let events = if cancel == 1 {
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
                    let b = draw(
                        &ctx,
                        &mut s,
                        events,
                        live,
                        cancel != 2,
                        cancel != 3,
                        cancel == 4,
                    );
                    assert!(b.preview.is_none());
                }
                draw(
                    &ctx,
                    &mut s,
                    vec![button(end, false)],
                    live,
                    true,
                    true,
                    false,
                );
                let mut expected = before;
                if cancel == 0 || (live && cancel == 5) {
                    apply(&mut expected, preview);
                }
                if cancel == 5 {
                    expected.knee_db = 12.0;
                }
                assert_eq!(s, expected);
            }
        }
    }
    #[test]
    fn curve_drag_respects_bounds_and_shift_without_modifier_jumps() {
        let ctx = context();
        let mut s = CompressorSettings::default();
        let b = draw(&ctx, &mut s, vec![], false, true, true, false);
        let start = b.handle.center();
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap(), parameters(&s));
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(egui::Modifiers {
                shift: true,
                ..Default::default()
            })],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap(), parameters(&s));
        let end = start + Vec2::new(15.0, -20.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(end)],
            false,
            true,
            true,
            false,
        );
        let fine = b.preview.unwrap();
        assert!(fine[0] > s.threshold_db && fine[0] < s.threshold_db + 1.0);
        assert!(fine[1] < s.ratio && fine[1] > 3.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::ModifiersChanged(Default::default())],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap(), fine);
        let far = start + Vec2::new(2000.0, -2000.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(far)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap(), [0.0, 1.0]);
        let far = start + Vec2::new(-2000.0, 2000.0);
        let b = draw(
            &ctx,
            &mut s,
            vec![egui::Event::PointerMoved(far)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(b.preview.unwrap(), [-60.0, 20.0]);
        draw(
            &ctx,
            &mut s,
            vec![button(far, false)],
            false,
            true,
            true,
            false,
        );
        assert_eq!(parameters(&s), [-60.0, 20.0]);
    }
    #[test]
    fn curve_layout_and_live_point_follow_the_shared_theme_and_bypass_rules() {
        for (mode, palette) in [
            (egui::Theme::Dark, theme::LIGHT),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            for scale in [1.0, 1.25, 2.0] {
                let ctx = context();
                crate::fonts::configure(&ctx);
                theme::configure(&ctx);
                theme::apply(&ctx, mode, palette, theme::ACCENT);
                ctx.set_pixels_per_point(scale);
                for (compact, enabled) in [(false, true), (false, false), (true, true)] {
                    let mut s = CompressorSettings {
                        enabled,
                        ..Default::default()
                    };
                    let before = s.clone();
                    let mut bounds = None;
                    let mut out = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(1000.0, 400.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            bounds = Some(crate::compressor::show(
                                ui,
                                &mut s,
                                compact,
                                layout::unit_control_height(),
                                crate::compressor::Controls {
                                    markers: &std::array::from_fn(|_| None),
                                    reduction: 0.0.into(),
                                    level: 0.0.into(),
                                    graph: Input {
                                        live_edits: false,
                                        detector_level: 0.5,
                                    },
                                },
                            ));
                        },
                    );
                    out.textures_delta.clear();
                    let b = bounds.unwrap();
                    assert_eq!(s, before);
                    if compact {
                        assert_eq!(b.graph.section, Rect::NOTHING);
                        continue;
                    }
                    assert!(b.graph.section.right() <= b.sections[0].left());
                    assert!((b.graph.section.width() - spectrum_graph::WIDTH).abs() < 0.01);
                    assert!(b.graph.section.contains_rect(b.graph.reduction_meter));
                    assert!(b.graph.plot.right() < b.graph.reduction_meter.left());
                    assert_ne!(b.controls[2][2], b.graph.reduction_meter);
                    assert!(!b.sections[2].contains_rect(b.graph.reduction_meter));
                    assert_eq!(
                        out.shapes
                            .iter()
                            .filter(
                                |p| matches!(&p.shape,egui::Shape::Text(t) if t.galley.text()=="GR")
                            )
                            .count(),
                        1
                    );
                    let center = |name: &str| {
                        out.shapes
                            .iter()
                            .find_map(|p| match &p.shape {
                                egui::Shape::Text(t) if t.galley.text() == name => {
                                    Some(t.pos.y + t.galley.size().y * 0.5)
                                }
                                _ => None,
                            })
                            .unwrap()
                    };
                    for label in ["Comp", "Response", "Out"] {
                        assert!(
                            (center("Curve") - center(label)).abs() < 0.01,
                            "footer mismatch: {label}"
                        );
                    }
                    let dot = out.shapes.iter().find_map(|p| match &p.shape {
                        egui::Shape::Circle(c)
                            if c.radius == 2.5 && b.graph.plot.contains(c.center) =>
                        {
                            Some(c.center)
                        }
                        _ => None,
                    });
                    assert_eq!(dot.is_some(), enabled);
                    if let Some(dot) = dot {
                        let db = 20.0 * 0.5_f32.log10();
                        assert!(
                            dot.distance(point(
                                b.graph.plot,
                                db,
                                output(db, parameters(&s), s.knee_db, s.curve)
                            )) < 0.01
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn handle_stays_at_the_knee_midpoint_for_every_threshold_ratio_and_knee_bound() {
        for threshold_db in [-60.0, -18.0, 0.0] {
            for ratio in [1.0, 4.0, 20.0] {
                for knee_db in [0.0, 6.0, 24.0] {
                    let ctx = context();
                    let mut settings = CompressorSettings {
                        threshold_db,
                        ratio,
                        knee_db,
                        ..Default::default()
                    };
                    let b = draw(&ctx, &mut settings, vec![], false, true, true, false);
                    let reduction = (1.0 - 1.0 / ratio) * knee_db / 8.0;
                    let expected = point(b.plot, threshold_db, threshold_db - reduction);
                    assert!(b.handle.center().distance(expected) < 0.01);
                    assert!(b.plot.contains(expected));
                }
            }
        }
    }
    #[test]
    fn command_and_option_lock_axes_and_switch_during_the_same_drag_without_jumps() {
        for live in [false, true] {
            let ctx = context();
            let mut settings = CompressorSettings::default();
            let b = draw(&ctx, &mut settings, vec![], live, true, true, false);
            let mut pos = b.handle.center();
            draw(
                &ctx,
                &mut settings,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
                live,
                true,
                true,
                false,
            );
            let original = parameters(&settings);
            let mut previous = original;
            let command = egui::Modifiers {
                command: true,
                mac_cmd: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Default::default()
            };
            let option = egui::Modifiers {
                alt: true,
                ..Default::default()
            };
            let both = egui::Modifiers {
                alt: true,
                ..command
            };
            for (step, modifiers) in [command, option, both, egui::Modifiers::default()]
                .into_iter()
                .enumerate()
            {
                let b = draw(
                    &ctx,
                    &mut settings,
                    vec![egui::Event::ModifiersChanged(modifiers)],
                    live,
                    true,
                    true,
                    false,
                );
                assert_eq!(
                    b.preview.unwrap(),
                    previous,
                    "modifier change must not move the handle"
                );
                pos += Vec2::new(-5.0, 10.0);
                let b = draw(
                    &ctx,
                    &mut settings,
                    vec![egui::Event::PointerMoved(pos)],
                    live,
                    true,
                    true,
                    false,
                );
                let current = b.preview.unwrap();
                if step == 0 || step == 2 {
                    assert_eq!(current[0], previous[0]);
                } else {
                    assert!(current[0] < previous[0]);
                }
                if step == 1 || step == 2 {
                    assert_eq!(current[1], previous[1]);
                } else {
                    assert!(current[1] > previous[1]);
                }
                if live {
                    for (index, response) in b.edits {
                        assert_eq!(response.changed(), current[index] != previous[index]);
                    }
                } else {
                    assert_eq!(parameters(&settings), original);
                }
                previous = current;
            }
            draw(
                &ctx,
                &mut settings,
                vec![button(pos, false)],
                live,
                true,
                true,
                false,
            );
            assert_eq!(parameters(&settings), previous);
        }
    }
    #[test]
    fn curve_knob_bends_the_visible_response_without_moving_the_knee_handle() {
        let ctx = context();
        let mut settings = CompressorSettings::default();
        let mut dots = Vec::new();
        let mut handle = None;
        for curve in [-1.0, 0.0, 1.0] {
            settings.curve = curve;
            let before = settings.clone();
            let bounds = draw(&ctx, &mut settings, vec![], false, true, true, false);
            assert_eq!(settings, before);
            if let Some(handle) = handle {
                assert_eq!(bounds.handle, handle);
            } else {
                handle = Some(bounds.handle);
            }
            let shapes = ctx
                .data(|d| {
                    d.get_temp::<Vec<egui::epaint::ClippedShape>>(egui::Id::new(
                        "compressor-curve-shapes-test",
                    ))
                })
                .unwrap();
            dots.push(
                shapes
                    .iter()
                    .find_map(|p| match &p.shape {
                        egui::Shape::Circle(c) if c.radius == 2.5 => Some(c.center),
                        _ => None,
                    })
                    .unwrap(),
            );
        }
        assert_eq!(dots[0].x, dots[1].x);
        assert_eq!(dots[1].x, dots[2].x);
        assert!(dots[0].y < dots[1].y && dots[1].y < dots[2].y);
    }
}
