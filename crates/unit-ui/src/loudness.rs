//! Shared built-in and standalone analyzer body. History selection is view state.
use crate::{control_capture, layout as panels, menus::Dropdown, meters, rows};

use daw_unit_core::{
    LOUDNESS_HISTORY, LOUDNESS_SCALARS, LoudnessMeter, LoudnessSettings as Settings,
};

use egui::{Rect, Response, RichText, Ui};

pub const STRIP_COUNT: usize = 7;

pub const WIDTH: f32 = 8.0 * panels::UNIT_STRIP_WIDTH;

pub const RESET: u32 = 256;

pub const HOLD_ON: u32 = 512;

pub const HOLD_OFF: u32 = 1024;

pub struct Bounds {
    pub sections: [Rect; STRIP_COUNT],
    pub footers: [Rect; STRIP_COUNT],
    pub controls: [Rect; 3],
    pub actions: u32,
    pub power: Rect,
    pub hold: Rect,
    pub reset: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            sections: [Rect::NOTHING; STRIP_COUNT],
            footers: [Rect::NOTHING; STRIP_COUNT],
            controls: [Rect::NOTHING; 3],
            actions: 0,
            power: Rect::NOTHING,
            hold: Rect::NOTHING,
            reset: Rect::NOTHING,
        }
    }
}
fn selector(ui: &mut Ui, label: &str, value: &mut u8, choices: &[&str]) -> Response {
    rows::selector_row(ui, label, |ui| {
        let before = *value;

        let mut r = Dropdown::from_id_salt(label)
            .small_items()
            .truncate()
            .width(ui.available_width())
            .selected_text(RichText::new(choices[*value as usize]).size(panels::HEADER_FONT_SIZE))
            .show_ui(ui, |ui| {
                for (i, label) in choices.iter().enumerate() {
                    ui.selectable_value(value, i as u8, *label);
                }
            })
            .response
            .on_hover_text(label);

        if before != *value {
            r.mark_changed();
        }
        control_capture::record(ui, &r);

        r
    })
}
fn readout(ui: &mut Ui, label: &str, value: f32, unit: &str, enabled: bool) {
    let palette = crate::theme::palette(ui.ctx());

    let color = if enabled {
        palette.text
    } else {
        palette.secondary
    };

    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(label)
                .size(panels::HEADER_FONT_SIZE)
                .color(palette.secondary),
        );

        let value = if !enabled {
            format!("0.0 {unit}")
        } else if value.is_finite() {
            format!("{value:.1} {unit}")
        } else {
            format!("−∞ {unit}")
        };

        ui.label(
            RichText::new(value)
                .size(panels::HEADER_FONT_SIZE)
                .color(color),
        );
    });
}
fn history(ui: &mut Ui, settings: &Settings, values: &[f32]) {
    let id = ui.id().with("history-metric");
    let mut metric = ui.data_mut(|d| d.get_temp::<u8>(id).unwrap_or(0));
    let selector_top = ui.cursor().min.y;
    // View state stays outside host parameter capture and saved settings.
    Dropdown::from_id_salt(id)
        .small_items()
        .truncate()
        .selected_text(
            RichText::new(if metric == 0 {
                "Momentary"
            } else {
                "Short term"
            })
            .size(panels::HEADER_FONT_SIZE),
        )
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut metric, 0, "Momentary");
            ui.selectable_value(&mut metric, 1, "Short term");
        });
    ui.data_mut(|d| d.insert_temp(id, metric));
    let padding = f32::from(crate::theme::TOOLBAR_PADDING);
    let height =
        3.0 * crate::knobs::CHOICE_SIZE + 2.0 * padding - (ui.cursor().min.y - selector_top);
    let (graph, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let plot = graph.shrink2(egui::vec2(4.0, 12.0));
    let palette = crate::theme::palette(ui.ctx());
    let y = |value: f32| egui::remap_clamp(value, -60.0..=0.0, plot.bottom()..=plot.top());
    let font = egui::FontId::proportional(9.0);
    for tick in [-48.0, -24.0, 0.0] {
        ui.painter().hline(
            plot.x_range(),
            y(tick),
            egui::Stroke::new(0.5, palette.secondary.gamma_multiply(0.25)),
        );
        ui.painter().text(
            egui::pos2(plot.left(), y(tick)),
            egui::Align2::LEFT_BOTTOM,
            format!("{tick:.0}"),
            font.clone(),
            palette.secondary,
        );
    }
    if settings.enabled {
        if let Some(target) = settings.target_lufs() {
            ui.painter().hline(
                plot.x_range(),
                y(target),
                egui::Stroke::new(1.0, palette.secondary),
            );
        }
        let start = LOUDNESS_SCALARS + metric as usize * LOUDNESS_HISTORY;
        let history = &values[start..start + LOUDNESS_HISTORY];
        let x = |i| plot.left() + i as f32 / (LOUDNESS_HISTORY - 1) as f32 * plot.width();
        for i in 1..LOUDNESS_HISTORY {
            if history[i - 1].is_finite() && history[i].is_finite() && values[12] > 0.0 {
                ui.painter().line_segment(
                    [
                        egui::pos2(x(i - 1), y(history[i - 1])),
                        egui::pos2(x(i), y(history[i])),
                    ],
                    egui::Stroke::new(1.3, crate::theme::accent(ui.ctx())),
                );
            }
        }
    }
    ui.painter().text(
        graph.left_bottom(),
        egui::Align2::LEFT_BOTTOM,
        "−3 min",
        font.clone(),
        palette.secondary,
    );
    ui.painter().text(
        graph.right_bottom(),
        egui::Align2::RIGHT_BOTTOM,
        "Now",
        font,
        palette.secondary,
    );
    response.on_hover_text(
        "Rolling three-minute history. One point per second. Target is a display reference.",
    );
}
fn output_row(
    ui: &mut Ui,
    row: usize,
    settings: &Settings,
    values: &[f32],
    meter: Option<&LoudnessMeter>,
    bounds: &mut Bounds,
) {
    match row {
        0 => {
            let incomplete = values[13] > 0.0;
            let seconds = values[12] as u64;
            let color = if incomplete {
                ui.visuals().error_fg_color
            } else {
                crate::theme::palette(ui.ctx()).secondary
            };
            ui.label(
                RichText::new(if incomplete { "Incomplete" } else { "Elapsed" })
                    .size(panels::HEADER_FONT_SIZE)
                    .color(color),
            )
            .on_hover_text(if incomplete {
                "An observation was lost or invalid. Reset and measure again."
            } else {
                "Measured audio duration"
            });
            ui.label(
                RichText::new(format!("{}:{:02}", seconds / 60, seconds % 60))
                    .size(panels::HEADER_FONT_SIZE),
            );
        }
        1 => {
            let held = meter.is_some_and(|m| m.held());
            let response = ui.add_enabled(
                settings.enabled && meter.is_some(),
                egui::Button::new(if held { "Resume" } else { "Hold" }),
            );
            bounds.hold = response.rect;
            if response.clicked() {
                if let Some(meter) = meter {
                    meter.set_hold(!held);
                }
                bounds.actions |= if held { HOLD_OFF } else { HOLD_ON };
            }
        }
        2 => {
            let response = ui.add_enabled(
                settings.enabled && meter.is_some(),
                egui::Button::new("Reset"),
            );
            bounds.reset = response.rect;
            if response.clicked() {
                if let Some(meter) = meter {
                    meter.reset();
                }
                bounds.actions |= RESET;
            }
        }
        _ => unreachable!(),
    }
}
fn option_row(ui: &mut Ui, row: usize, settings: &mut Settings, bounds: &mut Bounds) {
    bounds.controls[row] = match row {
        0 => selector(
            ui,
            "Target",
            &mut settings.target,
            &["Off", "−14 LUFS", "−23 LUFS"],
        ),
        1 => {
            let mut value = u8::from(settings.hq);
            let response = selector(ui, "TP quality", &mut value, &["ECO", "HQ"]);
            settings.hq = value != 0;
            response
        }
        2 => {
            let mut value = u8::from(settings.continuous);
            let response = selector(ui, "Clock", &mut value, &["Transport", "Continuous"]);
            settings.continuous = value != 0;
            response
        }
        _ => unreachable!(),
    }
    .rect;
}
pub fn show(
    ui: &mut Ui,
    settings: &mut Settings,
    compact: bool,
    height: f32,
    meter: Option<&LoudnessMeter>,
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds::default();
    let reading = meter.map(|meter| meter.reading()).unwrap_or_default();
    let mut values = meters::unit_reading(settings.enabled, reading).values;
    // Enabled but not measured is unavailable, rather than a 0 LUFS signal.
    if settings.enabled && values[12] == 0.0 {
        for i in [0, 1, 2, 3, 4, 5, 7, 8, 9] {
            values[i] = f32::NEG_INFINITY;
        }
    }
    let names = [
        "Options", "Loudness", "Maxima", "Range", "Dynamics", "History", "Out",
    ];
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for (strip, name) in names.iter().enumerate() {
            let width = panels::UNIT_STRIP_WIDTH * if strip == 5 { 2.0 } else { 1.0 };
            let rect = panels::unit_strip_with_width(
                ui,
                height,
                width,
                egui::UiBuilder::new().id_salt(strip),
                |ui| {
                    if !compact {
                        if strip == 5 {
                            history(ui, settings, &values);
                        } else {
                            for row in 0..3 {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), crate::knobs::CHOICE_SIZE),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_height(crate::knobs::CHOICE_SIZE);
                                        match strip {
                                            0 => option_row(ui, row, settings, &mut bounds),
                                            1 => readout(
                                                ui,
                                                ["Momentary", "Short term", "Integrated"][row],
                                                values[row],
                                                "LUFS",
                                                settings.enabled,
                                            ),
                                            2 => readout(
                                                ui,
                                                ["Max M", "Max S", "Max TP"][row],
                                                values[3 + row],
                                                if row == 2 { "dBTP" } else { "LUFS" },
                                                settings.enabled,
                                            ),
                                            3 => readout(
                                                ui,
                                                ["LRA", "Low", "High"][row],
                                                values[6 + row],
                                                if row == 0 { "LU" } else { "LUFS" },
                                                settings.enabled,
                                            ),
                                            4 => readout(
                                                ui,
                                                ["True peak", "PSR", "PLR"][row],
                                                values[9 + row],
                                                if row == 0 { "dBTP" } else { "LU" },
                                                settings.enabled,
                                            ),
                                            6 => output_row(
                                                ui,
                                                row,
                                                settings,
                                                &values,
                                                meter,
                                                &mut bounds,
                                            ),
                                            _ => unreachable!(),
                                        }
                                    },
                                );
                            }
                        }
                    }
                    bounds.footers[strip] = rows::centered(
                        ui,
                        if compact {
                            height
                        } else {
                            panels::COMPACT_BUTTON_SIZE
                        },
                        |ui| rows::unit_label(ui, name),
                    )
                    .response
                    .rect;
                },
            )
            .response
            .rect;

            if strip == 6 && !compact {
                meters::effect_output_level(
                    ui,
                    rect,
                    panels::PANEL_MARGIN,
                    level,
                    settings.enabled,
                );
            }
            bounds.sections[strip] = rect;
        }
    });
    if settings.enabled {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    bounds
}
