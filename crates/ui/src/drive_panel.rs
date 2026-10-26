use crate::{knobs, meters, panels};
use daw_core::DriveSettings;
use egui::{Rect, Ui};

pub(super) struct Bounds {
    #[cfg(test)]
    pub meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 2],
    pub labels: [Rect; 2],
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, dimensions, sources, live, level),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let open = settings.modulation_open;
    let bases = [
        settings.volume_db,
        settings.gain,
        settings.tone,
        settings.color_strength,
        settings.color_frequency_hz,
    ];
    let markers: [Option<knobs::ModulationMarker>; 5] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            daw_core::ModulationParameter::DRIVE[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        #[cfg(test)]
        meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 2],
        labels: [Rect::NOTHING; 2],
    };
    let (mut enabled, mut collapsed) = (settings.enabled, settings.collapsed);
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Drive", compact), height, &mut enabled, &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let shared = daw_unit_ui::drive::show(ui, settings, compact,
                    crate::envelope_panel::body_height(true), &markers, level);
                #[cfg(test)] { bounds.meter = shared.meter; }
                bounds.knobs = shared.knobs;
                bounds.labels = shared.labels;
                if open {
                    daw_unit_ui::drive::divider(ui, shared.rect.right(), shared.rect.y_range());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources,
                        <DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS,
                        <DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
            });
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(
        ui,
        &result.header,
        settings.collapsed,
        &mut settings.modulation_open,
    );
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
