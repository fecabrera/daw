use crate::{meters, panels, theme};
use daw_core::{LimiterSettings, ModulationParameter as Parameter};
pub(super) use daw_unit_ui::limiter::Bounds;
use egui::{Rect, Ui};

pub(super) fn show<'a>(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
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
    settings: &mut LimiterSettings,
    dimensions: (bool, f32),
    reduction_db: meters::GainReduction<'_>,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let reduction_db = reduction_db.for_unit(settings.enabled);
    let bases = [
        settings.input_db,
        settings.release_ms,
        settings.lookahead_ms,
        settings.ceiling_db,
    ];
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::LIMITER[i],
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
        controls: [[Rect::NOTHING; 3]; 2],
        sections: [Rect::NOTHING; 2],
        meter: Rect::NOTHING,
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Limiter", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
 bounds = daw_unit_ui::limiter::show(ui, settings, compact, crate::envelope_panel::body_height(compact), &markers, reduction_db, level);
if open {
                    let last = bounds.sections[1];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources,
                        <LimiterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS,
                        <LimiterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
