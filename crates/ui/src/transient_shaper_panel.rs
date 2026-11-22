use crate::{meters, modulation_panel, panels};
use daw_core::{ModulationParameter, TransientShaperSettings};
use daw_unit_core::ModulationDestinationSettings;
use egui::{Response, Ui, Vec2};

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut TransientShaperSettings,
    (compact, height): (bool, f32),
    sources: &[modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Response, egui::Rect) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, (compact, height), sources, live, level),
    )
    .inner
}

fn show_inner(
    ui: &mut Ui,
    settings: &mut TransientShaperSettings,
    (compact, height): (bool, f32),
    sources: &[modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Response, egui::Rect) {
    let bases = settings.values();
    let parameters = ModulationParameter::TRANSIENT_SHAPER;
    let markers = std::array::from_fn(|i| {
        modulation_panel::marker(
            &settings.modulations,
            sources,
            parameters[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|values| values[i]), None],
        )
    });
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let panel = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    };
    let open = settings.modulation_open;
    let result = panel.unit_in_lane(
        ui,
        ("Transient Shaper", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                daw_unit_ui::transient_shaper::show(
                    ui,
                    settings,
                    compact,
                    crate::envelope_panel::body_height(true),
                    &markers,
                    level,
                );
                if open {
                    modulation_panel::body(
                        ui,
                        &mut settings.modulations,
                        sources,
                        &parameters,
                        TransientShaperSettings::MODULATION_SCOPE,
                        compact,
                    );
                }
            });
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    modulation_panel::toggle(ui, &result, collapsed, &mut settings.modulation_open);
    (result.header, result.power.rect)
}
