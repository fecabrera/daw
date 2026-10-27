use crate::lfo_controls::choice as float_choice;
use crate::parameter_knobs;
use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    layout as panels, meters, rows, theme,
};
use daw_unit_core::{AutoFilterSettings, FilterShape, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

use crate::layout::PANEL_MARGIN;
pub struct Bounds {
    pub meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 4],
    pub lfo: Rect,
    pub sections: [Rect; 3],
}
pub fn filter_diagram(value: f32) -> Vec<Vec2> {
    let shape = FilterShape::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let band = (-((x - 0.5) * 5.0).powi(2)).exp();
            let level = match shape {
                FilterShape::LowPass => low,
                FilterShape::HighPass => 1.0 - low,
                FilterShape::BandPass => band,
                FilterShape::Notch => 1.0 - band,
            };
            Vec2::new(x - 0.5, 0.5 - level)
        })
        .collect()
}

fn row(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    strip: usize,
    index: usize,
    markers: &[Option<knobs::ModulationMarker>; 7],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        match (strip, index) {
            (0, 0) => float_choice(
                ui,
                &mut s.shape,
                FilterShape::ALL,
                KnobSpec::new("Filter shape", 0.0..=3.0, 0.0, 1.0)
                    .stepped(1.0)
                    .cyclic()
                    .diagrams(filter_diagram)
                    .format(|v| FilterShape::ALL[v as usize].name().into()),
            ),
            (0, 1) => ui.add(
                Knob::new(
                    &mut s.frequency_hz,
                    parameter_knobs::destination(Parameter::Frequency),
                )
                .modulated(markers[0].as_ref(), &mut s.modulation_open),
            ),
            (0, 2) => ui.add(
                Knob::new(&mut s.q, parameter_knobs::destination(Parameter::Resonance))
                    .modulated(markers[1].as_ref(), &mut s.modulation_open),
            ),
            (1, 0) => crate::lfo_controls::wave(ui, &mut s.wave),
            (1, 1) => crate::lfo_controls::mode(ui, &mut s.rate_mode),
            (1, 2) => crate::lfo_controls::rate(ui, s.rate_mode, &mut s.rate_hz, &mut s.note_rate),
            (2, 0) => ui.add(
                Knob::new(
                    &mut s.strength_octaves,
                    parameter_knobs::destination(Parameter::AutoFilterStrength),
                )
                .modulated(markers[4].as_ref(), &mut s.modulation_open),
            ),
            (2, 1) => ui.add(
                Knob::new(
                    &mut s.sync_phase_degrees,
                    parameter_knobs::destination(Parameter::AutoFilterSyncPhase),
                )
                .modulated(markers[5].as_ref(), &mut s.modulation_open),
            ),
            (2, 2) => ui.add(
                Knob::new(
                    &mut s.stereo_phase_degrees,
                    parameter_knobs::destination(Parameter::AutoFilterStereoPhase),
                )
                .modulated(markers[6].as_ref(), &mut s.modulation_open),
            ),
            (3, 0) => ui.add(
                Knob::new(
                    &mut s.level_db,
                    parameter_knobs::destination(Parameter::Level),
                )
                .modulated(markers[2].as_ref(), &mut s.modulation_open),
            ),
            (3, 1) => ui.add(
                Knob::new(
                    &mut s.dry_wet,
                    parameter_knobs::destination(Parameter::DryWet),
                )
                .modulated(markers[3].as_ref(), &mut s.modulation_open),
            ),
            _ => ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover()),
        }
    })
    .rect
}

fn controls(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    strip: usize,
    bounds: &mut Bounds,
    markers: &[Option<knobs::ModulationMarker>; 7],
) {
    for index in 0..3 {
        bounds.knobs[strip][index] = row(ui, settings, strip, index, markers);
    }
}

fn footer(ui: &mut Ui, name: &str, compact: bool, body_height: f32) {
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
}
pub fn show(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    dimensions: (bool, f32),
    markers: &[Option<knobs::ModulationMarker>; 7],
    internal_lfo: bool,
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let (compact, body_height) = dimensions;
    let mut bounds = Bounds {
        meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 4],
        lfo: Rect::NOTHING,
        sections: [Rect::NOTHING; 3],
    };
    ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for section in (0..3).filter(|section| internal_lfo || *section != 1) {
                let rect = ui.push_id(section, |ui| egui::Frame::new().inner_margin(PANEL_MARGIN).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let section_width = panels::unit_columns_width(if section == 1 { 2 } else { 1 });
                        let width = section_width - 2.0 * f32::from(PANEL_MARGIN);
                        ui.set_width(width);
                        ui.set_min_height(body_height);
                        if !compact {
                            if section == 1 {
                                panels::unit_columns(ui, 2, |ui, column| {
                                    controls(ui, settings, column + 1, &mut bounds, markers);
                                });
                            } else { controls(ui, settings, if section == 0 { 0 } else { 3 }, &mut bounds, markers); }
                        }
                        if section == 1 {
                            rows::centered(ui, if compact { body_height } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                                let response = rows::power_label(ui, "LFO", "Toggle filter modulation", settings.lfo_enabled).on_hover_text("Off keeps the static filter active at its Frequency and Resonance settings.");
                                bounds.lfo = response.rect;
                                if response.clicked() { settings.lfo_enabled = !settings.lfo_enabled; }
                            });
                        } else { footer(ui, if section == 0 { "Cutoff" } else { "Out" }, compact, body_height); }
                    });
                })).inner.response.rect;
                bounds.sections[section] = rect;
                if section == 2 && !compact {
                    let rail = meters::effect_output_level(ui, rect, PANEL_MARGIN, level, settings.enabled);
                     {bounds.meter=rail;}
                     let _=rail;
                }
                if section > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
            }

        });
    bounds
}
