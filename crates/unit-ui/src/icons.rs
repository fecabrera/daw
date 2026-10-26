use egui::{Response, Ui};
pub fn paint_dot_toggle(
    ui: &Ui,
    response: &Response,
    diameter: f32,
    selected: bool,
    color: egui::Color32,
) {
    let color = if ui.is_enabled() {
        color
    } else {
        color.gamma_multiply(0.5)
    };
    if response.hovered() || response.has_focus() {
        ui.painter().circle_filled(
            response.rect.center(),
            diameter * 0.5 + 3.0,
            color.gamma_multiply(0.15),
        );
    }
    ui.painter().circle(
        response.rect.center(),
        (diameter - 1.0) * 0.5,
        if selected {
            color
        } else {
            egui::Color32::TRANSPARENT
        },
        egui::Stroke::new(1.0, color),
    );
}
