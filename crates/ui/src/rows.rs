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
    power_label_with(ui, label, name, on, |ui, label, width| {
        ui.add_sized(
            Vec2::new(width, crate::icons::COMPACT_BUTTON_SIZE),
            egui::Label::new(label).selectable(false).truncate(),
        )
    })
    .0
}

/// Reuse the footer geometry with an interactive label or inline name editor.
pub(super) fn power_label_with(
    ui: &mut Ui,
    label: &str,
    name: &str,
    on: bool,
    label_ui: impl FnOnce(&mut Ui, egui::RichText, f32) -> egui::Response,
) -> (egui::Response, egui::Response) {
    let palette = crate::theme::palette(ui.ctx());
    let font = egui::FontId::proportional(crate::panels::HEADER_FONT_SIZE);
    let color = if on { palette.text } else { palette.secondary };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), color);
    let gap = f32::from(crate::theme::TOOLBAR_PADDING);
    let label_width = galley
        .size()
        .x
        .min((ui.available_width() - crate::icons::COMPACT_BUTTON_SIZE - gap).max(0.0));
    let pair_width = crate::icons::COMPACT_BUTTON_SIZE + gap + label_width;
    ui.spacing_mut().item_spacing.x = gap;
    ui.add_space(((ui.available_width() - pair_width) * 0.5).max(0.0).round());
    let response = crate::icons::circular_button(
        ui,
        crate::icons::Lucide::Power,
        name,
        Some(on),
        crate::icons::COMPACT_BUTTON_SIZE,
    );
    let label_response = label_ui(
        ui,
        egui::RichText::new(label).font(font).color(color),
        label_width,
    );
    (response, label_response)
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
