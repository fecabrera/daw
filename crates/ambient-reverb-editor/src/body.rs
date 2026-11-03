use daw_drive_plugin::Parameter as P;
use daw_unit_ui::{control_capture, layout::*, meters};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    daw_unit_ui::ambient_reverb::STRIP_COUNT as f32 * UNIT_STRIP_WIDTH,
    unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; daw_unit_ui::ambient_reverb::STRIP_COUNT],
    pub controls: [Option<Response>; P::COUNT - 1],
    pub readouts: Vec<(u32, Response)>,
}
pub fn render(
    ui: &mut Ui,
    s: &mut daw_unit_core::AmbientReverbSettings,
    level: meters::OutputLevel<'_>,
    _: meters::GainReduction<'_>,
    _: meters::BandGainChange<'_>,
) -> Bounds {
    let before = daw_drive_plugin::PluginState::from_settings(s).unwrap();
    let (b, c) = control_capture::scope(ui, |ui| {
        daw_unit_ui::ambient_reverb::show(
            ui,
            s,
            false,
            unit_control_height(),
            &std::array::from_fn::<_, 8, _>(|_| None),
            level,
        )
    });
    Bounds {
        sections: b.sections,
        controls: std::array::from_fn(|i| {
            control_capture::at(&c, b.controls[i]).map(|mut r| {
                let parameter = P::ALL[i + 1];
                if before.get(parameter)
                    != daw_drive_plugin::PluginState::from_settings(s)
                        .unwrap()
                        .get(parameter)
                {
                    r.mark_changed();
                }
                r
            })
        }),
        readouts: vec![],
    }
}

#[cfg(test)]
pub const SCROLLABLE: bool = false;
#[cfg(test)]
pub fn show(
    ui: &mut Ui,
    s: &mut daw_unit_core::AmbientReverbSettings,
    _: bool,
    _: f32,
    _: &[Option<daw_unit_ui::knobs::ModulationMarker>; P::COUNT - 1],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    render(ui, s, level, 0.0.into(), [0.0; 3].into())
}
