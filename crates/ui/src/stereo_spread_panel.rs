use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{ModulationParameter as Parameter, StereoSpreadSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 5],
    pub sections: [Rect; 3],
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut StereoSpreadSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, compact, height, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut StereoSpreadSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [
        settings.lower_intensity,
        settings.upper_intensity,
        settings.lower_frequency_hz,
        settings.upper_frequency_hz,
    ];
    let markers: [_; 4] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::STEREO_SPREAD[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let panel = panels::PanelStyle::default();
    let open = settings.modulation_open;
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [Rect::NOTHING; 5],
        sections: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Stereo Spread", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Lower", "Upper", "Spread"].into_iter().enumerate() {
                    let rect = ui.push_id(column, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin));
                            ui.set_min_height(crate::envelope_panel::body_height(compact));
                            if !compact {
                                for row in 0..3 {
                                    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
                                        if (column, row) == (2, 0) {
                                            let mut order = f32::from(s.order);
                                            let response = ui.add(Knob::new(&mut order,
                                                KnobSpec::new("Order", 2.0..=12.0, 8.0, 1.0)
                                                    .stepped(1.0).reference_step(1.0)
                                                    .value_label(|v| format!("{v:.0}"))
                                                    .format(|v| format!("{v:.0} bands"))
                                                    .endpoint_labels("2", "12")));
                                            if response.changed() { s.order = order as u8; }
                                            bounds.controls[4] = response.rect;
                                            return response.on_hover_text("Order: number of alternating frequency bands.");
                                        }
                                        let (index, control, value) = match (column, row) {
                                            (0, 0) => (0, 0, &mut s.lower_intensity),
                                            (0, 1) => (2, 1, &mut s.lower_frequency_hz),
                                            (1, 0) => (1, 2, &mut s.upper_intensity),
                                            (1, 1) => (3, 3, &mut s.upper_frequency_hz),
                                            _ => return ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover()),
                                        };
                                        let response = ui.add(Knob::new(value,
                                            parameter_knobs::destination(Parameter::STEREO_SPREAD[index]))
                                            .modulated(markers[index].as_ref(), &mut s.modulation_open));
                                        if response.changed() {
                                            if index == 2 { s.lower_frequency_hz = s.lower_frequency_hz.min(s.upper_frequency_hz - 1.0); }
                                            if index == 3 { s.upper_frequency_hz = s.upper_frequency_hz.max(s.lower_frequency_hz + 1.0); }
                                        }
                                        bounds.controls[control] = response.rect;
                                        response
                                    });
                                }
                            }
                            rows::centered(ui, if compact { crate::envelope_panel::body_height(true) }
                                else { icons::COMPACT_BUTTON_SIZE }, |ui| { rows::unit_label(ui, name); });
                        });
                    })).inner.response.rect;
                    bounds.sections[column] = rect;
                    if column > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
                }
                if open {
                    let last = bounds.sections[2];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources,
                        &Parameter::STEREO_SPREAD,
                        <StereoSpreadSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
            });
        });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
