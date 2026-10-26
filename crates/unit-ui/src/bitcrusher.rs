//! Bitcrusher body shared by the DAW shell and plugin editors.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout::*,
    meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{BitcrusherSettings, ModulationParameter as P};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    2.0 * UNIT_STRIP_WIDTH,
    crate::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub meter: Rect,
    pub sections: [Rect; 2],
    /// Spatial order: three left controls, then the two output controls.
    pub knobs: [Rect; 5],
    /// Host parameter order: Level, Drive, then the two unit choices and Dry/Wet.
    pub controls: [Option<Response>; 5],
    pub rect: Rect,
}
pub fn show(
    ui: &mut Ui,
    settings: &mut BitcrusherSettings,
    compact: bool,
    compact_height: f32,
    markers: &[Option<ModulationMarker>; 5],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        meter: Rect::NOTHING,
        sections: [Rect::NOTHING; 2],
        knobs: [Rect::NOTHING; 5],
        controls: std::array::from_fn(|_| None),
        rect: Rect::NOTHING,
    };
    let result = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        for (column, name) in ["Crush", "Output"].into_iter().enumerate() {
            let strip = ui
                .push_id(column, |ui| {
                    egui::Frame::new()
                        .inner_margin(PANEL_MARGIN)
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                ui.set_min_height(if compact {
                                    compact_height
                                } else {
                                    crate::layout::unit_control_height()
                                });
                                if !compact {
                                    for row in 0..3 {
                                        let index = column * 3 + row;
                                        if index == 5 {
                                            knobs::row(
                                                ui,
                                                settings,
                                                knobs::CHOICE_SIZE,
                                                |ui, _| {
                                                    ui.allocate_response(
                                                        Vec2::splat(knobs::CHOICE_SIZE),
                                                        egui::Sense::hover(),
                                                    )
                                                },
                                            );
                                            continue;
                                        }

                                        let response = knobs::row(
                                            ui,
                                            settings,
                                            knobs::CHOICE_SIZE,
                                            |ui, s| {
                                                let mut value = match index {
                                                    0 => s.drive_db,
                                                    1 => f32::from(s.resolution_bits),
                                                    2 => f32::from(s.downsampling),
                                                    3 => s.level_db,
                                                    _ => s.dry_wet,
                                                };
                                                let response = ui.add(
                                                    Knob::new(
                                                        &mut value,
                                                        parameter_knobs::destination(
                                                            P::BITCRUSHER[index],
                                                        ),
                                                    )
                                                    .modulated(
                                                        markers[index].as_ref(),
                                                        &mut s.modulation_open,
                                                    ),
                                                );
                                                if response.changed() {
                                                    match index {
                                                        0 => s.drive_db = value,
                                                        1 => s.resolution_bits = value as u8,
                                                        2 => s.downsampling = value as u8,
                                                        3 => s.level_db = value,
                                                        _ => s.dry_wet = value,
                                                    }
                                                }
                                                response
                                            },
                                        );
                                        let slot = [1, 2, 3, 0, 4][index];

                                        bounds.knobs[index] = response.rect;
                                        bounds.controls[slot] = Some(response);
                                    }
                                }
                                rows::centered(
                                    ui,
                                    if compact {
                                        compact_height
                                    } else {
                                        COMPACT_BUTTON_SIZE
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
            bounds.sections[column] = strip;
            if column == 1 && !compact {
                bounds.meter =
                    meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
            }
            if column > 0 {
                crate::drive::divider(ui, strip.left(), strip.y_range());
            }
        }
    });
    bounds.rect = result.response.rect;
    bounds
}
