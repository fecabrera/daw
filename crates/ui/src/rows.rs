use egui::{Align, InnerResponse, Layout, Ui, Vec2};

pub const NAME_FONT_SIZE: f32 = 13.0;

/// Reserve the tallest control's height before placing vertically centered items.
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

/// Keep row widget identities attached to an item when its position changes.
pub fn centered_with_id<R>(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.push_id(id_salt, |ui| centered(ui, height, content))
        .inner
}

/// Keep form controls aligned after a shared label column.
pub fn field<R>(
    ui: &mut Ui,
    label_width: f32,
    label: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    centered(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(label_width, crate::toolbars::CONTROL_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(label_width);
                label(ui);
            },
        );
        control(ui)
    })
}

pub fn wrapped<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center).with_main_wrap(true),
        content,
    )
}

/// Standard centered unit-footer pair: 18-point Power, 8-point gap, 11-point label.
/// The caller supplies state and handles the returned release action.
pub fn power_label(ui: &mut Ui, label: &str, name: &str, on: bool) -> egui::Response {
    let palette = crate::theme::palette(ui.ctx());
    let label = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(crate::panels::HEADER_FONT_SIZE),
        if on { palette.text } else { palette.secondary },
    );
    let gap = f32::from(crate::theme::TOOLBAR_PADDING);
    let pair_width = crate::icons::COMPACT_BUTTON_SIZE + gap + label.size().x;
    ui.spacing_mut().item_spacing.x = gap;
    ui.add_space(((ui.available_width() - pair_width) * 0.5).max(0.0).round());
    let response = crate::icons::circular_button(
        ui,
        crate::icons::Lucide::Power,
        name,
        Some(on),
        crate::icons::COMPACT_BUTTON_SIZE,
    );
    ui.add(egui::Label::new(label).selectable(false));
    response
}

/// Center a standard unit footer label in its reserved row.
pub fn unit_label(ui: &mut Ui, name: &str) -> egui::Response {
    let label = ui.painter().layout_no_wrap(
        name.into(),
        egui::FontId::proportional(crate::panels::HEADER_FONT_SIZE),
        crate::theme::palette(ui.ctx()).text,
    );
    ui.add_space(
        ((ui.available_width() - label.size().x) * 0.5)
            .max(0.0)
            .round(),
    );
    ui.add(egui::Label::new(label).selectable(false))
}
