use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels,
    layout::UNIT_STRIP_WIDTH,
    meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{
    LimiterMode, LimiterRouting, LimiterSettings, ModulationParameter as Parameter,
};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub output_meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 2],
    pub sections: [Rect; 2],
    pub meter: Rect,
}

fn row(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
    column: usize,
    row: usize,
    markers: &[Option<knobs::ModulationMarker>; 4],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        let target = match (column, row) {
            (0, 0) => Some((0, &mut s.input_db)),
            (0, 1) => Some((1, &mut s.release_ms)),
            (0, 2) => Some((2, &mut s.lookahead_ms)),
            (1, 0) => Some((3, &mut s.ceiling_db)),
            _ => None,
        };
        if let Some((index, value)) = target {
            return ui.add(Knob::new(value, parameter_knobs::destination(Parameter::LIMITER[index]))
                .modulated(markers[index].as_ref(), &mut s.modulation_open));
        }
        match (column, row) {
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

pub fn show(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 4],
    reduction_db: meters::GainReduction<'_>,
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        output_meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[Rect::NOTHING; 3]; 2],
        sections: [Rect::NOTHING; 2],
        meter: Rect::NOTHING,
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        for (column, name) in ["Limit", "Out"].into_iter().enumerate() {
            let rect = ui
                .push_id(column, |ui| {
                    egui::Frame::new()
                        .inner_margin(PANEL_MARGIN)
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                ui.set_min_height(body_height);
                                if !compact {
                                    for index in 0..3 {
                                        bounds.controls[column][index] =
                                            row(ui, settings, column, index, markers);
                                    }
                                }
                                rows::centered(
                                    ui,
                                    if compact {
                                        body_height
                                    } else {
                                        icons::COMPACT_BUTTON_SIZE
                                    },
                                    |ui| {
                                        rows::unit_label(ui, name);
                                    },
                                );
                            });
                        })
                })
                .inner
                .response
                .rect;
            bounds.sections[column] = rect;
            if !compact && column == 1 {
                bounds.output_meter =
                    meters::effect_output_level(ui, rect, PANEL_MARGIN, level, settings.enabled);
            }
            if !compact && column == 0 {
                let rail = meters::unit_monitor_rail(rect, PANEL_MARGIN);
                bounds.meter = rail;
                meters::gain_reduction_rail(ui, rail, reduction_db);
            }
            if column > 0 {
                panels::unit_divider(ui, rect.left(), rect.y_range().into());
            }
        }
    });
    bounds
}
