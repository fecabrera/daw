//! Shared Transient Shaper control strips for the DAW and plugin editors.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout::{self, COMPACT_BUTTON_SIZE, HEADER_FONT_SIZE, PANEL_MARGIN, UNIT_STRIP_WIDTH},
    meters, parameter_knobs, rows, spectrum_graph, theme,
};
use daw_unit_core::{
    ModulationParameter as Parameter, TransientShaperClipMode, TransientShaperSettings,
};
use egui::{Align2, FontId, Rect, Response, Stroke, Ui, UiBuilder, Vec2, pos2};

const PREVIEW_WIDTH: f32 = 220.0;
pub const SIZE: Vec2 = Vec2::new(
    PREVIEW_WIDTH + 3.0 * UNIT_STRIP_WIDTH,
    layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);

pub struct Bounds {
    /// Host parameter order, excluding Bypass.
    pub controls: [Option<Response>; 7],
    pub sections: [Rect; 4],
}

pub fn show(
    ui: &mut Ui,
    settings: &mut TransientShaperSettings,
    compact: bool,
    compact_height: f32,
    markers: &[Option<ModulationMarker>; 6],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut sections = [Rect::NOTHING; 4];
    let mut controls = std::array::from_fn(|_| None);
    let height = if compact {
        compact_height
    } else {
        layout::unit_control_height()
    };
    let footer_height = if compact { height } else { COMPACT_BUTTON_SIZE };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        sections[0] = layout::unit_strip_with_width(
            ui,
            height,
            PREVIEW_WIDTH,
            UiBuilder::new().id_salt("gain-preview"),
            |ui| {
                if !compact {
                    let chart = spectrum_graph::chart(ui, height, false);
                    gain_preview(ui, chart, settings);
                }
                rows::centered(ui, footer_height, |ui| {
                    rows::unit_label(ui, "Gain preview")
                        .on_hover_text("Illustrative gain change on example hits")
                });
            },
        )
        .response
        .rect;
        for (column, (name, indices)) in [
            ("Shape", &[0, 1, 2][..]),
            ("Detector", &[3][..]),
            ("Out", &[4, 5][..]),
        ]
        .into_iter()
        .enumerate()
        {
            let strip = layout::unit_strip(ui, height, UiBuilder::new().id_salt(name), |ui| {
                if !compact {
                    for &index in indices {
                        let response =
                            knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, settings| {
                                let value = match index {
                                    0 => &mut settings.attack_db,
                                    1 => &mut settings.pump_db,
                                    2 => &mut settings.sustain_db,
                                    3 => &mut settings.speed,
                                    4 => &mut settings.level_db,
                                    _ => &mut settings.dry_wet,
                                };
                                let parameter = Parameter::TRANSIENT_SHAPER[index];
                                ui.add(
                                    Knob::new(
                                        value,
                                        parameter_knobs::destination(parameter)
                                            .named(parameter.label()),
                                    )
                                    .modulated(
                                        markers[index].as_ref(),
                                        &mut settings.modulation_open,
                                    ),
                                )
                            });
                        controls[[1, 2, 3, 4, 0, 5][index]] = Some(response);
                    }
                    if name == "Out" {
                        controls[6] = Some(rows::selector_row(ui, "Clip Mode", |ui| {
                            let before = settings.clip_mode;
                            let mut response = crate::menus::Dropdown::from_id_salt("clip-mode")
                                .small_items()
                                .selected_text(
                                    egui::RichText::new(before.label()).size(HEADER_FONT_SIZE),
                                )
                                .width(65.0)
                                .show_ui(ui, |ui| {
                                    for mode in TransientShaperClipMode::ALL {
                                        ui.selectable_value(
                                            &mut settings.clip_mode,
                                            mode,
                                            mode.label(),
                                        );
                                    }
                                })
                                .response;
                            if before != settings.clip_mode {
                                response.mark_changed();
                            }
                            response
                        }));
                    } else if name == "Detector" {
                        ui.add_space(
                            2.0 * (knobs::CHOICE_SIZE + f32::from(theme::TOOLBAR_PADDING)),
                        );
                    }
                }
                rows::centered(ui, footer_height, |ui| rows::unit_label(ui, name));
            });
            sections[column + 1] = strip.response.rect;
            if name == "Out" && !compact {
                meters::effect_output_level(
                    ui,
                    strip.response.rect,
                    PANEL_MARGIN,
                    level,
                    settings.enabled,
                );
            }
        }
    });
    Bounds { controls, sections }
}

fn gain_preview(ui: &Ui, chart: Rect, settings: &TransientShaperSettings) {
    let graph = chart.shrink2(spectrum_graph::EXPANDED_PLOT_INSET);
    let palette = theme::palette(ui.ctx());
    let (painter, grid) = spectrum_graph::surface(ui, chart);
    let font = FontId::proportional(9.0);
    let speed = settings.speed;
    let rise = 1.2 + (1.0 - speed) * 5.0;
    let decay = 10.0 + (1.0 - speed) * 24.0;
    let bump = |age: f32, rise: f32, decay: f32| {
        let age = age.max(0.0);
        (1.0 - (-age / rise).exp()) * (-age / decay).exp()
    };
    let samples: [f32; 201] = std::array::from_fn(|i| {
        if !settings.enabled {
            return 0.0;
        }
        let time = 2.0 * i as f32;
        [22.0, 134.0, 259.0, 331.0]
            .into_iter()
            .map(|hit| {
                let age = time - hit;
                let attack = 1.38 * bump(age, rise, decay);
                let pump = 1.42 * bump(age - 8.0 - (1.0 - speed) * 15.0, rise * 1.5, decay * 1.5);
                let body = 1.39 * bump(age - 24.0, 11.0 + (1.0 - speed) * 15.0, decay * 3.6);
                settings.attack_db * attack - settings.pump_db * pump + settings.sustain_db * body
            })
            .sum()
    });
    const GAIN_LIMIT_DB: i32 = 40;
    let limit = GAIN_LIMIT_DB as f32;
    let y = |db| egui::remap(db, -limit..=limit, graph.bottom()..=graph.top());
    for db in (-GAIN_LIMIT_DB..=GAIN_LIMIT_DB).step_by(10) {
        let label = if db == 0 {
            db.to_string()
        } else {
            format!("{db:+}")
        };
        painter.hline(
            graph.x_range(),
            y(db as f32),
            if db == 0 {
                Stroke::new(1.0, palette.secondary.gamma_multiply(0.5))
            } else {
                grid
            },
        );
        painter.text(
            pos2(graph.left() - 4.0, y(db as f32)),
            Align2::RIGHT_CENTER,
            label,
            font.clone(),
            palette.secondary,
        );
    }
    for (i, label) in ["0", "100", "200", "300", "400 ms"].into_iter().enumerate() {
        let x = graph.left() + graph.width() * i as f32 / 4.0;
        painter.vline(x, graph.y_range(), grid);
        painter.text(
            pos2(x, graph.bottom() + 3.0),
            Align2::CENTER_TOP,
            label,
            font.clone(),
            palette.secondary,
        );
    }
    painter.text(
        pos2(graph.left() - 4.0, graph.bottom() + 3.0),
        Align2::RIGHT_TOP,
        "dB",
        font,
        palette.secondary,
    );
    let curve: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(i, db)| pos2(graph.left() + graph.width() * i as f32 / 200.0, y(*db)))
        .collect();
    let painter = painter.with_clip_rect(graph);
    let accent = theme::accent(ui.ctx());
    let mut fill = egui::Mesh::default();
    for point in &curve {
        fill.colored_vertex(*point, accent.gamma_multiply(0.18));
        fill.colored_vertex(pos2(point.x, y(0.0)), accent.gamma_multiply(0.18));
    }
    for i in (0..400).step_by(2) {
        fill.add_triangle(i, i + 1, i + 2);
        fill.add_triangle(i + 2, i + 1, i + 3);
    }
    painter.add(egui::Shape::mesh(fill));
    painter.add(egui::Shape::line(
        curve,
        Stroke::new(spectrum_graph::CURVE_STROKE_WIDTH, accent),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auto_filter, compressor, compressor_curve};

    #[test]
    fn gain_preview_axes_stay_fixed_across_shape_settings() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        theme::configure(&ctx);
        let chart = Rect::from_min_size(pos2(10.0, 10.0), Vec2::new(200.0, 172.0));
        let paint = |settings: TransientShaperSettings| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                gain_preview(ui, chart, &settings);
            });
            out.textures_delta.clear();
            let labels: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.pos)),
                    _ => None,
                })
                .collect();
            let curve = out
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) if path.points.len() == 201 => {
                        assert!(
                            path.points
                                .iter()
                                .all(|point| shape.clip_rect.contains(*point))
                        );
                        Some(path.points.clone())
                    }
                    _ => None,
                })
                .unwrap();
            (labels, curve)
        };
        let (neutral_labels, neutral_curve) = paint(TransientShaperSettings::default());
        assert!(
            neutral_curve
                .iter()
                .all(|point| point.y == chart.center().y)
        );
        assert_eq!(
            neutral_labels
                .iter()
                .map(|(text, _)| text.as_str())
                .collect::<Vec<_>>(),
            [
                "-40", "-30", "-20", "-10", "0", "+10", "+20", "+30", "+40", "0", "100", "200",
                "300", "400 ms", "dB"
            ]
        );
        for (attack_db, pump_db, sustain_db) in [(-12.0, 12.0, -12.0), (12.0, 0.0, 12.0)] {
            let (labels, curve) = paint(TransientShaperSettings {
                attack_db,
                pump_db,
                sustain_db,
                speed: 0.0,
                ..Default::default()
            });
            assert_eq!(labels, neutral_labels);
            assert_ne!(curve, neutral_curve);
        }
    }

    #[test]
    fn expanded_strips_match_auto_filter_and_compressor_geometry() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        theme::configure(&ctx);
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1600.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                ui.horizontal_top(|ui| {
                    let height = layout::unit_control_height();
                    let transient = show(
                        ui,
                        &mut Default::default(),
                        false,
                        0.0,
                        &[const { None }; 6],
                        0.0.into(),
                    );
                    let filter = auto_filter::show(
                        ui,
                        &mut Default::default(),
                        (false, height),
                        &[const { None }; 7],
                        true,
                        0.0.into(),
                    );
                    let comp = compressor::show(
                        ui,
                        &mut Default::default(),
                        false,
                        height,
                        compressor::Controls {
                            markers: &[const { None }; 8],
                            reduction: 0.0.into(),
                            level: 0.0.into(),
                            graph: compressor_curve::Input {
                                live_edits: false,
                                detector_level: 0.0,
                                history: None,
                            },
                        },
                    );
                    for strip in transient.sections {
                        assert_eq!(strip.y_range(), filter.sections[0].y_range());
                        assert_eq!(strip.y_range(), comp.graph.section.y_range());
                        assert_eq!(strip.height(), SIZE.y);
                    }
                    for pair in transient.sections.windows(2) {
                        assert_eq!(pair[0].right(), pair[1].left());
                    }
                    for (index, row) in [(1, 0), (2, 1), (3, 2), (4, 0), (0, 0), (5, 1)] {
                        let rect = transient.controls[index].as_ref().unwrap().rect;
                        assert_eq!(rect.size(), comp.controls[1][row].size());
                        assert_eq!(rect.y_range(), comp.controls[1][row].y_range());
                    }
                    for (index, row) in [(0, 0), (5, 1)] {
                        assert_eq!(
                            transient.controls[index].as_ref().unwrap().rect.y_range(),
                            filter.knobs[3][row].y_range()
                        );
                    }
                });
            },
        );
        out.textures_delta.clear();

        let baseline = |label| {
            out.shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos.y),
                    _ => None,
                })
                .unwrap()
        };
        for label in ["Shape", "Detector", "Gain preview", "Out", "Cutoff", "Comp"] {
            assert_eq!(baseline(label), baseline("Curve"), "{label} footer");
        }
    }
}
