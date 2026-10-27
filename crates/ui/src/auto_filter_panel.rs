use crate::{knobs, meters, panels, theme};
use daw_core::{AutoFilterSettings, ModulationParameter as Parameter};
pub(super) use daw_unit_ui::auto_filter::Bounds;
#[cfg(test)]
use daw_unit_ui::auto_filter::filter_diagram;
use egui::{Rect, Ui, Vec2};

fn show_panel(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 8]>,
    options: (u32, bool),
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (sample_rate_hz, internal_lfo) = options;
    let (compact, height) = dimensions;
    ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),|ui| {
    let modulation_open=settings.modulation_open;
    let bases = [settings.frequency_hz, settings.q, settings.level_db, settings.dry_wet, settings.strength_octaves, settings.sync_phase_degrees, settings.stereo_phase_degrees];
    let lfo_enabled = settings.enabled && settings.lfo_enabled;
    let strength_marker = crate::modulation_panel::marker(&settings.modulations, sources, Parameter::AutoFilterStrength, settings.strength_octaves, lfo_enabled, None, [live.map(|v|v[5]), None]);
    let sweep_depth = strength_marker.as_ref().map_or(settings.strength_octaves, |m| m.range[1]);
    let mut markers: [Option<knobs::ModulationMarker>; 7] = std::array::from_fn(|i| crate::modulation_panel::marker(
        &settings.modulations, sources, daw_core::ModulationParameter::AUTO_FILTER[i], bases[i], if i < 4 { settings.enabled } else { lfo_enabled },
        (i==0 && internal_lfo).then_some((settings.lfo_enabled, sweep_depth)),
        [live.map(|v| v[if i==0 {0}else{i+1}]), if i==0 && internal_lfo {live.map(|v|v[1])}else{None}],
    ));
    if let Some(marker) = &mut markers[0] { marker.range = marker.range.map(|v| v.min(sample_rate_hz as f32 * 0.45)); }
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 4],
        lfo: Rect::NOTHING,
        sections: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(ui, (if internal_lfo { "Auto-filter" } else { "Filter" }, compact), height, &mut enabled, &mut collapsed, |ui| {
        ui.horizontal_top(|ui| { ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING)); bounds = daw_unit_ui::auto_filter::show(ui, settings, (compact, crate::envelope_panel::body_height(compact)), &markers, internal_lfo, level);
if modulation_open {
                let rect = bounds.sections[2];
                panels::unit_divider(ui, rect.right(), rect.y_range().into());
                crate::modulation_panel::body(ui, &mut settings.modulations, sources, if internal_lfo { &Parameter::AUTO_FILTER } else { &Parameter::FILTER }, <daw_unit_core::AutoFilterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
            }
})
    });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    crate::modulation_panel::toggle(ui,&result.header,settings.collapsed,&mut settings.modulation_open);
    (bounds, result.header)
    }).inner
}

/// Auto-filter and Filter share their base controls, shell and modulation strips.
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
    sample_rate_hz: u32,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    show_panel(
        ui,
        settings,
        dimensions,
        sources,
        live.map(|v| std::array::from_fn(|i| v[i])),
        (sample_rate_hz, true),
        level,
    )
}

pub(super) fn show_static(
    ui: &mut Ui,
    settings: &mut daw_core::FilterSettings,
    dimensions: (bool, f32),
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
    sample_rate_hz: u32,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    // This is a temporary control view, never saved or sent to the processor.
    // Move routes into it so painting does not clone the connection list.
    let mut view = AutoFilterSettings {
        enabled: settings.enabled,
        collapsed: settings.collapsed,
        modulation_open: settings.modulation_open,
        shape: settings.shape,
        frequency_hz: settings.frequency_hz,
        q: settings.q,
        level_db: settings.level_db,
        dry_wet: settings.dry_wet,
        lfo_enabled: false,
        modulations: std::mem::take(&mut settings.modulations),
        ..Default::default()
    };
    let result = show_panel(
        ui,
        &mut view,
        dimensions,
        sources,
        live.map(|v| std::array::from_fn(|i| if i < 5 { v[i] } else { f32::NAN })),
        (sample_rate_hz, false),
        level,
    );
    settings.enabled = view.enabled;
    settings.collapsed = view.collapsed;
    settings.modulation_open = view.modulation_open;
    settings.shape = view.shape;
    settings.frequency_hz = view.frequency_hz;
    settings.q = view.q;
    settings.level_db = view.level_db;
    settings.dry_wet = view.dry_wet;
    settings.modulations = view.modulations;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfo_controls::diagram as lfo_diagram;
    use daw_unit_core::LfoWave;

    #[test]
    fn diagrams_show_the_filter_response_and_each_distinct_waveform() {
        let low = filter_diagram(0.0);
        let high = filter_diagram(1.0);
        let band = filter_diagram(2.0);
        let notch = filter_diagram(3.0);
        assert!(low[0].y < low[24].y);
        assert!(high[0].y > high[24].y);
        assert!(band[12].y < band[0].y && band[12].y < band[24].y);
        assert!(notch[12].y > notch[0].y && notch[12].y > notch[24].y);
        for (index, _) in LfoWave::ALL.iter().enumerate() {
            let wave = lfo_diagram(index as f32);
            assert!(
                wave.iter()
                    .all(|point| point.x.abs() <= 0.5 && point.y.abs() <= 0.5)
            );
            for other in 0..index {
                assert_ne!(wave, lfo_diagram(other as f32));
            }
        }
        let up = lfo_diagram(2.0);
        let down = lfo_diagram(3.0);
        assert!(up[0].y > up[1].y);
        assert!(down[0].y < down[1].y);
    }
}
