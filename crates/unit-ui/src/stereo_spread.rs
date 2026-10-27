use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels,
    layout::UNIT_STRIP_WIDTH,
    parameter_knobs, rows, theme,
};
use daw_unit_core::{ModulationParameter as Parameter, StereoSpreadSettings};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 5],
    pub sections: [Rect; 3],
}

pub fn show(
    ui: &mut Ui,
    settings: &mut StereoSpreadSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 4],
) -> Bounds {
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [Rect::NOTHING; 5],
        sections: [Rect::NOTHING; 3],
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        for (column, name) in ["Lower", "Upper", "Spread"].into_iter().enumerate() {
            let rect = ui
                .push_id(column, |ui| {
                    egui::Frame::new()
                        .inner_margin(PANEL_MARGIN)
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                ui.set_min_height(body_height);
                                if !compact {
                                    for row in 0..3 {
                                        knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
                                            if (column, row) == (2, 0) {
                                                let mut order = f32::from(s.order);
                                                let response = ui.add(Knob::new(
                                                    &mut order,
                                                    KnobSpec::new("Order", 2.0..=12.0, 8.0, 1.0)
                                                        .stepped(1.0)
                                                        .reference_step(1.0)
                                                        .value_label(|v| format!("{v:.0}"))
                                                        .format(|v| format!("{v:.0} bands"))
                                                        .endpoint_labels("2", "12"),
                                                ));
                                                if response.changed() {
                                                    s.order = order as u8;
                                                }
                                                bounds.controls[4] = response.rect;
                                                return response.on_hover_text(
                                                    "Order: number of alternating frequency bands.",
                                                );
                                            }
                                            let (index, control, value) = match (column, row) {
                                                (0, 0) => (0, 0, &mut s.lower_intensity),
                                                (0, 1) => (2, 1, &mut s.lower_frequency_hz),
                                                (1, 0) => (1, 2, &mut s.upper_intensity),
                                                (1, 1) => (3, 3, &mut s.upper_frequency_hz),
                                                _ => {
                                                    return ui.allocate_response(
                                                        Vec2::splat(knobs::CHOICE_SIZE),
                                                        egui::Sense::hover(),
                                                    );
                                                }
                                            };
                                            let response = ui.add(
                                                Knob::new(
                                                    value,
                                                    parameter_knobs::destination(
                                                        Parameter::STEREO_SPREAD[index],
                                                    ),
                                                )
                                                .modulated(
                                                    markers[index].as_ref(),
                                                    &mut s.modulation_open,
                                                ),
                                            );
                                            if response.changed() {
                                                if index == 2 {
                                                    s.lower_frequency_hz = s
                                                        .lower_frequency_hz
                                                        .min(s.upper_frequency_hz - 1.0);
                                                }
                                                if index == 3 {
                                                    s.upper_frequency_hz = s
                                                        .upper_frequency_hz
                                                        .max(s.lower_frequency_hz + 1.0);
                                                }
                                            }
                                            bounds.controls[control] = response.rect;
                                            response
                                        });
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
            if column > 0 {
                panels::unit_divider(ui, rect.left(), rect.y_range().into());
            }
        }
    });
    bounds
}
