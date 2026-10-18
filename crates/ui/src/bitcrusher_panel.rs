use crate::{
    icons,
    knobs::{self, Knob},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{BitcrusherSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 5],
    pub sections: [Rect; 2],
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut BitcrusherSettings,
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
    settings: &mut BitcrusherSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [
        settings.drive_db,
        f32::from(settings.resolution_bits),
        f32::from(settings.downsampling),
        settings.level_db,
        settings.dry_wet,
    ];
    let markers: [_; 5] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::BITCRUSHER[i],
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
        sections: [Rect::NOTHING; 2],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Bitcrusher", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Crush", "Output"].into_iter().enumerate() {
                    let rect = ui.push_id(column, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin));
                            ui.set_min_height(crate::envelope_panel::body_height(compact));
                            if !compact {
                                for row in 0..3 {
                                    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
                                        let index = column * 3 + row;
                                        if index >= 5 { return ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover()); }
                                        let mut value = match index {
                                            0 => s.drive_db, 1 => f32::from(s.resolution_bits),
                                            2 => f32::from(s.downsampling), 3 => s.level_db, _ => s.dry_wet,
                                        };
                                        let response = ui.add(Knob::new(&mut value, parameter_knobs::destination(Parameter::BITCRUSHER[index]))
                                            .modulated(markers[index].as_ref(), &mut s.modulation_open));
                                        if response.changed() {
                                            match index { 0 => s.drive_db = value, 1 => s.resolution_bits = value as u8,
                                                2 => s.downsampling = value as u8, 3 => s.level_db = value, _ => s.dry_wet = value }
                                        }
                                        bounds.controls[index] = response.rect;
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
                    let last = bounds.sections[1];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, &Parameter::BITCRUSHER,
                        <BitcrusherSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
