use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels, meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{ChorusSettings, ModulationParameter as P};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub power: Rect,
    pub sections: [Rect; 3],
    pub controls: [[Rect; 3]; 3],
    pub meter: Rect,
    pub rate_mode: Rect,
}

fn parameter(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    index: usize,
    marker: Option<&knobs::ModulationMarker>,
) -> egui::Response {
    let mut value = s.values()[index];
    let response = ui.add(
        Knob::new(
            &mut value,
            parameter_knobs::destination(P::CHORUS[index]).named(P::CHORUS[index].label()),
        )
        .modulated(marker, &mut s.modulation_open),
    );
    if response.changed() {
        match index {
            0 => s.delay_ms = value,
            1 => s.depth = value,
            2 => s.feedback = value,
            3 => s.rate_hz = value,
            4 => s.stereo_phase_degrees = value,
            5 => s.width = value,
            6 => s.level_db = value,
            _ => s.dry_wet = value,
        }
    }
    response
}

pub fn show(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    compact: bool,
    body_height: f32,
    markers: &[Option<knobs::ModulationMarker>; 8],
    level: meters::OutputLevel<'_>,
    tempo: f32,
) -> Bounds {
    let mut b = Bounds {
        power: Rect::NOTHING,

        sections: [Rect::NOTHING; 3],

        controls: [[Rect::NOTHING; 3]; 3],

        meter: Rect::NOTHING,

        rate_mode: Rect::NOTHING,
    };
    ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));

                for (section, name) in ["Chorus", "Motion", "Out"].into_iter().enumerate() {
                    let width = panels::unit_columns_width(if section == 1 { 2 } else { 1 });
                    let strip = ui.push_id(section, |ui| {
                        panels::unit_strip_with_width(ui, body_height, width, egui::UiBuilder::new(), |ui| {
                            if !compact {
                                if section == 1 {
                                    panels::unit_columns(ui, 2, |ui, column| {
                                                    for row in 0..2 {
                                                        let response = knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| match (column, row) {
                                                            (0, 0) => crate::lfo_controls::mode(ui, &mut s.rate_mode),
                                                            (0, 1) => crate::lfo_controls::modulated_rate_with_spec(ui, s.rate_mode, &mut s.rate_hz, &mut s.note_rate, crate::lfo_controls::RateModulation {
                                                                marker: markers[3].as_ref(), open: &mut s.modulation_open, tempo,
                                                            }, parameter_knobs::destination(P::ChorusRate)),
                                                            (1, 0) => parameter(ui, s, 4, markers[4].as_ref()),
                                                            _ => {
                                                                let mut voices = f32::from(s.voices);
                                                                let r = ui.add(Knob::new(&mut voices, KnobSpec::new("Voices", 1.0..=4.0, 2.0, 1.0).stepped(1.0)
                                                                    .value_label(|v| format!("{v:.0}")).format(|v| format!("{v:.0} voices per channel")).endpoint_labels("1", "4")));
                                                                if r.changed() { s.voices = voices as u8; }
                                                                r
                                                            }
                                                        });
                                                         {
                                                            match (column, row) {
                                                                (0, 0) => b.rate_mode = response.rect,
                                                                (0, 1) => b.controls[1][0] = response.rect,
                                                                (1, 0) => b.controls[1][1] = response.rect,
                                                                _ => b.controls[1][2] = response.rect,
                                                            }
                                                        }
                                                         let _ = response;
                                                    }
                                    });
                                    // Reserve the third shared row so all strip footers align.
                                    ui.allocate_exact_size(Vec2::new(ui.available_width(), knobs::CHOICE_SIZE), egui::Sense::hover());
                                } else {
                                    for (row, index) in if section == 0 { [0,1,2] } else { [5,6,7] }.into_iter().enumerate() {
                                        let response = knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| parameter(ui, s, index, markers[index].as_ref()));
                                         { b.controls[section][row] = response.rect; }
                                         let _ = (row, response);
                                    }
                                }
                            }
                            rows::centered(ui, if compact { body_height } else { icons::COMPACT_BUTTON_SIZE }, |ui| rows::unit_label(ui, name));
                        })
                    }).inner.response.rect;
                    if section > 0 { panels::unit_divider(ui, strip.left(), strip.y_range().into()); }
                    if section == 2 && !compact {
                        let rail = meters::effect_output_level(ui, strip, PANEL_MARGIN, level, s.enabled);
                         { b.meter = rail; }
                         let _ = rail;
                    }
                     { b.sections[section] = strip; }

                }

            });
    b
}
