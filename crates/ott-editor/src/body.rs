//! Bind plugin automation to the same portable body used by the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    3.0 * unit_columns_width(2) + UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 4],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::OttSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, settings, level, 0.0.into(), [0.0; 3].into())
}
pub fn render(
    ui: &mut Ui,
    settings: &mut daw_unit_core::OttSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    gains: meters::BandGainChange<'_>,
) -> Bounds {
    let _ = (level, reduction, gains);
    let before = PluginState::from_settings(settings).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        daw_unit_ui::ott::show(
            ui,
            settings,
            false,
            unit_control_height(),
            &std::array::from_fn::<_, 19, _>(|_| None),
            gains,
            level,
        )
    });
    let after = PluginState::from_settings(settings).ok();
    let controls = std::array::from_fn(|slot| {
        let rect = match P::ALL[slot + 1] {
            P::LowFrequency => b.controls[0][0][0],
            P::HighFrequency => b.controls[2][0][0],
            P::Level => b.controls[3][0][0],
            P::DryWet => b.controls[3][0][1],
            P::Detection => b.controls[3][0][2],
            P::LowDepth => b.controls[0][1][0],
            P::LowInputDb => b.controls[0][0][1],
            P::LowOutputDb => b.controls[0][1][1],
            P::LowAttackMs => b.controls[0][0][2],
            P::LowReleaseMs => b.controls[0][1][2],
            P::LowEnabled => b.bands[0],
            P::MidDepth => b.controls[1][1][0],
            P::MidInputDb => b.controls[1][0][1],
            P::MidOutputDb => b.controls[1][1][1],
            P::MidAttackMs => b.controls[1][0][2],
            P::MidReleaseMs => b.controls[1][1][2],
            P::MidEnabled => b.bands[1],
            P::HighDepth => b.controls[2][1][0],
            P::HighInputDb => b.controls[2][0][1],
            P::HighOutputDb => b.controls[2][1][1],
            P::HighAttackMs => b.controls[2][0][2],
            P::HighReleaseMs => b.controls[2][1][2],
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
    Bounds {
        sections: b.sections,
        controls,
        readouts: b
            .meters
            .iter()
            .enumerate()
            .filter_map(|(i, rect)| control_capture::at(&captured, *rect).map(|r| (4 << i, r)))
            .collect(),
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;

#[cfg(test)]
use daw_unit_ui::knobs::ModulationMarker;
