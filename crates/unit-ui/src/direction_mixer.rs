use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout::UNIT_STRIP_WIDTH,
    parameter_knobs, rows, theme,
};
use daw_unit_core::{
    DirectionMixerInput, DirectionMixerSettings, ModulationParameter as Parameter,
};
use egui::{Rect, Ui};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub section: Rect,
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 3],
}

pub fn show(
    ui: &mut Ui,
    settings: &mut DirectionMixerSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 2],
) -> Bounds {
    let mut bounds = Bounds {
        section: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [Rect::NOTHING; 3],
    };
    ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                let _strip = egui::Frame::new().inner_margin(PANEL_MARGIN).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.set_min_height(body_height);
                        if !compact {
                            let mut mode = f32::from(settings.input == DirectionMixerInput::MidSide);
                            let response = knobs::row(ui, &mut mode, knobs::CHOICE_SIZE, |ui, value| ui.add(Knob::new(value,
                                KnobSpec::new("Input format", 0.0..=1.0, 0.0, 1.0).two_way_labels("L/R", "M/S")
                                    .format(|v| if v == 0.0 { "Left/right stereo".into() } else { "Mid/side recording".into() }))));
                            bounds.controls[0] = response.rect;
                            if response.changed() { settings.input = if mode == 0.0 { DirectionMixerInput::LeftRight } else { DirectionMixerInput::MidSide }; }
                            response.on_hover_text("L/R: ordinary stereo. M/S: left input is Mid, right input is Side; output is decoded stereo.");
                            for (i, marker) in markers.iter().enumerate() {
                                let value = if i == 0 { &mut settings.direction_degrees } else { &mut settings.spread };
                                bounds.controls[i+1] = knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, value| ui.add(
                                    Knob::new(value, parameter_knobs::destination(Parameter::DIRECTION_MIXER[i]))
                                        .modulated(marker.as_ref(), &mut settings.modulation_open))).rect;
                            }
                        }
                        rows::centered(ui, if compact { body_height } else { icons::COMPACT_BUTTON_SIZE }, |ui| { rows::unit_label(ui, "Stereo"); });
                    });
                }).response.rect;
 bounds.section = _strip;

            });
    bounds
}
