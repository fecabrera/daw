//! Bind plugin automation to the same portable body used by the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    3.0 * UNIT_STRIP_WIDTH,
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
    settings: &mut daw_unit_core::CompressorSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, settings, level, 0.0.into(), [0.0; 3].into())
}
pub fn render(
    ui: &mut Ui,
    settings: &mut daw_unit_core::CompressorSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    gains: meters::BandGainChange<'_>,
) -> Bounds {
    let _ = (level, reduction, gains);
    let before = PluginState::from_settings(settings).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        daw_unit_ui::compressor::show(
            ui,
            settings,
            false,
            unit_control_height(),
            &std::array::from_fn::<_, 7, _>(|_| None),
            reduction,
            level,
        )
    });
    let after = PluginState::from_settings(settings).ok();
    let controls = std::array::from_fn(|slot| {
        let rect = match P::ALL[slot + 1] {
            P::Threshold => b.controls[0][0],
            P::Ratio => b.controls[0][1],
            P::Attack => b.controls[1][0],
            P::Release => b.controls[1][1],
            P::Knee => b.controls[1][2],
            P::Level => b.controls[2][0],
            P::DryWet => b.controls[2][1],
            P::Detection => b.controls[0][2],
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
    Bounds {
        sections: b.sections,
        controls,
        readouts: control_capture::at(&captured, b.controls[2][2])
            .map(|r| vec![(2, r)])
            .unwrap_or_default(),
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;

#[cfg(test)]
use daw_unit_ui::knobs::ModulationMarker;
