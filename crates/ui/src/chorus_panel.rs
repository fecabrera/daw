use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters, panels, parameter_knobs, rows, theme,
};
use daw_core::{ChorusSettings, ModulationParameter as P};
use egui::{Rect, Ui, Vec2};
pub(super) struct Bounds {
    pub power: Rect,
    #[cfg(test)]
    pub sections: [Rect; 3],
    #[cfg(test)]
    pub controls: [[Rect; 3]; 3],
    #[cfg(test)]
    pub meter: Rect,
    #[cfg(test)]
    pub rate_mode: Rect,
}
pub(super) fn show(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    tempo: f32,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, s, dimensions, sources, live, level, tempo),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    tempo: f32,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let mut bases = s.values();
    bases[3] = s.base_rate_hz(tempo) as f32;
    let mut markers: [_; 8] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &s.modulations,
            sources,
            P::CHORUS[i],
            bases[i],
            s.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    if let Some(marker) = &mut markers[3] {
        let [lo, hi] = s.rate_bounds(tempo).map(|v| v as f32);
        marker.range = marker.range.map(|v| v.clamp(lo, hi));
    }
    let open = s.modulation_open;
    let mut b = Bounds {
        power: Rect::NOTHING,
        #[cfg(test)]
        sections: [Rect::NOTHING; 3],
        #[cfg(test)]
        controls: [[Rect::NOTHING; 3]; 3],
        #[cfg(test)]
        meter: Rect::NOTHING,
        #[cfg(test)]
        rate_mode: Rect::NOTHING,
    };
    let panel = panels::PanelStyle::default();
    let mut enabled = s.enabled;
    let mut collapsed = s.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Chorus", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                let mut last = Rect::NOTHING;
                for (section, name) in ["Chorus", "Motion", "Out"].into_iter().enumerate() {
                    let width = panels::unit_columns_width(if section == 1 { 2 } else { 1 });
                    let strip = ui.push_id(section, |ui| {
                        panels::unit_strip_with_width(ui, crate::envelope_panel::body_height(compact), width, egui::UiBuilder::new(), |ui| {
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
                                                        #[cfg(test)] {
                                                            match (column, row) {
                                                                (0, 0) => b.rate_mode = response.rect,
                                                                (0, 1) => b.controls[1][0] = response.rect,
                                                                (1, 0) => b.controls[1][1] = response.rect,
                                                                _ => b.controls[1][2] = response.rect,
                                                            }
                                                        }
                                                        #[cfg(not(test))] let _ = response;
                                                    }
                                    });
                                    // Reserve the third shared row so all strip footers align.
                                    ui.allocate_exact_size(Vec2::new(ui.available_width(), knobs::CHOICE_SIZE), egui::Sense::hover());
                                } else {
                                    for (row, index) in if section == 0 { [0,1,2] } else { [5,6,7] }.into_iter().enumerate() {
                                        let response = knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| parameter(ui, s, index, markers[index].as_ref()));
                                        #[cfg(test)] { b.controls[section][row] = response.rect; }
                                        #[cfg(not(test))] let _ = (row, response);
                                    }
                                }
                            }
                            rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| rows::unit_label(ui, name));
                        })
                    }).inner.response.rect;
                    if section > 0 { panels::unit_divider(ui, strip.left(), strip.y_range().into()); }
                    if section == 2 && !compact {
                        let rail = meters::effect_output_level(ui, strip, panel.margin, level, s.enabled);
                        #[cfg(test)] { b.meter = rail; }
                        #[cfg(not(test))] let _ = rail;
                    }
                    #[cfg(test)] { b.sections[section] = strip; }
                    last = strip;
                }
                if open {
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut s.modulations, sources, &P::CHORUS, <ChorusSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
            });
        },
    );
    s.enabled = enabled;
    s.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut s.modulation_open);
    b.power = result.power.rect;
    (b, result.header)
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
