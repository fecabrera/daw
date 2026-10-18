use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{DirectionMixerInput, DirectionMixerSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 3],
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut DirectionMixerSettings,
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
    settings: &mut DirectionMixerSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [settings.direction_degrees, settings.spread];
    let markers: [_; 2] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::DIRECTION_MIXER[i],
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
        controls: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Direction Mixer", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                let strip = egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin));
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
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
                        rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| { rows::unit_label(ui, "Stereo"); });
                    });
                }).response.rect;
                if open {
                    panels::unit_divider(ui, strip.right(), strip.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, &Parameter::DIRECTION_MIXER,
                        <DirectionMixerSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
