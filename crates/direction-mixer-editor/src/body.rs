//! Bind plugin automation to the same portable body used by the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 1],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::DirectionMixerSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, settings, level, 0.0.into(), [0.0; 3].into())
}
pub fn render(
    ui: &mut Ui,
    settings: &mut daw_unit_core::DirectionMixerSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    gains: meters::BandGainChange<'_>,
) -> Bounds {
    let _ = (level, reduction, gains);
    let before = PluginState::from_settings(settings).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        daw_unit_ui::direction_mixer::show(
            ui,
            settings,
            false,
            unit_control_height(),
            &std::array::from_fn::<_, 2, _>(|_| None),
        )
    });
    let after = PluginState::from_settings(settings).ok();
    let controls = std::array::from_fn(|slot| {
        let rect = match P::ALL[slot + 1] {
            P::Input => b.controls[0],
            P::Direction => b.controls[1],
            P::Spread => b.controls[2],
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
        sections: [b.section],
        controls,
        readouts: vec![],
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;

#[cfg(test)]
use daw_unit_ui::knobs::ModulationMarker;
