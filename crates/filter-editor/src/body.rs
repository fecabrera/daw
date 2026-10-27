//! Bind plugin automation to the same portable body used by the DAW.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    2.0 * UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 2],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::FilterSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, settings, level, 0.0.into(), [0.0; 3].into())
}
pub fn render(
    ui: &mut Ui,
    settings: &mut daw_unit_core::FilterSettings,
    level: meters::OutputLevel<'_>,
    reduction: meters::GainReduction<'_>,
    gains: meters::BandGainChange<'_>,
) -> Bounds {
    let _ = (level, reduction, gains);
    let before = PluginState::from_settings(settings).ok();
    let (b, captured) = control_capture::scope(ui, |ui| {
        let mut view = daw_unit_core::AutoFilterSettings {
            enabled: settings.enabled,
            shape: settings.shape,
            frequency_hz: settings.frequency_hz,
            q: settings.q,
            level_db: settings.level_db,
            dry_wet: settings.dry_wet,
            lfo_enabled: false,
            ..Default::default()
        };
        let b = daw_unit_ui::auto_filter::show(
            ui,
            &mut view,
            (false, unit_control_height()),
            &std::array::from_fn(|_| None),
            false,
            level,
        );
        settings.shape = view.shape;
        settings.frequency_hz = view.frequency_hz;
        settings.q = view.q;
        settings.level_db = view.level_db;
        settings.dry_wet = view.dry_wet;
        b
    });
    let after = PluginState::from_settings(settings).ok();
    let controls = std::array::from_fn(|slot| {
        let rect = match P::ALL[slot + 1] {
            P::Shape => b.knobs[0][0],
            P::Frequency => b.knobs[0][1],
            P::Q => b.knobs[0][2],
            P::Level => b.knobs[3][0],
            P::DryWet => b.knobs[3][1],
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
        sections: [b.sections[0], b.sections[2]],
        controls,
        readouts: vec![],
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;

#[cfg(test)]
use daw_unit_ui::knobs::ModulationMarker;
