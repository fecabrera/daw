pub use daw_unit_ui::knobs::*;

pub(super) const HEADER_SIZE: f32 = 22.0;

// The standard Pan's reference tick reaches 0.75 points above its hit slot;
// the lower face ends 4.5 points before the slot bottom. Keep visible layout
// separate from the shared knob's unchanged interaction rectangle.
pub(super) const PAN_VISIBLE_HEIGHT: f32 = SIZE - 3.75;
pub(super) fn pan_visible(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    let (face, _) =
        ui.allocate_exact_size(egui::vec2(SIZE, PAN_VISIBLE_HEIGHT), egui::Sense::hover());
    let hit = egui::Rect::from_min_size(face.min + egui::vec2(0.0, 0.75), egui::Vec2::splat(SIZE));
    let mut control = ui.new_child(egui::UiBuilder::new().id_salt("pan_face").max_rect(hit));
    pan(&mut control, value)
}

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

pub(super) fn header_send_gain(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    header_slot(ui, |ui| send_gain_scaled(ui, value, HEADER_SIZE / SIZE))
}
