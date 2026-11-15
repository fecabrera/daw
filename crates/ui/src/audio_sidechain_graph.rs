//! Sidechain transfer display, using the compressor curve's visual conventions.
use crate::{knobs, theme};
use daw_unit_core::AudioSidechain;
use egui::{Align2, FontId, Rect, Stroke, Ui, Vec2};

// Match the responsive display follow used by the live EQ spectrum.
const FOLLOW_SECONDS: f64 = 0.035;
#[derive(Clone)]
struct Marker {
    /// Normalized input x and measured output offset from the displayed mapping.
    position: Vec2,
    settings: AudioSidechain,
    time: f64,
    frame: u64,
}
impl Marker {
    fn follow(&mut self, target: Vec2, settings: AudioSidechain, time: f64, frame: u64) {
        if self.settings != settings || frame > self.frame + 1 || time < self.time {
            self.position = target;
        } else {
            let amount = (1.0 - (-(time - self.time).max(0.0) / FOLLOW_SECONDS).exp()) as f32;
            self.position += (target - self.position) * amount;
        }
        self.settings = settings;
        self.time = time;
        self.frame = frame;
    }
}
fn mapped_value(settings: &AudioSidechain, x: f32) -> f32 {
    let db = settings.lower_db + x * (settings.upper_db - settings.lower_db);
    settings.map(10.0_f32.powf(db / 20.0))
}
fn marker(
    ui: &Ui,
    id: egui::Id,
    plot: Rect,
    settings: &AudioSidechain,
    live: Option<[f32; 2]>,
) -> Option<egui::Pos2> {
    let id = id.with("sidechain-live-marker");
    let Some([db, value]) =
        live.filter(|v| ui.is_rect_visible(plot) && v.iter().all(|v| v.is_finite()))
    else {
        ui.data_mut(|data| data.remove::<Marker>(id));
        return None;
    };
    // Animate the display only. Audio and numeric readouts stay immediate.
    let target = egui::vec2(
        ((db - settings.lower_db) / (settings.upper_db - settings.lower_db)).clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    );
    // Follow the input coordinate along the mapping curve. Preserve any measured
    // output offset (for example while an edited mapping awaits audio publication).
    let path_target = egui::vec2(target.x, target.y - mapped_value(settings, target.x));
    let time = ui.input(|input| input.time);
    let frame = ui.ctx().cumulative_frame_nr();
    let mut display = ui
        .data_mut(|data| data.get_temp::<Marker>(id))
        .unwrap_or(Marker {
            position: path_target,
            settings: *settings,
            time,
            frame,
        });
    display.follow(path_target, *settings, time, frame);
    let mut shown = egui::vec2(
        display.position.x,
        (mapped_value(settings, display.position.x) + display.position.y).clamp(0.0, 1.0),
    );
    let distance = (shown - target) * plot.size();
    if distance.length_sq() > 0.01 {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    } else {
        display.position = path_target;
        shown = target;
    }
    let point = egui::pos2(
        plot.left() + shown.x * plot.width(),
        plot.bottom() - shown.y * plot.height(),
    );
    ui.data_mut(|data| data.insert_temp(id, display));
    Some(point)
}

pub(super) fn show(ui: &mut Ui, settings: &AudioSidechain, live: Option<[f32; 2]>) -> Rect {
    let height = 3.0 * knobs::CHOICE_SIZE + 2.0 * f32::from(theme::TOOLBAR_PADDING);
    let (chart, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let plot = Rect::from_min_max(
        chart.min + egui::vec2(24.0, 50.0),
        chart.max - egui::vec2(24.0, 18.0),
    );
    let colors = theme::palette(ui.ctx());
    let painter = ui.painter_at(chart);
    painter.rect_filled(chart, 2.0, colors.background);
    let readings: [String; 2] = std::array::from_fn(|column| {
        live.filter(|v| v[column].is_finite())
            .map_or("—".into(), |v| {
                if column == 0 {
                    format!("{:.1} dBFS", v[0])
                } else {
                    format!("{:.3}", v[1])
                }
            })
    });
    for (column, label) in ["Input", "Value"].into_iter().enumerate() {
        let x = chart.left() + chart.width() * (0.25 + column as f32 * 0.5);
        painter.text(
            egui::pos2(x, chart.top() + 8.0),
            Align2::CENTER_TOP,
            label,
            egui::TextStyle::Small.resolve(ui.style()),
            colors.secondary,
        );
        painter.text(
            egui::pos2(x, chart.top() + 25.0),
            Align2::CENTER_TOP,
            &readings[column],
            crate::fonts::numeric(9.0),
            colors.text,
        );
    }

    let grid = Stroke::new(1.0, colors.secondary.gamma_multiply(0.12));
    let font = FontId::proportional(9.0);
    for i in 0..=4 {
        let fraction = i as f32 / 4.0;
        let x = plot.left() + fraction * plot.width();
        let y = plot.bottom() - fraction * plot.height();
        painter.vline(x, plot.y_range(), grid);
        painter.hline(plot.x_range(), y, grid);
        if i % 2 == 0 {
            let db = settings.lower_db + fraction * (settings.upper_db - settings.lower_db);
            painter.text(
                egui::pos2(x, plot.bottom() + 3.0),
                Align2::CENTER_TOP,
                format!("{db:.1}"),
                font.clone(),
                colors.secondary,
            );
            painter.text(
                egui::pos2(plot.left() - 4.0, y),
                Align2::RIGHT_CENTER,
                format!("{fraction:.1}"),
                font.clone(),
                colors.secondary,
            );
        }
    }
    let curve = ui.painter_at(plot);
    curve.line_segment(
        [plot.left_bottom(), plot.right_top()],
        Stroke::new(1.0, colors.secondary.gamma_multiply(0.15)),
    );
    curve.add(egui::Shape::line(
        (0..=160)
            .map(|i| {
                let x = i as f32 / 160.0;
                egui::pos2(
                    plot.left() + x * plot.width(),
                    plot.bottom() - mapped_value(settings, x) * plot.height(),
                )
            })
            .collect(),
        Stroke::new(1.5, colors.secondary),
    ));
    if let Some(point) = marker(ui, response.id, plot, settings, live) {
        curve.circle_filled(point, 2.5, theme::accent(ui.ctx()));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            response.enabled(),
            format!(
                "Sidechain response: Input {}; Value {}",
                readings[0], readings[1]
            ),
        )
    });
    response.on_hover_text(format!(
        "Input range: {:.1}–{:.1} dBFS",
        settings.lower_db, settings.upper_db
    ));
    plot
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_follow_is_frame_rate_independent_and_bounded() {
        let settings = AudioSidechain::new(daw_core::Id::new_v4());
        let start = egui::vec2(0.1, 0.1);
        let target = egui::vec2(0.9, -0.1);
        let expected = target + (start - target) * (-0.1 / FOLLOW_SECONDS).exp() as f32;
        for frames in [3, 6, 12] {
            let mut display = Marker {
                position: start,
                settings,
                time: 0.0,
                frame: 0,
            };
            for frame in 1..=frames {
                display.follow(target, settings, 0.1 * frame as f64 / frames as f64, frame);
                assert!((start.x..=target.x).contains(&display.position.x));
                assert!((target.y..=start.y).contains(&display.position.y));
            }
            assert!((display.position - expected).length() < 0.00001);
            assert_ne!(
                display.position, target,
                "telemetry steps must not jump straight to the target"
            );
        }
    }
    #[test]
    fn marker_resets_for_mapping_changes_hidden_frames_and_clock_restart() {
        let mut settings = AudioSidechain::new(daw_core::Id::new_v4());
        let mut display = Marker {
            position: Vec2::ZERO,
            settings,
            time: 0.0,
            frame: 0,
        };
        display.follow(Vec2::ONE, settings, 0.016, 1);
        let position = display.position;
        assert!(position.x > 0.0 && position.x < 1.0);
        display.follow(Vec2::ONE, settings, 0.016, 1);
        assert_eq!(
            display.position, position,
            "repeat UI passes must not animate twice"
        );
        settings.invert = true;
        display.follow(egui::vec2(0.25, 0.0), settings, 0.032, 2);
        assert_eq!(display.position, egui::vec2(0.25, 0.0));
        display.follow(egui::vec2(0.7, 0.0), settings, 0.2, 6);
        assert_eq!(display.position, egui::vec2(0.7, 0.0));
        display.follow(egui::vec2(0.2, 0.0), settings, 0.0, 7);
        assert_eq!(display.position, egui::vec2(0.2, 0.0));
    }
    #[test]
    fn live_markers_follow_the_curve_remain_route_local_and_clear_missing_data() {
        for invert in [false, true] {
            let ctx = egui::Context::default();
            let mut settings = AudioSidechain::new(daw_core::Id::new_v4());
            settings.curve = daw_unit_core::SidechainCurve::Amplitude;
            settings.invert = invert;
            let plot = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(150.0, 100.0));
            let low = [settings.lower_db, mapped_value(&settings, 0.0)];
            let high = [settings.upper_db, mapped_value(&settings, 1.0)];
            let point = |x: f32, y: f32| {
                egui::pos2(
                    plot.left() + x * plot.width(),
                    plot.bottom() - y * plot.height(),
                )
            };
            let draw = |time, input| {
                let mut positions = [None, None];
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(300.0, 200.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        positions[0] = marker(ui, egui::Id::new("a"), plot, &settings, input);
                        positions[1] = marker(ui, egui::Id::new("b"), plot, &settings, Some(high));
                    },
                );
                output.textures_delta.clear();
                positions
            };
            assert_eq!(
                draw(0.0, Some(low)),
                [Some(point(0.0, low[1])), Some(point(1.0, high[1]))]
            );
            let positions = draw(0.016, Some(high));
            let moving = positions[0].unwrap();
            assert!(moving.x > plot.left() && moving.x < plot.right());
            let x = (moving.x - plot.left()) / plot.width();
            assert!((moving.y - point(x, mapped_value(&settings, x)).y).abs() < 0.0001);
            assert_eq!(positions[1], Some(point(1.0, high[1])));
            assert_eq!(draw(0.032, None)[0], None);
            assert_eq!(draw(0.048, Some(high))[0], Some(point(1.0, high[1])));
            assert_eq!(draw(0.064, Some([f32::NAN, 0.0]))[0], None);
            assert_eq!(
                draw(0.080, Some([settings.upper_db, 0.4]))[0],
                Some(point(1.0, 0.4)),
                "a fresh marker must preserve the measured output even while a mapping edit awaits publication"
            );
        }
    }
    #[test]
    fn graph_matches_sidechain_mapping_in_both_themes_and_clips_live_readings() {
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            for mapping in [
                daw_unit_core::SidechainCurve::Decibels,
                daw_unit_core::SidechainCurve::Amplitude,
            ] {
                for invert in [false, true] {
                    let ctx = egui::Context::default();
                    crate::fonts::configure(&ctx);
                    crate::theme::configure(&ctx);
                    daw_unit_ui::theme::apply(&ctx, mode, theme::LIGHT, theme::ACCENT);
                    let mut settings = AudioSidechain::new(daw_core::Id::new_v4());
                    settings.curve = mapping;
                    settings.invert = invert;
                    let before = settings;
                    let mut plot = Rect::NOTHING;
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        ui.set_width(
                            daw_unit_ui::spectrum_graph::WIDTH
                                - 2.0 * f32::from(daw_unit_ui::layout::PANEL_MARGIN),
                        );
                        plot = show(ui, &settings, Some([240.0, 2.0]));
                    });
                    let frame = out
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(r)
                                if r.corner_radius == egui::CornerRadius::same(2)
                                    && r.fill == theme::palette(&ctx).background =>
                            {
                                Some(r.rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(frame.bottom() - plot.bottom(), 18.0);
                    assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Input · dBFS")));
                    for label in ["Input", "Value", "240.0 dBFS", "2.000"] {
                        let text = out
                            .shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Text(t) if t.galley.text() == label => {
                                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                                }
                                _ => None,
                            })
                            .unwrap();
                        assert!(frame.contains_rect(text));
                        assert!(text.bottom() < plot.top());
                    }
                    let points = out
                        .shapes
                        .iter()
                        .find_map(|s| match &s.shape {
                            egui::Shape::Path(p) if p.points.len() == 161 => Some(&p.points),
                            _ => None,
                        })
                        .expect("mapping curve");
                    assert!(
                        points
                            .iter()
                            .all(|p| p.is_finite() && plot.expand(0.001).contains(*p))
                    );
                    let middle_db = (settings.lower_db + settings.upper_db) * 0.5;
                    let mapped = settings.map(10.0_f32.powf(middle_db / 20.0));
                    assert!(
                        (points[80].y - (plot.bottom() - mapped * plot.height())).abs() < 0.001
                    );
                    let dot = out
                        .shapes
                        .iter()
                        .find_map(|s| match &s.shape {
                            egui::Shape::Circle(c) if c.radius == 2.5 => Some(c.center),
                            _ => None,
                        })
                        .expect("actual live marker");
                    assert_eq!(dot, plot.right_top());
                    let colors = theme::palette(&ctx);
                    assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == colors.background && r.corner_radius == egui::CornerRadius::same(2))));
                    assert_eq!(settings, before, "painting is not an edit");
                    out.textures_delta.clear();
                }
            }
        }
    }
}
