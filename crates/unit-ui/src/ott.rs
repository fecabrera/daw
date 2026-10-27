use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels, meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{CompressorDetection, ModulationParameter as Parameter, OttSettings};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub output_meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub controls: [[[Rect; 3]; 2]; 4],
    pub sections: [Rect; 4],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}

pub fn parameter_index(section: usize, column: usize, row: usize) -> Option<usize> {
    match (section, column, row) {
        (0, 0, 0) => Some(0),
        (2, 0, 0) => Some(1),
        (band, 1, 0) if band < 3 => Some(2 + 5 * band),
        (band, column, 1) if band < 3 => Some(3 + 5 * band + column),
        (band, column, 2) if band < 3 => Some(5 + 5 * band + column),
        (3, 0, 0) => Some(17),
        (3, 0, 1) => Some(18),
        _ => None,
    }
}

fn row(
    ui: &mut Ui,
    settings: &mut OttSettings,
    section: usize,
    column: usize,
    row: usize,
    markers: &[Option<knobs::ModulationMarker>; 19],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        if let Some(index) = parameter_index(section, column, row) {
            let value = match index {
                0 => &mut s.low_frequency_hz,
                1 => &mut s.high_frequency_hz,
                17 => &mut s.level_db,
                18 => &mut s.dry_wet,
                _ => {
                    let band = &mut s.bands[(index - 2) / 5];
                    match (index - 2) % 5 {
                        0 => &mut band.depth,
                        1 => &mut band.input_db,
                        2 => &mut band.output_db,
                        3 => &mut band.attack_ms,
                        4 => &mut band.release_ms,
                        _ => unreachable!(),
                    }
                }
            };
            let response = ui.add(
                Knob::new(
                    value,
                    if index == 17 {
                        parameter_knobs::destination(Parameter::Level).named("OTT output level")
                    } else {
                        parameter_knobs::destination(Parameter::OTT[index])
                    },
                )
                .modulated(markers[index].as_ref(), &mut s.modulation_open),
            );
            if response.changed() {
                if index == 0 {
                    s.low_frequency_hz = s.low_frequency_hz.min(s.high_frequency_hz - 1.0);
                }
                if index == 1 {
                    s.high_frequency_hz = s.high_frequency_hz.max(s.low_frequency_hz + 1.0);
                }
            }
            return response;
        }
        if section < 3 {
            return ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover());
        }
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
    })
    .rect
}

pub fn show(
    ui: &mut Ui,
    settings: &mut OttSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 19],
    gains: meters::BandGainChange<'_>,
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        output_meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[[Rect::NOTHING; 3]; 2]; 4],
        sections: [Rect::NOTHING; 4],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for (section, name) in ["Low", "Mid", "High", "Out"].into_iter().enumerate() {
                let rect = ui.push_id(section, |ui| egui::Frame::new().inner_margin(PANEL_MARGIN).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let width = panels::unit_columns_width(if section < 3 { 2 } else { 1 });
                        ui.set_width(width - 2.0 * f32::from(PANEL_MARGIN));
                        ui.set_min_height(body_height);
                        if !compact {
                            panels::unit_columns(ui, if section < 3 { 2 } else { 1 }, |ui, column| {
                                for index in 0..3 { bounds.controls[section][column][index] = row(ui, settings, section, column, index, markers); }
                            });
                        }
                        rows::centered(ui, if compact { body_height } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                            if section < 3 {
                                let response = rows::power_label(ui, name, "Toggle band compression", settings.bands[section].enabled).on_hover_text("Off bypasses compression; band input/output gains remain active.");
                                bounds.bands[section] = response.rect;
                                if response.clicked() { settings.bands[section].enabled = !settings.bands[section].enabled; }
                            } else { rows::unit_label(ui, name); }
                        });
                    });
                })).inner.response.rect;
                bounds.sections[section] = rect;
                    if !compact && section == 3 {
                        bounds.output_meter = meters::effect_output_level(ui, rect, PANEL_MARGIN, level, settings.enabled);
                    }
                if section > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
                if !compact && section < 3 {
                    let rail = meters::unit_monitor_rail(rect, PANEL_MARGIN);
                    bounds.meters[section] = rail;
                    meters::gain_change(ui, rail, name, gains.band(section), settings.bands[section].enabled);
                }
            }

        });
    bounds
}
