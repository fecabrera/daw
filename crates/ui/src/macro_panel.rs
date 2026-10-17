//! Multiple stable-ID Macro providers in one shared unit shell and targeted route editor.
use crate::{envelope_panel, icons, knobs, modulation_panel, panels, rows, theme};
use daw_core::{Id, MacroInstance, MacroModuleSettings, ModulationParameter};
use daw_unit_core::ModulationScope;
use egui::{Ui, Vec2};

pub(super) struct Controls<'a> {
    pub layer: Id,
    pub sources: &'a [modulation_panel::Source],
    pub available: &'a [(daw_core::ModulationSourceId, Vec<modulation_panel::Source>)],
    pub live: &'a [(Id, f32)],
    pub name_edit: &'a mut Option<NameEdit>,
}
pub(super) struct NameEdit {
    layer: Id,
    unit: Id,
    text: String,
    focus: bool,
}
impl NameEdit {
    pub(super) fn on_layer(&self, layer: Id) -> bool {
        self.layer == layer
    }
    pub(super) fn available(&self, instrument: &daw_core::InstrumentSettings) -> bool {
        instrument.macro_modules().any(|(owner, module)| {
            owner == self.layer
                && module.present
                && !module.collapsed
                && module
                    .controls
                    .iter()
                    .any(|c| c.id == self.unit && c.present)
        })
    }
}

fn name_label(
    ui: &mut Ui,
    control: &mut MacroInstance,
    layer: Id,
    edit: &mut Option<NameEdit>,
    label: egui::RichText,
    width: f32,
) -> egui::Response {
    if edit
        .as_ref()
        .is_some_and(|e| e.layer == layer && e.unit == control.id)
    {
        let mut draft = edit.take().unwrap();
        let mut output = egui::TextEdit::singleline(&mut draft.text)
            .id(ui.make_persistent_id("macro-name"))
            .font(egui::FontId::proportional(panels::HEADER_FONT_SIZE))
            .desired_width(width)
            .min_size(Vec2::new(width, icons::COMPACT_BUTTON_SIZE))
            .margin(Vec2::new(2.0, 0.0))
            .show(ui);
        output.response.widget_info(|| {
            egui::WidgetInfo::text_edit(ui.is_enabled(), label.text(), &draft.text, "Macro name")
        });
        if draft.focus {
            output.response.request_focus();
            output
                .state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(draft.text.chars().count()),
                )));
            output.state.store(ui.ctx(), output.response.id);
            draft.focus = false;
        }
        if let Some(action) = crate::inline_edit_action(ui, &output.response) {
            output.response.surrender_focus();
            let name = draft.text.trim();
            if action == crate::InlineEditAction::Commit
                && MacroInstance::valid_name(name)
                && name != label.text()
            {
                control.name = Some(name.to_owned());
            }
        } else {
            *edit = Some(draft);
        }
        output.response.response
    } else {
        let response = ui
            .add_sized(
                Vec2::new(width, icons::COMPACT_BUTTON_SIZE),
                egui::Label::new(label.clone())
                    .selectable(false)
                    .truncate()
                    .sense(egui::Sense::click()),
            )
            .on_hover_text(format!("{}\nDouble-click to rename macro.", label.text()));
        if response.double_clicked() && ui.is_enabled() && ui.input(|i| i.focused) {
            *edit = Some(NameEdit {
                layer,
                unit: control.id,
                text: label.text().to_owned(),
                focus: true,
            });
            ui.ctx().request_repaint();
        }
        response
    }
}
pub(super) fn unit(
    ui: &mut Ui,
    module: &mut MacroModuleSettings,
    height: f32,
    compact: bool,
    controls: Controls<'_>,
) -> Option<panels::UnitResponse<()>> {
    if !module.present {
        return None;
    }
    let is_enabled = module.enabled;
    let mut enabled = is_enabled;
    let mut collapsed = module.collapsed;
    let open = module.modulation_open;
    let mut add = false;
    let mut remove = None;
    // An empty component still needs room for separate header power and modulation controls.
    let add_width = if module.controls.iter().any(|c| c.present) {
        icons::COMPACT_BUTTON_SIZE + 20.0
    } else {
        crate::midi_editor::UNIT_STRIP_WIDTH
    };
    let result = ui.scope_builder(egui::UiBuilder::new().id_salt("macro-module")
        .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")), |ui| {
        panels::PanelStyle { margin: 0, ..Default::default() }.unit(ui, ("Macro", compact), height,
            &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let body_top = ui.cursor().top();
                for (index, control) in module.controls.iter_mut().enumerate().filter(|(_, c)| c.present) {
                    let routes: Vec<_> = module.modulations.iter().filter(|r| r.target == Some(control.id)).copied().collect();
                    let live = controls.live.iter().find(|(id, _)| *id == control.id).map(|(_, v)| *v);
                    let marker = modulation_panel::marker(&routes, controls.sources, ModulationParameter::MacroValue,
                        control.settings.value, is_enabled && control.enabled, None, [live, None]);
                    let strip = ui.scope_builder(egui::UiBuilder::new().id_salt(control.id).sense(egui::Sense::click()), |ui| egui::Frame::new().inner_margin(10).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                            ui.set_width(crate::midi_editor::UNIT_STRIP_WIDTH - 20.0);
                            ui.set_min_height(envelope_panel::body_height(compact));
                            let mut responses = Vec::with_capacity(3);
                            if !compact {
                                responses.push(knobs::row(ui, &mut control.settings.value, knobs::CHOICE_SIZE, |ui, value|
                                    ui.add(knobs::Knob::new(value, crate::parameter_knobs::destination(ModulationParameter::MacroValue))
                                        .modulated(marker.as_ref(), &mut module.modulation_open))));
                                for _ in 0..2 { ui.allocate_exact_size(Vec2::new(ui.available_width(), knobs::CHOICE_SIZE), egui::Sense::hover()); }
                            }
                            let footer = rows::centered(ui, if compact { envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                                let name = control.display_name(index).into_owned();
                                let (power, label) = rows::power_label_with(ui, &name, &format!("Toggle {name}"), control.enabled,
                                    |ui, label, width| name_label(ui, control, controls.layer, controls.name_edit, label, width));
                                if power.clicked() {
                                    control.enabled = !control.enabled;
                                }
                                [power, label]
                            }).inner;
                            responses.extend(footer);
                            responses
                        })
                    }));
                    // Child controls own their pointer events; include them as well as strip padding.
                    for response in strip.inner.inner.inner.iter().chain([&strip.response]) {
                        response.context_menu(|ui| {
                            if ui.add_enabled(response.enabled(), egui::Button::new("Remove macro")).clicked() { remove = Some(index); ui.close(); }
                        });
                    }
                    panels::unit_divider(ui, strip.response.rect.right(), strip.response.rect.y_range().into());
                }
                add = icons::circular_strip_button(ui, icons::Lucide::Plus, "Add macro",
                    Vec2::new(add_width, envelope_panel::body_height(compact) + 20.0),
                    icons::COMPACT_BUTTON_SIZE).clicked();
                if open {
                    panels::unit_divider(ui, ui.cursor().left(), body_top..=(body_top + envelope_panel::body_height(compact) + 20.0));
                    let targets: Vec<_> = module.controls.iter().enumerate().filter(|(_, c)| c.present).map(|(index, c)|
                        modulation_panel::Target { parameter: ModulationParameter::MacroValue, id: Some(c.id), label: format!("{} · Value", c.display_name(index)) }).collect();
                    modulation_panel::targeted_body_with_sources(ui, &mut module.modulations, &targets,
                        ModulationScope::Shared, compact, |target| controls.available.iter()
                            .find(|(id, _)| id.layer == controls.layer && Some(id.unit) == target)
                            .map_or(&[], |(_, sources)| sources.as_slice()));
                }
            });
        })
    }).inner;
    module.enabled = enabled;
    module.collapsed = collapsed;
    modulation_panel::toggle(
        ui,
        &result.header,
        module.collapsed,
        &mut module.modulation_open,
    );
    result.header.context_menu(|ui| {
        if ui.button("Remove Macro component").clicked() {
            module.present = false;
            ui.close();
        }
    });
    if let Some(index) = remove {
        module.controls[index].present = false;
    }
    if add {
        module.add();
    }
    Some(result)
}
