use crate::{panels, theme};
use daw_core::{DirectionMixerSettings, ModulationParameter as Parameter};
pub(super) use daw_unit_ui::direction_mixer::Bounds;
use egui::{Rect, Ui};

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut DirectionMixerSettings,
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
    settings: &mut DirectionMixerSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [settings.direction_degrees, settings.spread];
    let markers: [_; 2] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::DIRECTION_MIXER[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let panel = panels::PanelStyle::default();
    let open = settings.modulation_open;
    let mut bounds = Bounds {
        section: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui, ("Direction Mixer", compact), height, &mut enabled, &mut collapsed, |ui| {
            ui.horizontal_top(|ui| {
 ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
 bounds = daw_unit_ui::direction_mixer::show(ui, settings, compact, crate::envelope_panel::body_height(compact), &markers);
if open {
                    panels::unit_divider(ui, bounds.section.right(), bounds.section.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, &Parameter::DIRECTION_MIXER,
                        <DirectionMixerSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
})
        });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
