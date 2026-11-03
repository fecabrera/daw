use crate::{meters, panels};
pub(super) use daw_unit_ui::loudness::Bounds;
pub(super) fn show(
    ui: &mut egui::Ui,
    s: &mut daw_core::LoudnessSettings,
    dimensions: (bool, f32),
    meter: Option<&daw_unit_core::LoudnessMeter>,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let mut enabled = s.enabled;
    let mut collapsed = s.collapsed;
    let mut bounds = Bounds::default();
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Loudness Meter", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            bounds = daw_unit_ui::loudness::show(
                ui,
                s,
                compact,
                crate::envelope_panel::body_height(compact),
                meter,
                level,
            );
        },
    );
    s.enabled = enabled;
    s.collapsed = collapsed;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
