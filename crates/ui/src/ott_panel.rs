use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{CompressorDetection, OttSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[[Rect; 3]; 2]; 4],
    pub sections: [Rect; 4],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}
fn db(v: f32) -> String {
    format!("{v:.1} dB")
}
fn percent(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}
fn time(v: f32) -> String {
    if v < 10.0 {
        format!("{v:.1} ms")
    } else {
        format!("{v:.0} ms")
    }
}
fn timing(attack: bool) -> KnobSpec {
    let (name, min, max, reset) = if attack {
        ("Attack", 0.1, 200.0, 10.0)
    } else {
        ("Release", 10.0, 2000.0, 100.0)
    };
    KnobSpec::new(name, min..=max, reset, 0.1)
        .scale(
            if attack {
                |v| (v / 0.1).ln() / 2000_f32.ln()
            } else {
                |v| (v / 10.0).ln() / 200_f32.ln()
            },
            if attack {
                |p| 0.1 * 2000_f32.powf(p)
            } else {
                |p| 10.0 * 200_f32.powf(p)
            },
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(time)
        .format(time)
        .endpoint_labels(
            if attack { "0.1" } else { "10" },
            if attack { "200" } else { "2k" },
        )
}
fn frequency(low: bool) -> KnobSpec {
    let (name, min, max, reset) = if low {
        ("Low/Mid crossover", 20.0, 2000.0, 250.0)
    } else {
        ("Mid/High crossover", 200.0, 20_000.0, 2500.0)
    };
    KnobSpec::new(name, min..=max, reset, 1.0)
        .scale(
            if low {
                |v| (v / 20.0).log10() / 2.0
            } else {
                |v| (v / 200.0).log10() / 2.0
            },
            if low {
                |p| 20.0 * 100_f32.powf(p)
            } else {
                |p| 200.0 * 100_f32.powf(p)
            },
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels(
            if low { "20" } else { "200" },
            if low { "2k" } else { "20k" },
        )
}
fn row(ui: &mut Ui, settings: &mut OttSettings, section: usize, column: usize, row: usize) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        if section < 3 {
            match (column, row) {
                (0, 0) if section != 1 => {
                    let (value, limit) = if section == 0 {
                        (&mut s.low_frequency_hz, s.high_frequency_hz - 1.0)
                    } else {
                        (&mut s.high_frequency_hz, s.low_frequency_hz + 1.0)
                    };
                    let response = ui.add(Knob::new(value, frequency(section == 0)));
                    if response.changed() {
                        *value = if section == 0 {
                            value.min(limit)
                        } else {
                            value.max(limit)
                        };
                    }
                    response
                }
                (0, 0) => {
                    ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover())
                }
                (1, 0) => ui.add(Knob::new(
                    &mut s.bands[section].depth,
                    KnobSpec::new("Compression depth", 0.0..=1.0, 1.0, 0.01)
                        .value_label(percent)
                        .format(percent)
                        .endpoint_labels("0", "100"),
                )),
                (_, 1) => ui.add(Knob::new(
                    if column == 0 {
                        &mut s.bands[section].input_db
                    } else {
                        &mut s.bands[section].output_db
                    },
                    KnobSpec::new(
                        if column == 0 {
                            "Band input"
                        } else {
                            "Band output"
                        },
                        -24.0..=24.0,
                        0.0,
                        0.1,
                    )
                    .centered(0.0)
                    .bipolar()
                    .value_label(db)
                    .format(db)
                    .endpoint_labels("−24", "+24"),
                )),
                (_, 2) => ui.add(Knob::new(
                    if column == 0 {
                        &mut s.bands[section].attack_ms
                    } else {
                        &mut s.bands[section].release_ms
                    },
                    timing(column == 0),
                )),
                _ => unreachable!(),
            }
        } else {
            match row {
                0 => knobs::gain_with_markers_named(ui, &mut s.level_db, "OTT output level"),
                1 => ui.add(Knob::new(
                    &mut s.dry_wet,
                    KnobSpec::new("Dry/wet", 0.0..=1.0, 1.0, 0.01)
                        .value_label(percent)
                        .format(percent)
                        .endpoint_labels("Dry", "Wet"),
                )),
                2 => {
                    let mut position = f32::from(s.detection == CompressorDetection::Rms);
                    let response = ui.add(Knob::new(
                        &mut position,
                        KnobSpec::new("Detection mode", 0.0..=1.0, 0.0, 1.0)
                            .two_way_labels("Peak", "RMS")
                            .format(|v| {
                                if v == 0.0 {
                                    "Peak".into()
                                } else {
                                    "RMS".into()
                                }
                            }),
                    ));
                    if response.changed() {
                        s.detection = if position == 0.0 {
                            CompressorDetection::Peak
                        } else {
                            CompressorDetection::Rms
                        };
                    }
                    response
                }
                _ => unreachable!(),
            }
        }
    })
    .rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut OttSettings,
    compact: bool,
    height: f32,
    gains: [f32; 3],
) -> (Bounds, egui::Response) {
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[[Rect::NOTHING; 3]; 2]; 4],
        sections: [Rect::NOTHING; 4],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(ui, ("OTT", compact), height, &mut enabled, &mut collapsed, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for (section, name) in ["Low", "Mid", "High", "Out"].into_iter().enumerate() {
                let rect = ui.push_id(section, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let column_width = knobs::CHOICE_SIZE;
                        let padding = (UNIT_STRIP_WIDTH - column_width) * 0.5;
                        let width = if section < 3 { 2.0 * column_width + 3.0 * padding } else { UNIT_STRIP_WIDTH };
                        ui.set_width(width - 2.0 * f32::from(panel.margin));
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        if !compact {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = padding;
                                ui.add_space(padding - f32::from(panel.margin));
                                for column in 0..if section < 3 { 2 } else { 1 } {
                                    ui.push_id(column, |ui| ui.allocate_ui_with_layout(Vec2::new(column_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                        ui.set_width(column_width);
                                        for index in 0..3 { bounds.controls[section][column][index] = row(ui, settings, section, column, index); }
                                    }));
                                }
                            });
                        }
                        rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                            if section < 3 {
                                let response = rows::power_label(ui, name, "Toggle band compression", settings.bands[section].enabled).on_hover_text("Off bypasses compression; band input/output gains remain active.");
                                bounds.bands[section] = response.rect;
                                if response.clicked() { settings.bands[section].enabled = !settings.bands[section].enabled; }
                            } else { rows::unit_label(ui, name); }
                        });
                    });
                })).inner.response.rect;
                bounds.sections[section] = rect;
                if section > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
                if !compact && section < 3 {
                    let rail = meters::unit_monitor_rail(rect, panel.margin);
                    bounds.meters[section] = rail;
                    meters::gain_change(ui, rail, name, gains[section]);
                }
            }
        });
    });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
