use daw_drive_plugin::Parameter as P;
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    daw_unit_ui::loudness::WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 7],
    pub controls: [Option<Response>; P::COUNT - 1],
}
pub fn render(
    ui: &mut Ui,
    s: &mut daw_unit_core::LoudnessSettings,
    model: &crate::Model,
) -> Bounds {
    let before = daw_drive_plugin::PluginState::from_settings(s).unwrap();
    let (b, c) = control_capture::scope(ui, |ui| {
        daw_unit_ui::loudness::show(
            ui,
            s,
            false,
            unit_control_height(),
            Some(&model.output_meter.loudness),
            meters::OutputLevel::new(
                model.output_meter.readings()[0],
                Some(&model.output_meter.boost_peaks()[0]),
            )
            .with_reset_request(&model.output_reset),
        )
    });
    model
        .meter_reset
        .fetch_or(b.actions, std::sync::atomic::Ordering::Release);
    Bounds {
        sections: b.sections,
        controls: std::array::from_fn(|i| {
            control_capture::at(&c, b.controls[i]).map(|mut r| {
                if before.get(P::ALL[i + 1])
                    != daw_drive_plugin::PluginState::from_settings(s)
                        .unwrap()
                        .get(P::ALL[i + 1])
                {
                    r.mark_changed();
                }
                r
            })
        }),
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    s: &mut daw_unit_core::LoudnessSettings,
    _: bool,
    _: f32,
    _: &[Option<daw_unit_ui::knobs::ModulationMarker>; P::COUNT - 1],
    _: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, s, &crate::Model::default())
}
