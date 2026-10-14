use crate::theme;
use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

const MIN_DBFS: f32 = -60.0;
const CHANNEL_WIDTH: f32 = 8.0;
const CHANNEL_SPACING: f32 = 4.0;
const SCALE_INSET: f32 = 5.0;
pub const SCALE_WIDTH: f32 = 24.0;
pub const STEREO_WIDTH: f32 = SCALE_WIDTH + 2.0 * CHANNEL_WIDTH + CHANNEL_SPACING;
const MARKS: [i32; 15] = [
    0, -3, -6, -9, -12, -15, -18, -21, -24, -30, -35, -40, -45, -50, -60,
];

/// Signed band gain monitor. Center is unity; left is reduction, right is boost.
pub fn gain_change(ui: &mut Ui, rail: Rect, name: &str, gain_db: f32) {
    let db = if gain_db.is_finite() { gain_db } else { 0.0 };
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter();
    painter.rect_filled(rail, 1.0, palette.input);
    let x = rail.center().x + rail.width() * 0.5 * (db / 24.0).clamp(-1.0, 1.0);
    let fill = Rect::from_min_max(
        Pos2::new(x.min(rail.center().x), rail.top()),
        Pos2::new(x.max(rail.center().x), rail.bottom()),
    );
    painter.rect_filled(
        fill,
        1.0,
        if db > 0.0 {
            palette.meter
        } else {
            theme::accent(ui.ctx())
        },
    );
    painter.vline(
        rail.center().x,
        rail.y_range(),
        Stroke::new(1.0, palette.secondary),
    );
    let response = ui.interact(rail, ui.id().with(("gain-change", name)), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            ui.is_enabled(),
            format!("{name} gain change"),
        );
        info.value = Some(f64::from(db));
        info
    });
    response.on_hover_text(format!("{name} compression gain: {db:+.1} dB. Left: reduction; right: boost. Display ±24 dB; before Dry/Wet."));
}

/// Read-only compression monitor. The rail covers 0–24 dB; the number retains larger readings.
pub fn gain_reduction(ui: &mut Ui, reduction_db: f32) -> Response {
    let db = if reduction_db.is_finite() {
        reduction_db.max(0.0)
    } else {
        0.0
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(crate::knobs::CHOICE_SIZE), Sense::hover());
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter();
    let rail = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 8.0, 8.0));
    painter.rect_filled(rail, 1.0, palette.input);
    let fill = Rect::from_min_size(
        rail.min,
        Vec2::new(rail.width() * (db / 24.0).min(1.0), rail.height()),
    );
    if fill.width() > 0.0 {
        painter.rect_filled(fill, 1.0, theme::accent(ui.ctx()));
    }
    painter.text(
        rect.center_top(),
        egui::Align2::CENTER_TOP,
        if db > 0.0 {
            format!("−{db:.1} dB")
        } else {
            "0.0 dB".into()
        },
        crate::fonts::numeric(9.0),
        palette.text,
    );
    for (position, align, label) in [
        (rect.left_bottom(), egui::Align2::LEFT_BOTTOM, "0"),
        (rect.center_bottom(), egui::Align2::CENTER_BOTTOM, "GR"),
        (rect.right_bottom(), egui::Align2::RIGHT_BOTTOM, "24"),
    ] {
        painter.text(
            position,
            align,
            label,
            crate::fonts::numeric(9.0),
            palette.secondary,
        );
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            ui.is_enabled(),
            "Gain reduction",
        );
        info.value = Some(f64::from(db));
        info
    });
    response.on_hover_text(format!(
        "Gain reduction: {db:.1} dB before Dry/Wet. Read-only; 0–24 dB display scale."
    ))
}

fn position_for_dbfs(db: f32) -> f32 {
    ((db - MIN_DBFS) / -MIN_DBFS).clamp(0.0, 1.0)
}

fn position_for_peak(peak: f32) -> f32 {
    if peak > 0.0 {
        position_for_dbfs(20.0 * peak.log10())
    } else {
        0.0
    }
}

fn level_rail(rect: Rect) -> Rect {
    rect.shrink2(Vec2::new(0.0, SCALE_INSET.min(rect.height() / 4.0)))
}

fn peak_text(peak: f32) -> String {
    if peak > 0.0 {
        let db = 20.0 * peak.log10();
        if db > 0.0 {
            format!("+{db:.1}")
        } else {
            format!("{db:.1}")
        }
    } else {
        "−∞".into()
    }
}

/// Read the held stereo peak; clicking resets monitoring without changing gain.
pub fn peak_level(ui: &mut Ui, meter: &daw_output::StereoMeter) -> Response {
    let mut text = peak_text(meter.held_peak());
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(crate::NUMERIC_WIDTH, crate::toolbars::CONTROL_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(crate::toolbars::CONTROL_HEIGHT);
                crate::numeric_field(
                    ui,
                    &mut text,
                    "Held peak level (dBFS). Click to reset peak and stereo clipping.",
                    false,
                )
            },
        )
        .inner
        .interact(Sense::click());
    reset_on_click(ui, Some(meter), [response.clicked(), false]);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Held peak level (dBFS): {text}. Reset peak and stereo clipping"),
        )
    });
    response
}

/// Shared reset for channel bars and the held-peak readout in every view.
pub fn reset_on_click(ui: &Ui, meter: Option<&daw_output::StereoMeter>, clicked: [bool; 2]) {
    if ui.is_enabled()
        && ui.input(|input| input.focused)
        && clicked.into_iter().any(|clicked| clicked)
        && let Some(meter) = meter
    {
        meter.reset();
        ui.ctx().request_repaint();
    }
}

/// One shared dBFS scale and clipping warning for both channels.
pub fn stereo(ui: &mut Ui, height: f32, levels: [(f32, bool); 2]) -> [Response; 2] {
    stereo_with_scale(ui, height, levels, true)
}

fn stereo_with_scale(
    ui: &mut Ui,
    height: f32,
    levels: [(f32, bool); 2],
    show_scale: bool,
) -> [Response; 2] {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let scale = show_scale.then(|| {
            ui.allocate_exact_size(Vec2::new(SCALE_WIDTH, height), Sense::hover())
                .0
        });
        let left = channel(ui, "L", height, levels[0].0, levels[0].1);
        ui.add_space(CHANNEL_SPACING);
        let right = channel(ui, "R", height, levels[1].0, levels[1].1);
        if let Some(scale) = scale {
            paint_scale(ui, scale, level_rail(left.rect));
        }
        [left, right]
    })
    .inner
}

fn paint_scale(ui: &Ui, scale: Rect, rail: Rect) {
    let palette = theme::palette(ui.ctx());
    let right = scale.right() - 2.0;
    let y_at = |db: f32| egui::lerp(rail.bottom()..=rail.top(), position_for_dbfs(db));
    let font = egui::FontId::proportional(8.0);
    let bottom_label = ui
        .painter()
        .layout_no_wrap("-60".into(), font.clone(), palette.text);
    let bottom_label_top = rail.bottom() - bottom_label.size().y / 2.0;
    let mut previous_bottom = scale.top() - 0.25;
    for db in MARKS {
        let y = y_at(db as f32);
        let unity = db == 0;
        ui.painter().line_segment(
            [
                Pos2::new(right - if unity { 6.0 } else { 4.0 }, y),
                Pos2::new(right, y),
            ],
            Stroke::new(
                if unity { 1.5 } else { 1.0 },
                if unity {
                    palette.text
                } else {
                    palette.secondary
                },
            ),
        );
        let label = ui
            .painter()
            .layout_no_wrap(db.to_string(), font.clone(), palette.text);
        // Keep endpoint labels inside the panel and omit interior labels that collide.
        let top = (y - label.size().y / 2.0).clamp(
            scale.top(),
            (scale.bottom() - label.size().y).max(scale.top()),
        );
        if top >= previous_bottom + 0.25
            && (db == -60 || top + label.size().y + 0.25 <= bottom_label_top)
        {
            let pos = Pos2::new(right - 8.0 - label.size().x, top);
            previous_bottom = top + label.size().y;
            ui.painter().galley(pos, label, palette.text);
        }
    }
    for (low, high, step) in [(-60, -30, 5), (-30, 0, 1)] {
        for db in (low..high).step_by(step as usize) {
            if !MARKS.contains(&db) && (y_at(db as f32) - y_at((db + step) as f32)).abs() >= 3.0 {
                let y = y_at(db as f32);
                ui.painter().line_segment(
                    [Pos2::new(right - 2.0, y), Pos2::new(right, y)],
                    Stroke::new(1.0, palette.border),
                );
            }
        }
    }
}

/// Timeline controls share the meter bars without Mixer scale labels or ticks.
pub fn stereo_panel(
    ui: &mut Ui,
    height: f32,
    levels: [(f32, bool); 2],
    content: impl FnOnce(&mut Ui),
) -> [bool; 2] {
    ui.horizontal_top(|ui| {
        let content_width =
            (ui.available_width() - (STEREO_WIDTH - SCALE_WIDTH) - ui.spacing().item_spacing.x)
                .max(1.0);
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, height),
            egui::Layout::top_down(egui::Align::Min),
            content,
        );
        stereo_with_scale(ui, height, levels, false).map(|response| response.clicked())
    })
    .inner
}

/// Shared channel monitor for tracks and the master output.
pub fn channel(ui: &mut Ui, label: &str, height: f32, peak: f32, clipped: bool) -> Response {
    let level = if peak > 0.0 {
        format!("{:.1} dBFS", 20.0 * peak.log10())
    } else {
        "Silent".into()
    };
    let progress = position_for_peak(peak);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(CHANNEL_WIDTH, height), Sense::click());
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, ui.is_enabled(), label);
        info.value = Some(f64::from(progress) * 100.0);
        info
    });
    let painter = ui.painter();
    let rail = level_rail(rect);
    painter.rect_filled(rail, 1.0, theme::palette(ui.ctx()).input);
    if progress > 0.0 {
        let fill = Rect::from_min_max(
            Pos2::new(rail.left(), rail.bottom() - rail.height() * progress),
            rail.max,
        );
        painter.rect_filled(
            fill,
            1.0,
            if clipped {
                theme::palette(ui.ctx()).error
            } else {
                theme::palette(ui.ctx()).meter
            },
        );
    }
    if clipped {
        let warning = Rect::from_min_size(rail.min, Vec2::new(rail.width(), 3.0));
        painter.rect_filled(warning, 1.0, theme::palette(ui.ctx()).error);
    }
    response.on_hover_text(if clipped {
        format!("{label}: {level}. Clipping detected. Click to reset peak and stereo clipping.")
    } else {
        format!("{label}: {level}")
    })
}

/// Thin limiter monitor fits above the standard knob rows without changing unit height.
pub fn gain_reduction_rail(ui: &mut Ui, rail: Rect, reduction_db: f32) {
    let db = if reduction_db.is_finite() {
        reduction_db.max(0.0)
    } else {
        0.0
    };
    ui.painter()
        .rect_filled(rail, 1.0, theme::palette(ui.ctx()).input);
    ui.painter().rect_filled(
        Rect::from_min_size(
            rail.min,
            Vec2::new(rail.width() * (db / 24.0).min(1.0), rail.height()),
        ),
        1.0,
        theme::accent(ui.ctx()),
    );
    let response = ui.interact(rail, ui.id().with("limiter-reduction"), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            ui.is_enabled(),
            "Limiter gain reduction",
        );
        info.value = Some(f64::from(db));
        info
    });
    response.on_hover_text(format!(
        "Gain reduction: {db:.1} dB (display range 0–24 dB)"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_channel_click_clears_stereo_warning_in_scaled_and_compact_meters() {
        for show_scale in [true, false] {
            for channel in 0..2 {
                for (enabled, focused) in [(true, true), (false, true), (true, false)] {
                    let ctx = egui::Context::default();
                    theme::configure(&ctx);
                    let meter = daw_output::StereoMeter::default();
                    meter.publish_peaks(if channel == 0 {
                        [1.5, 0.25]
                    } else {
                        [0.25, 1.5]
                    });
                    meter.publish_peaks([0.25, 0.5]);
                    let mut rects = [Rect::NOTHING; 2];
                    for phase in 0..4 {
                        let events = if phase == 1 || phase == 2 {
                            let point = rects[channel].center();
                            vec![
                                egui::Event::PointerMoved(point),
                                egui::Event::PointerButton {
                                    pos: point,
                                    button: egui::PointerButton::Primary,
                                    pressed: phase == 1,
                                    modifiers: Default::default(),
                                },
                            ]
                        } else {
                            vec![]
                        };
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                events,
                                focused,
                                ..Default::default()
                            },
                            |ui| {
                                ui.add_enabled_ui(enabled, |ui| {
                                    let responses =
                                        stereo_with_scale(ui, 54.0, meter.levels(), show_scale);
                                    rects = responses.each_ref().map(|response| response.rect);
                                    reset_on_click(
                                        ui,
                                        Some(&meter),
                                        responses.map(|response| response.clicked()),
                                    );
                                });
                            },
                        );
                        output.textures_delta.clear();
                        let expected_clipped = phase < 2 || !enabled || !focused;
                        assert_eq!(
                            meter.levels(),
                            [(0.25, expected_clipped), (0.5, expected_clipped)]
                        );
                        assert_eq!(meter.held_peak(), if expected_clipped { 1.5 } else { 0.0 });
                        // Both red caps appear before clearing, and disappear on the next frame.
                        let expected_caps = if phase < 3 || !enabled || !focused {
                            2
                        } else {
                            0
                        };
                        let caps = output.shapes.iter().filter(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(rect) if rect.rect.height() == 3.0 && rects.iter().any(|channel| level_rail(*channel).min == rect.rect.min)
                        )).count();
                        assert_eq!(
                            caps, expected_caps,
                            "scale={show_scale}, channel={channel}, enabled={enabled}, focused={focused}, phase={phase}"
                        );
                    }
                    meter.publish_peaks([0.25, 1.5]);
                    assert!(meter.levels().iter().all(|(_, clipped)| *clipped));
                }
            }
        }
    }

    #[test]
    fn held_peak_readout_resets_on_release_and_does_not_accept_text_input() {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        for (peak, expected) in [
            (0.0_f32, "−∞"),
            (0.25, "-12.0"),
            (0.5, "-6.0"),
            (2.0, "+6.0"),
            (1.0, "0.0"),
            (daw_core::linear_gain(-90.0), "-90.0"),
        ] {
            let meter = daw_output::StereoMeter::default();
            meter.publish_peaks([peak; 2]);
            let mut rect = Rect::NOTHING;
            for phase in 0..4 {
                let mut events = vec![];
                if phase == 1 || phase == 2 {
                    let point = rect.center();
                    events.push(egui::Event::PointerMoved(point));
                    events.push(egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: Default::default(),
                    });
                    if phase == 2 {
                        events.push(egui::Event::Text("25".into()));
                    }
                }
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        rect = peak_level(ui, &meter).rect;
                    },
                );
                output.textures_delta.clear();
                let expected = if phase < 3 { expected } else { "−∞" };
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == expected
                )));
                assert!(!ctx.egui_wants_keyboard_input());
                assert_eq!(meter.is_clipped(), phase < 2 && peak > 1.0);
                assert_eq!(meter.held_peak(), if phase < 2 { peak } else { 0.0 });
            }
        }
        for (enabled, focused) in [(false, true), (true, false)] {
            let meter = daw_output::StereoMeter::default();
            meter.publish_peaks([2.0; 2]);
            let mut rect = Rect::NOTHING;
            for phase in 0..3 {
                let events = if phase == 0 {
                    vec![]
                } else {
                    vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed: phase == 1,
                            modifiers: Default::default(),
                        },
                    ]
                };
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events,
                        focused,
                        ..Default::default()
                    },
                    |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            rect = peak_level(ui, &meter).rect;
                        });
                    },
                );
                output.textures_delta.clear();
                assert_eq!(meter.held_peak(), 2.0);
                assert!(meter.is_clipped());
            }
        }
    }

    #[test]
    fn meter_positions_follow_dbfs_and_preserve_headroom_in_readings() {
        for (db, position) in [(0.0, 1.0), (-6.0, 0.9), (-30.0, 0.5), (-60.0, 0.0)] {
            assert!((position_for_peak(daw_core::linear_gain(db)) - position).abs() < 0.00001);
        }
        for peak in [0.0, -1.0, f32::NAN, daw_core::linear_gain(-90.0)] {
            assert_eq!(position_for_peak(peak), 0.0);
        }
        assert_eq!(position_for_peak(2.0), 1.0);
        assert_eq!(position_for_peak(f32::INFINITY), 1.0);
    }

    fn frame(
        ctx: &egui::Context,
        height: f32,
        levels: [(f32, bool); 2],
        events: Vec<egui::Event>,
        enabled: bool,
    ) -> ([Rect; 2], [bool; 2], Vec<egui::epaint::ClippedShape>) {
        let mut rects = [Rect::NOTHING; 2];
        let mut clicked = [false; 2];
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 300.0))),
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let responses = stereo(ui, height, levels);
                    rects = responses.each_ref().map(|response| response.rect);
                    clicked = responses.map(|response| response.clicked());
                });
            },
        );
        output.textures_delta.clear();
        (rects, clicked, output.shapes)
    }

    #[test]
    fn stereo_scale_labels_fit_and_match_channel_fill_in_both_themes() {
        for color_theme in [
            crate::settings::ColorTheme::Dark,
            crate::settings::ColorTheme::Light,
        ] {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            theme::apply(
                &ctx,
                crate::settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            let palette = theme::palette(&ctx);
            for height in [24.0, 54.0, 160.0, 220.0] {
                let levels = [
                    (daw_core::linear_gain(-6.0), false),
                    (daw_core::linear_gain(-30.0), false),
                ];
                let (rects, _, shapes) = frame(&ctx, height, levels, vec![], true);
                assert_eq!(rects[1].left() - rects[0].right(), CHANNEL_SPACING);
                for (rect, position) in rects.into_iter().zip([0.9, 0.5]) {
                    let rail = level_rail(rect);
                    let fill = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.fill == palette.meter
                                    && (rect.rect.left() - rail.left()).abs() < 0.1 =>
                            {
                                Some(rect.rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert!(rail.contains_rect(fill));
                    assert!((fill.height() - rail.height() * position).abs() < 0.0001);
                }
                let labels: Vec<_> = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some((
                            text.galley.text(),
                            text.galley.rect.translate(text.pos.to_vec2()),
                        )),
                        _ => None,
                    })
                    .collect();
                assert_eq!(labels.first().unwrap().0, "0");
                assert_eq!(labels.last().unwrap().0, "-60");
                for (_, bounds) in &labels {
                    assert!(bounds.left() >= rects[0].left() - SCALE_WIDTH);
                    assert!(bounds.right() < rects[0].left());
                    assert!(bounds.top() >= rects[0].top());
                    assert!(bounds.bottom() <= rects[0].bottom());
                }
                for pair in labels.windows(2) {
                    assert!(pair[0].1.bottom() < pair[1].1.top());
                }
                if height == 220.0 {
                    assert_eq!(labels.len(), MARKS.len());
                    let subdivisions = shapes.iter().filter(|shape| matches!(&shape.shape,
                        egui::Shape::LineSegment { points, .. } if (points[1].x - points[0].x - 2.0).abs() < 0.1
                    )).count();
                    assert!(subdivisions >= 15);
                }
            }
            let levels = [(2.0, true), (2.0, true)];
            let (rects, _, shapes) = frame(&ctx, 220.0, levels, vec![], true);
            for rail in rects {
                let rail = level_rail(rail);
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.fill == palette.error && rect.rect.min == rail.min && rect.rect.height() == 3.0
                )));
            }
            for enabled in [true, false] {
                let point = rects[0].center();
                let button = |pressed| egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                };
                frame(
                    &ctx,
                    220.0,
                    levels,
                    vec![egui::Event::PointerMoved(point), button(true)],
                    enabled,
                );
                let (_, clicked, _) = frame(&ctx, 220.0, levels, vec![button(false)], enabled);
                assert_eq!(clicked, [enabled, false]);
            }
        }
    }
}
