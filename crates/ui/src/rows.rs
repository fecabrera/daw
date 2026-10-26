use egui::{Align, InnerResponse, Layout, Ui, Vec2};

pub const NAME_FONT_SIZE: f32 = 13.0;

/// Keep adjacent pane rows aligned even when text or frames round differently.
pub(super) fn fixed<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    InnerResponse {
        inner: content(&mut child),
        response,
    }
}

/// Reserve the tallest control's height before placing vertically centered items.
pub use daw_unit_ui::rows::centered;

/// Keep header text at the left inset while centering it vertically.
pub fn left_aligned<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center).with_main_align(Align::Min),
        |ui| {
            ui.set_min_height(height);
            content(ui)
        },
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

/// Preserve fixed row geometry and offscreen drop bounds without constructing controls.
/// Call only outside active interactions or text editing.
pub fn placeholder(
    ui: &mut Ui,
    id: impl egui::AsIdSalt,
    width: f32,
    height: f32,
    left_inset: f32,
) -> (egui::Rect, egui::Rect) {
    centered_with_id(ui, id, height, |ui| {
        let mut panel = ui
            .allocate_exact_size(Vec2::new(width, height), egui::Sense::hover())
            .0;
        panel.min.x += left_inset;
        let lane = ui
            .allocate_exact_size(
                Vec2::new(ui.available_width(), height),
                egui::Sense::hover(),
            )
            .0;
        (panel, lane)
    })
    .inner
}

/// Keep form controls aligned after a shared label column.
pub fn field<R>(
    ui: &mut Ui,
    label_width: f32,
    label: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    field_with_height(
        ui,
        label_width,
        crate::toolbars::CONTROL_HEIGHT,
        label,
        control,
    )
}

/// Reuse field alignment in panels whose rows also own vertical padding.
pub(super) fn field_with_height<R>(
    ui: &mut Ui,
    label_width: f32,
    height: f32,
    label: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    centered(ui, height, |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(label_width, height),
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
pub use daw_unit_ui::rows::unit_label;

/// Join related inputs inside one standard frame, retaining each widget's interaction.
pub fn input_group<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let palette = crate::theme::palette(ui.ctx());
    let border = ui.visuals().widgets.inactive.bg_stroke.width;
    let frame = egui::Frame::new()
        .fill(palette.input)
        .stroke(ui.visuals().widgets.inactive.bg_stroke)
        .corner_radius(ui.visuals().widgets.inactive.corner_radius);
    frame.show(ui, |ui| {
        // Include the outer border in the standard input height.
        ui.spacing_mut().interact_size.y =
            (crate::toolbars::CONTROL_HEIGHT - 2.0 * border).max(0.0);
        ui.spacing_mut().button_padding.y = (ui.spacing().button_padding.y - 2.0 * border).max(0.0);
        ui.spacing_mut().item_spacing.x = 0.0;
        // TextEdit otherwise adds its focused stroke outside the inner control.
        // Keep the selection text color; the group owns the input outline.
        ui.visuals_mut().selection.stroke.width = 0.0;
        let widgets = &mut ui.visuals_mut().widgets;
        for widget in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            widget.bg_stroke = egui::Stroke::NONE;
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
        ui.horizontal(content).inner
    })
}

/// Paint one input-group outline and divider with the standard focus/hover colors.
pub fn input_group_outline(ui: &Ui, rect: egui::Rect, divider: f32, controls: &[&egui::Response]) {
    let visuals = if controls.iter().any(|r| r.has_focus() || r.dragged()) {
        &ui.visuals().widgets.active
    } else if controls.iter().any(|r| r.hovered()) {
        &ui.visuals().widgets.hovered
    } else {
        &ui.visuals().widgets.inactive
    };
    ui.painter().rect_stroke(
        rect,
        visuals.corner_radius,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.painter().vline(
        divider,
        rect.top() + 1.0..=rect.bottom() - 1.0,
        egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
    );
}
