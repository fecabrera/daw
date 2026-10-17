use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{LimiterMode, LimiterRouting, LimiterSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 2],
    pub sections: [Rect; 2],
    pub meter: Rect,
}
fn db(value: f32) -> String {
    format!("{value:.1} dB")
}
fn time(value: f32) -> String {
    if value < 10.0 {
        format!("{value:.1} ms")
    } else {
        format!("{value:.0} ms")
    }
}
fn row(ui: &mut Ui, settings: &mut LimiterSettings, column: usize, row: usize) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        match (column, row) {
            (0, 0) => ui.add(Knob::new(&mut s.input_db,
                KnobSpec::new("Limiter input", -24.0..=24.0, 0.0, 0.1)
                    .reference_step(6.0).value_label(db).format(db).endpoint_labels("−24", "+24"))),
            (0, 1) => ui.add(Knob::new(&mut s.release_ms,
                KnobSpec::new("Release", 1.0..=2000.0, 100.0, 0.1)
                    .scale(|v| v.ln()/2000_f32.ln(), |p| 2000_f32.powf(p), |v,d| v*2_f32.powf(d/24.0))
                    .value_label(time).format(time).endpoint_labels("1", "2k"))),
            (0, 2) => ui.add(Knob::new(&mut s.lookahead_ms,
                KnobSpec::new("Lookahead", 0.0..=10.0, 1.0, 0.1)
                    .value_label(time).format(time).endpoint_labels("0", "10"))),
            (1, 0) => ui.add(Knob::new(&mut s.ceiling_db,
                KnobSpec::new("Ceiling", -24.0..=0.0, -1.0, 0.1)
                    .value_label(db).format(db).endpoint_labels("−24", "0"))),
            (1, 1) => {
                let mut value = f32::from(s.mode == LimiterMode::TruePeak);
                let response = ui.add(Knob::new(&mut value,
                    KnobSpec::new("Limiter mode", 0.0..=1.0, 0.0, 1.0)
                        .two_way_labels("Peak", "True")
                        .format(|v| if v == 0.0 { "Peak".into() } else { "True Peak".into() })));
                if response.changed() { s.mode = if value == 0.0 { LimiterMode::Peak } else { LimiterMode::TruePeak }; }
                response.on_hover_text("Peak: sample ceiling. True Peak: eight-times oversampled reconstruction guard.")
            }
            (1, 2) => {
                let mut value = f32::from(s.routing == LimiterRouting::MidSide);
                let response = ui.add(Knob::new(&mut value,
                    KnobSpec::new("Routing", 0.0..=1.0, 0.0, 1.0).two_way_labels("L/R", "M/S")
                        .format(|v| if v == 0.0 { "Linked left/right".into() } else { "Independent mid/side".into() })));
                if response.changed() { s.routing = if value == 0.0 { LimiterRouting::LeftRight } else { LimiterRouting::MidSide }; }
                response.on_hover_text("L/R: linked stereo limiting. M/S: independent Mid/Side limiting, then a linked output ceiling.")
            }
            _ => unreachable!(),
        }
    }).rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
    compact: bool,
    height: f32,
    reduction_db: f32,
) -> (Bounds, egui::Response) {
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[Rect::NOTHING; 3]; 2],
        sections: [Rect::NOTHING; 2],
        meter: Rect::NOTHING,
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Limiter", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Limit", "Out"].into_iter().enumerate() {
                    let rect = ui
                        .push_id(column, |ui| {
                            egui::Frame::new()
                                .inner_margin(panel.margin)
                                .show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_width(
                                                UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin),
                                            );
                                            ui.set_min_height(crate::envelope_panel::body_height(
                                                compact,
                                            ));
                                            if !compact {
                                                for index in 0..3 {
                                                    bounds.controls[column][index] =
                                                        row(ui, settings, column, index);
                                                }
                                            }
                                            rows::centered(
                                                ui,
                                                if compact {
                                                    crate::envelope_panel::body_height(true)
                                                } else {
                                                    icons::COMPACT_BUTTON_SIZE
                                                },
                                                |ui| {
                                                    rows::unit_label(ui, name);
                                                },
                                            );
                                        },
                                    );
                                })
                        })
                        .inner
                        .response
                        .rect;
                    bounds.sections[column] = rect;
                    if !compact && column == 0 {
                        let rail = meters::unit_monitor_rail(
                            rect.with_max_x(rect.left() + 2.0 * UNIT_STRIP_WIDTH),
                            panel.margin,
                        );
                        bounds.meter = rail;
                        meters::gain_reduction_rail(ui, rail, reduction_db);
                    }
                    if column > 0 {
                        panels::unit_divider(ui, rect.left(), rect.y_range().into());
                    }
                }
            });
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
