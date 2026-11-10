//! Bind the shared nine-strip body to host automation gestures.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    9.0 * UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 9],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
#[cfg(test)]
pub const SCROLLABLE: bool = false;
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    s: &mut daw_unit_core::HarmonicLiftSettings,
    _: bool,
    _: f32,
    _: &[Option<daw_unit_ui::knobs::ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, s, level, 0.0.into(), [0.0; 3].into(), [0.0.into(); 6])
}
pub fn render(
    ui: &mut Ui,
    s: &mut daw_unit_core::HarmonicLiftSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    _: meters::BandGainChange<'_>,
    band_levels: [meters::OutputLevel<'_>; 6],
) -> Bounds {
    let before = PluginState::from_settings(s).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        daw_unit_ui::harmonic_lift::show(
            ui,
            s,
            (false, unit_control_height()),
            daw_unit_ui::harmonic_lift::Controls {
                markers: &std::array::from_fn(|_| None),
                bands: &std::array::from_fn(|_| std::array::from_fn(|_| None)),
                reduction,
                level,
                band_levels,
            },
        )
    });
    let after = PluginState::from_settings(s).ok();
    let controls = std::array::from_fn(|i| {
        control_capture::at(&captured, b.controls[i]).map(|mut r| {
            if let (Some(before), Some(after)) = (before, after)
                && before.get(P::ALL[i + 1]) != after.get(P::ALL[i + 1])
            {
                r.mark_changed();
            }
            r
        })
    });
    let mut readouts: Vec<_> = b
        .band_meters
        .iter()
        .enumerate()
        .filter_map(|(i, rect)| control_capture::at(&captured, *rect).map(|r| (32 << i, r)))
        .collect();
    readouts.extend(control_capture::at(&captured, b.reduction).map(|r| (2, r)));
    Bounds {
        sections: b.sections,
        controls,
        readouts,
    }
}
