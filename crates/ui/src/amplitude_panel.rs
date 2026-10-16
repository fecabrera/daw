use crate::{icons, knobs, modulation_panel, panels, rows};
use daw_core::AmplitudeSettings;
use egui::{Ui, Vec2};
pub(super) fn unit(
    ui: &mut Ui,
    settings: &mut AmplitudeSettings,
    sources: &[modulation_panel::Source],
    height: f32,
    compact: bool,
    live: Option<f32>,
) -> Option<panels::UnitResponse<()>> {
    if !settings.present {
        return None;
    }
    let result = ui
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt("amplitude")
                .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
            |ui| {
                let open = settings.modulation_open;
                let marker = modulation_panel::marker(
                    &settings.modulations,
                    sources,
                    daw_core::ModulationParameter::Amplitude,
                    settings.level_db,
                    settings.enabled,
                    None,
                    [live, None],
                );
                let mut enabled = settings.enabled;
                let mut collapsed = settings.collapsed;
                let result = panels::PanelStyle {
                    margin: 0,
                    ..Default::default()
                }
                .unit(
                    ui,
                    ("Amplitude", compact),
                    height,
                    &mut enabled,
                    &mut collapsed,
                    |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let regular = egui::Frame::new().inner_margin(10).show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.spacing_mut().item_spacing.y =
                                        f32::from(crate::theme::TOOLBAR_PADDING);
                                    ui.set_width(158.0);
                                    ui.set_min_height(crate::envelope_panel::body_height(compact));
                                    if !compact {
                                        knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
                                            ui.add(
                                                knobs::Knob::new(
                                                    &mut s.level_db,
                                                    crate::parameter_knobs::destination(daw_core::ModulationParameter::Amplitude),
                                                )
                                                .modulated(marker.as_ref(), &mut s.modulation_open),
                                            )
                                        });
                                        for _ in 0..2 {
                                            ui.allocate_exact_size(
                                                Vec2::new(ui.available_width(), knobs::CHOICE_SIZE),
                                                egui::Sense::hover(),
                                            );
                                        }
                                    }
                                    rows::centered(
                                        ui,
                                        if compact {
                                            crate::envelope_panel::body_height(true)
                                        } else {
                                            icons::COMPACT_BUTTON_SIZE
                                        },
                                        |ui| rows::unit_label(ui, "Level"),
                                    );
                                });
                            });
                            if open {
                                panels::unit_divider(
                                    ui,
                                    regular.response.rect.right(),
                                    regular.response.rect.y_range().into(),
                                );
                                modulation_panel::body(
                                    ui,
                                    &mut settings.modulations,
                                    sources,
                                    <daw_unit_core::AmplitudeSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS, <daw_unit_core::AmplitudeSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE,
                                    compact,
                                );
                            }
                        });
                    },
                );
                settings.enabled = enabled;
                settings.collapsed = collapsed;
                modulation_panel::toggle(
                    ui,
                    &result.header,
                    settings.collapsed,
                    &mut settings.modulation_open,
                );
                result
            },
        )
        .inner;
    result.header.context_menu(|ui| {
        if ui.button("Remove amplitude").clicked() {
            settings.present = false;
            ui.close();
        }
    });

    Some(result)
}
