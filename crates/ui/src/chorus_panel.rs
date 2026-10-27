use crate::{meters, panels, theme};
use daw_core::{ChorusSettings, ModulationParameter as P};
pub(super) use daw_unit_ui::chorus::Bounds;
use egui::{Rect, Ui};
pub(super) fn show(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    tempo: f32,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, s, dimensions, sources, live, level, tempo),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    s: &mut ChorusSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
    tempo: f32,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let mut bases = s.values();
    bases[3] = s.base_rate_hz(tempo) as f32;
    let mut markers: [_; 8] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &s.modulations,
            sources,
            P::CHORUS[i],
            bases[i],
            s.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    if let Some(marker) = &mut markers[3] {
        let [lo, hi] = s.rate_bounds(tempo).map(|v| v as f32);
        marker.range = marker.range.map(|v| v.clamp(lo, hi));
    }
    let open = s.modulation_open;
    let mut b = Bounds {
        power: Rect::NOTHING,
        sections: [Rect::NOTHING; 3],
        controls: [[Rect::NOTHING; 3]; 3],
        meter: Rect::NOTHING,
        rate_mode: Rect::NOTHING,
    };
    let panel = panels::PanelStyle::default();
    let mut enabled = s.enabled;
    let mut collapsed = s.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Chorus", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
 b = daw_unit_ui::chorus::show(ui, s, compact, crate::envelope_panel::body_height(compact), &markers, level, tempo);
if open {
 let last = b.sections[2];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut s.modulations, sources, &P::CHORUS, <ChorusSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
