use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels,
    layout::UNIT_STRIP_WIDTH,
    meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{CompressorDetection, CompressorSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub output_meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 3],
    pub sections: [Rect; 3],
}

fn row(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    column: usize,
    row: usize,
    reduction_db: meters::GainReduction<'_>,
    markers: &[Option<knobs::ModulationMarker>; 7],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        if (column, row) == (2, 2) {
            return meters::gain_reduction(ui, reduction_db);
        }
        if (column, row) == (0, 2) {
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
            return response;
        }
        let (index, value) = match (column, row) {
            (0, 0) => (0, &mut s.threshold_db),
            (0, 1) => (1, &mut s.ratio),
            (1, 0) => (2, &mut s.attack_ms),
            (1, 1) => (3, &mut s.release_ms),
            (1, 2) => (4, &mut s.knee_db),
            (2, 0) => (5, &mut s.level_db),
            (2, 1) => (6, &mut s.dry_wet),
            _ => unreachable!(),
        };
        let mut spec = parameter_knobs::destination(Parameter::COMPRESSOR[index]);
        if index == 5 {
            spec = spec.named("Compressor output level");
        }
        ui.add(Knob::new(value, spec).modulated(markers[index].as_ref(), &mut s.modulation_open))
    })
    .rect
}

pub fn show(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 7],
    reduction_db: meters::GainReduction<'_>,
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        output_meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[Rect::NOTHING; 3]; 3],
        sections: [Rect::NOTHING; 3],
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        for (column, name) in ["Comp", "Response", "Out"].into_iter().enumerate() {
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
                                            row(ui, settings, column, index, reduction_db, markers);
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
            if !compact && column == 2 {
                bounds.output_meter =
                    meters::effect_output_level(ui, rect, PANEL_MARGIN, level, settings.enabled);
            }
            if column > 0 {
                panels::unit_divider(ui, rect.left(), rect.y_range().into());
            }
        }
    });
    bounds
}
