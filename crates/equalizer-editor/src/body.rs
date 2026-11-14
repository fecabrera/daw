//! Bind plugin automation to the same portable body used by the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    daw_unit_ui::eq_graph::WIDTH + 3.0 * UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 3],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::EqSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, settings, level, 0.0.into(), [0.0; 3], None)
}
pub fn render(
    ui: &mut Ui,
    settings: &mut daw_unit_core::EqSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    levels: [f32; 3],
    spectrum: Option<&daw_unit_core::SpectrumReading>,
) -> Bounds {
    let _ = (level, reduction);
    let before = PluginState::from_settings(settings).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        daw_unit_ui::equalizer::show(
            ui,
            settings,
            false,
            unit_control_height(),
            levels,
            &std::array::from_fn(|_| None),
            Some(daw_unit_ui::eq_graph::FixedInput {
                live_edits: true,
                spectrum,
                response: daw_equalizer::response,
                effective: None,
            }),
        )
    });
    let after = PluginState::from_settings(settings).ok();
    let mut controls = std::array::from_fn(|slot| {
        let rect = match P::ALL[slot + 1] {
            P::LowGain => b.knobs[0][0],
            P::LowFrequency => b.knobs[0][1],
            P::LowQ => b.knobs[0][2],
            P::LowEnabled => b.bands[0],
            P::MidGain => b.knobs[1][0],
            P::MidEnabled => b.bands[1],
            P::HighGain => b.knobs[2][0],
            P::HighFrequency => b.knobs[2][1],
            P::HighQ => b.knobs[2][2],
            P::HighEnabled => b.bands[2],
            _ => Rect::NOTHING,
        };
        control_capture::at(&captured, rect).map(|mut response| {
            if let (Some(before), Some(after)) = (before, after) {
                let p = P::ALL[slot + 1];
                if before.get(p) != after.get(p) {
                    response.mark_changed();
                }
            }
            response
        })
    });
    for (id, parameter, response) in b.graph.edits {
        let p = match (id.as_u128(), parameter) {
            (1, 0) => P::LowGain,
            (1, 1) => P::LowFrequency,
            (1, 2) => P::LowQ,
            (2, 0) => P::MidGain,
            (3, 0) => P::HighGain,
            (3, 1) => P::HighFrequency,
            (3, 2) => P::HighQ,
            _ => continue,
        };
        let slot = P::ALL.iter().position(|v| *v == p).unwrap() - 1;
        controls[slot] = Some(response);
    }
    Bounds {
        sections: b.sections,
        controls,
        readouts: vec![],
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;

#[cfg(test)]
use daw_unit_ui::knobs::ModulationMarker;
