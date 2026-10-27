use crate::{meters, panels, theme};
use daw_core::{CompressorSettings, ModulationParameter as Parameter};
pub(super) use daw_unit_ui::compressor::Bounds;
use egui::{Rect, Ui, Vec2};

pub(super) fn show<'a>(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    dimensions: (bool, f32),
    reduction_db: impl Into<meters::GainReduction<'a>>,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let reduction_db = reduction_db.into();
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, dimensions, reduction_db, sources, live, level),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    dimensions: (bool, f32),
    reduction_db: meters::GainReduction<'_>,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let reduction_db = reduction_db.for_unit(settings.enabled);
    let bases = [
        settings.threshold_db,
        settings.ratio,
        settings.attack_ms,
        settings.release_ms,
        settings.knee_db,
        settings.level_db,
        settings.dry_wet,
    ];
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::COMPRESSOR[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = settings.modulation_open;
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        output_meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[Rect::NOTHING; 3]; 3],
        sections: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Compressor", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
 bounds = daw_unit_ui::compressor::show(ui, settings, compact, crate::envelope_panel::body_height(compact), &markers, reduction_db, level);
if open {
                    let last = bounds.sections[2];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, <CompressorSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS, <CompressorSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
})
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
