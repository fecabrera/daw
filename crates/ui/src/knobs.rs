pub use daw_unit_ui::knobs::*;

pub(super) const HEADER_SIZE: f32 = 22.0;

fn header_slot(
    ui: &mut egui::Ui,
    knob: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    // Keep control centers, numeric inputs and header row height at their original positions.
    let (slot, _) = ui.allocate_exact_size(egui::Vec2::splat(SIZE), egui::Sense::hover());
    let mut control = ui.new_child(
        egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(
            slot.center(),
            egui::Vec2::splat(HEADER_SIZE),
        )),
    );
    knob(&mut control)
}

pub(super) fn header_gain(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    header_slot(ui, |ui| gain_scaled(ui, value, HEADER_SIZE / SIZE))
}

pub(super) fn header_pan(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    header_slot(ui, |ui| pan_scaled(ui, value, HEADER_SIZE / SIZE))
}
