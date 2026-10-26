use egui::{Align, InnerResponse, Layout, Ui, Vec2};
pub fn centered<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center),
        content,
    )
}

pub fn unit_label(ui: &mut Ui, name: &str) -> egui::Response {
    let label = ui.painter().layout_no_wrap(
        name.into(),
        egui::FontId::proportional(crate::layout::HEADER_FONT_SIZE),
        crate::theme::palette(ui.ctx()).text,
    );
    ui.add_space(
        ((ui.available_width() - label.size().x) * 0.5)
            .max(0.0)
            .round(),
    );
    ui.add(egui::Label::new(label).selectable(false))
}
