use crate::{meters, panels, theme};
use daw_core::{AmbientReverbSettings, ModulationParameter as P};
pub(super) use daw_unit_ui::ambient_reverb::Bounds;
use egui::Ui;
pub(super) fn show(
    ui: &mut Ui,
    s: &mut AmbientReverbSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    _tempo: f32,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, s, dimensions, sources, live, level, _tempo),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    s: &mut AmbientReverbSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    _tempo: f32,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let bases = s.values();
    let markers: [_; 8] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &s.modulations,
            sources,
            P::AMBIENT_REVERB[i],
            bases[i],
            s.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = s.modulation_open;
    let mut b = Bounds::default();
    let panel = panels::PanelStyle::default();
    let mut enabled = s.enabled;
    let mut collapsed = s.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Ambient Reverb", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
 b = daw_unit_ui::ambient_reverb::show(ui, s, compact, crate::envelope_panel::body_height(compact), &markers, level);
if open {
 let last = b.sections[daw_unit_ui::ambient_reverb::STRIP_COUNT - 1];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut s.modulations, sources, &P::AMBIENT_REVERB, <AmbientReverbSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
})
        },
    );
    s.enabled = enabled;
    s.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut s.modulation_open);
    b.power = result.power.rect;
    (b, result.header)
}
