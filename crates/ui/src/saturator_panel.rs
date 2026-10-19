use crate::{
    icons,
    knobs::{self, Knob},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{
    ModulationParameter as Parameter, SaturatorClipMode, SaturatorSettings, SaturatorShape,
};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 5],
    pub sections: [Rect; 2],
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut SaturatorSettings,
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
    settings: &mut SaturatorSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [settings.drive_db, settings.level_db, settings.dry_wet];
    let markers: [_; 3] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::SATURATOR[i],
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
        ui, ("Saturator", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Saturation", "Out"].into_iter().enumerate() {
                    let rect = ui.push_id(column, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin));
                            ui.set_min_height(crate::envelope_panel::body_height(compact));
                            if !compact {
                                for row in 0..3 {
                                    let index = column * 3 + row;
                                    let mut control = |ui: &mut Ui, s: &mut SaturatorSettings| {
                                        if index >= 5 { return ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover()); }
                                        let response = match index {
                                            0 => crate::modulation_panel::selector_row(ui,"Shape",|ui| {
                                                egui::ComboBox::from_id_salt("shape").selected_text(egui::RichText::new(s.shape.label()).size(panels::HEADER_FONT_SIZE)).width(crate::modulation_panel::SELECTOR_WIDTH).truncate().show_ui(ui,|ui| {for shape in SaturatorShape::ALL {ui.selectable_value(&mut s.shape,shape,shape.label());}}).response
                                            }),
                                            1 => crate::modulation_panel::selector_row(ui,"Clip Mode",|ui| {
                                                egui::ComboBox::from_id_salt("clip-mode").selected_text(egui::RichText::new(s.clip_mode.label()).size(panels::HEADER_FONT_SIZE)).width(crate::modulation_panel::SELECTOR_WIDTH).truncate().show_ui(ui,|ui| {for clip in SaturatorClipMode::ALL {ui.selectable_value(&mut s.clip_mode,clip,clip.label());}}).response
                                            }),
                                            _ => {
                                                let target=match index {2=>0,3=>1,_=>2};
                                                let value=match index {2=>&mut s.drive_db,3=>&mut s.level_db,_=>&mut s.dry_wet};
                                                ui.add(Knob::new(value,parameter_knobs::destination(Parameter::SATURATOR[target]).named(Parameter::SATURATOR[target].label())).modulated(markers[target].as_ref(),&mut s.modulation_open))
                                            }
                                        };
                                        bounds.controls[index] = response.rect;
                                        response
                                    };
                                    if index < 2 {control(ui,settings);} else {knobs::row(ui,settings,knobs::CHOICE_SIZE,control);}
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
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, &Parameter::SATURATOR,
                        <SaturatorSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
