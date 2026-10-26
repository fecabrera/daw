use crate::{meters, panels};
use daw_core::{DriveV2Settings, ModulationParameter as P};
use egui::{Rect, Ui};
pub(super) struct Bounds {
    #[cfg(test)]
    pub meter: Rect,
    pub power: Rect,
    #[cfg(test)]
    pub sections: [Rect; 3],
    #[cfg(test)]
    pub knobs: [[Rect; 3]; 3],
    #[cfg(test)]
    pub color_power: Rect,
    #[cfg(test)]
    pub clip: Rect,
}
pub(super) fn show(
    ui: &mut Ui,
    s: &mut DriveV2Settings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, s, dimensions, sources, live, level),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    s: &mut DriveV2Settings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let bases = [
        s.input_gain_db,
        s.gain,
        s.tone,
        s.color_strength,
        s.color_frequency_hz,
        s.level_db,
        s.dry_wet,
    ];
    let markers: [_; 7] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &s.modulations,
            sources,
            P::DRIVE_V2[i],
            bases[i],
            s.enabled && (!(3..=4).contains(&i) || s.color_enabled),
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = s.modulation_open;
    let mut bounds = Bounds {
        #[cfg(test)]
        meter: Rect::NOTHING,
        power: Rect::NOTHING,
        #[cfg(test)]
        sections: [Rect::NOTHING; 3],
        #[cfg(test)]
        knobs: [[Rect::NOTHING; 3]; 3],
        #[cfg(test)]
        color_power: Rect::NOTHING,
        #[cfg(test)]
        clip: Rect::NOTHING,
    };
    let (mut enabled, mut collapsed) = (s.enabled, s.collapsed);
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Drive v2", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let shared = daw_unit_ui::drive_v2::show(
                    ui,
                    s,
                    compact,
                    crate::envelope_panel::body_height(true),
                    &markers,
                    level,
                );
                #[cfg(test)]
                {
                    bounds.meter = shared.meter;
                    bounds.sections = shared.sections;
                    bounds.knobs = shared.knobs;
                    bounds.color_power = shared.color_power;
                    bounds.clip = shared.clip;
                }
                if open {
                    daw_unit_ui::drive::divider(ui, shared.rect.right(), shared.rect.y_range());
                    crate::modulation_panel::body(
                        ui,
                        &mut s.modulations,
                        sources,
                        &P::DRIVE_V2,
                        daw_unit_core::ModulationScope::Shared,
                        compact,
                    );
                }
            });
        },
    );
    s.enabled = enabled;
    s.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, s.collapsed, &mut s.modulation_open);
    bounds.power = result.power.rect;
    (bounds, result.header)
}
