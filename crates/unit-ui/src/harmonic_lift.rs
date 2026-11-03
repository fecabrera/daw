//! Headerless nine-strip Harmonic Lift body shared by DAW and plugin editors.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout as panels,
    menus::Dropdown,
    meters, parameter_knobs, rows,
};
use daw_unit_core::{
    HARMONIC_BAND_NAMES, HarmonicLiftSettings, HarmonicQuality, ModulationParameter as P,
};
use egui::{Rect, Ui, Vec2};
pub struct Bounds {
    pub sections: [Rect; 9],
    /// Plugin slot order: input, quality, six Power/Frequency/Gain triplets,
    /// Drive, Push, Limit Level, output Level and Dry/Wet.
    pub controls: [Rect; 25],
    pub output_meter: Rect,
    pub reduction: Rect,
    pub header: Rect,
    pub power: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            sections: [Rect::NOTHING; 9],
            controls: [Rect::NOTHING; 25],
            output_meter: Rect::NOTHING,
            reduction: Rect::NOTHING,
            header: Rect::NOTHING,
            power: Rect::NOTHING,
        }
    }
}
pub struct Controls<'a> {
    pub markers: &'a [Option<ModulationMarker>; 6],
    pub bands: &'a [[Option<ModulationMarker>; 2]; 6],
    pub reduction: meters::GainReduction<'a>,
    pub level: meters::OutputLevel<'a>,
}
pub fn show(
    ui: &mut Ui,
    s: &mut HarmonicLiftSettings,
    dimensions: (bool, f32),
    c: Controls<'_>,
) -> Bounds {
    let (compact, height) = dimensions;
    let mut b = Bounds::default();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for column in 0..9 {
            b.sections[column] =
                panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt(column), |ui| {
                    let start = ui.cursor().top();
                    if !compact {
                        if column == 0 {
                            b.controls[0] = knobs::row(ui, s, knobs::CHOICE_SIZE, |ui, s| {
                                ui.add(
                                    Knob::new(
                                        &mut s.input_db,
                                        parameter_knobs::destination(P::HarmonicInput),
                                    )
                                    .modulated(c.markers[0].as_ref(), &mut s.modulation_open),
                                )
                            })
                            .rect;
                            let response = Dropdown::from_id_salt("quality")
                                .small_items()
                                .selected_text(
                                    egui::RichText::new(match s.quality {
                                        HarmonicQuality::Normal => "Normal",
                                        HarmonicQuality::Hq => "HQ 2×",
                                    })
                                    .size(panels::HEADER_FONT_SIZE),
                                )
                                .truncate()
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut s.quality,
                                        HarmonicQuality::Normal,
                                        "Normal",
                                    );
                                    ui.selectable_value(
                                        &mut s.quality,
                                        HarmonicQuality::Hq,
                                        "HQ 2×",
                                    );
                                })
                                .response
                                .on_hover_text("Processing quality");
                            crate::control_capture::record(ui, &response);
                            b.controls[1] = response.rect;
                        } else if column < 7 {
                            let i = column - 1;
                            for row in 0..2 {
                                b.controls[3 + i * 3 + row] =
                                    knobs::row(ui, s, knobs::CHOICE_SIZE, |ui, s| {
                                        let spec = if row == 0 {
                                            parameter_knobs::harmonic_frequency(i)
                                        } else {
                                            parameter_knobs::harmonic_gain()
                                        };
                                        let value = if row == 0 {
                                            &mut s.bands[i].frequency_hz
                                        } else {
                                            &mut s.bands[i].gain_db
                                        };
                                        ui.add(Knob::new(value, spec).modulated(
                                            c.bands[i][row].as_ref(),
                                            &mut s.modulation_open,
                                        ))
                                    })
                                    .rect;
                            }
                        } else {
                            for row in 0..3 {
                                if column == 8 && row == 2 {
                                    break;
                                }
                                let (index, p) = if column == 7 {
                                    (
                                        1 + row,
                                        [P::HarmonicDrive, P::HarmonicPush, P::HarmonicLimit][row],
                                    )
                                } else {
                                    (4 + row, [P::Level, P::DryWet][row])
                                };
                                let slot = if column == 7 { 20 + row } else { 23 + row };
                                b.controls[slot] =
                                    knobs::row(ui, s, knobs::CHOICE_SIZE, |ui, s| {
                                        let value = match index {
                                            1 => &mut s.drive,
                                            2 => &mut s.push,
                                            3 => &mut s.limit_db,
                                            4 => &mut s.level_db,
                                            _ => &mut s.dry_wet,
                                        };
                                        ui.add(
                                            Knob::new(value, parameter_knobs::destination(p))
                                                .modulated(
                                                    c.markers[index].as_ref(),
                                                    &mut s.modulation_open,
                                                ),
                                        )
                                    })
                                    .rect;
                            }
                        }
                        ui.add_space(
                            (height - panels::COMPACT_BUTTON_SIZE - (ui.cursor().top() - start))
                                .max(0.0),
                        );
                    }
                    rows::centered(
                        ui,
                        if compact {
                            height
                        } else {
                            panels::COMPACT_BUTTON_SIZE
                        },
                        |ui| {
                            if (1..7).contains(&column) {
                                let i = column - 1;
                                let r = rows::power_label(
                                    ui,
                                    HARMONIC_BAND_NAMES[i],
                                    &format!("Toggle {} branch", HARMONIC_BAND_NAMES[i]),
                                    s.bands[i].enabled,
                                );
                                if r.clicked() {
                                    s.bands[i].enabled = !s.bands[i].enabled;
                                }
                                b.controls[2 + i * 3] = r.rect;
                            } else {
                                rows::unit_label(
                                    ui,
                                    match column {
                                        0 => "Input",
                                        7 => "Maximizer",
                                        _ => "Out",
                                    },
                                );
                            }
                        },
                    );
                })
                .response
                .rect;
            if column > 0 {
                panels::unit_divider(
                    ui,
                    b.sections[column].left(),
                    b.sections[column].y_range().into(),
                );
            }
        }
        if !compact {
            b.output_meter = meters::effect_output_level(
                ui,
                b.sections[8],
                panels::PANEL_MARGIN,
                c.level,
                s.enabled,
            );
            // Reduction rail occupies the same top rail as other dynamics units.
            let rect = b.sections[7];
            let rail = Rect::from_min_size(
                rect.min + Vec2::new(10.0, 2.0),
                Vec2::new(rect.width() - 20.0, 4.0),
            );
            b.reduction =
                meters::gain_reduction_rail(ui, rail, c.reduction.for_unit(s.enabled)).rect;
        }
    });
    b
}
