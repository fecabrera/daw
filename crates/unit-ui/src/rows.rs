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

/// Standard Color footer. The button changes power; the label is not interactive.
pub fn power_label(ui: &mut Ui, label: &str, name: &str, on: bool) -> egui::Response {
    let palette = crate::theme::palette(ui.ctx());
    let font = egui::FontId::proportional(crate::layout::HEADER_FONT_SIZE);
    let color = if on { palette.text } else { palette.secondary };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let size = crate::layout::COMPACT_BUTTON_SIZE;
    let gap = f32::from(crate::theme::TOOLBAR_PADDING);
    let width = galley
        .size()
        .x
        .min((ui.available_width() - size - gap).max(0.0));
    ui.spacing_mut().item_spacing.x = gap;
    ui.add_space(
        ((ui.available_width() - size - gap - width) * 0.5)
            .max(0.0)
            .round(),
    );
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    let (mut fill, _) = crate::theme::toggle_colors(ui.ctx(), on && ui.is_enabled());
    if !on
        && ui.is_enabled()
        && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on())
    {
        fill = ui.visuals().widgets.hovered.weak_bg_fill;
    }
    let stroke_color = if on && ui.is_enabled() {
        crate::theme::accent(ui.ctx())
    } else {
        palette.secondary
    };
    ui.painter().circle(
        rect.center(),
        (size - 1.0) * 0.5,
        fill,
        egui::Stroke::new(1.0, stroke_color),
    );
    let radius = size * 0.19;
    let points: Vec<_> = (0..=24)
        .map(|i| {
            let angle = (-50.0 + i as f32 * 280.0 / 24.0).to_radians();
            rect.center() + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect();
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(1.2, stroke_color),
    ));
    ui.painter().line_segment(
        [rect.center() - Vec2::new(0.0, radius * 1.35), rect.center()],
        egui::Stroke::new(1.2, stroke_color),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, name)
    });
    ui.add_sized(
        Vec2::new(width, size),
        egui::Label::new(galley).selectable(false).truncate(),
    );
    crate::control_capture::record(ui, &response);
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn selector_row(
    ui: &mut Ui,
    label: &str,
    selector: impl FnOnce(&mut Ui) -> egui::Response,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), crate::knobs::CHOICE_SIZE),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_min_height(crate::knobs::CHOICE_SIZE);
            ui.spacing_mut().item_spacing.y = f32::from(crate::theme::TOOLBAR_PADDING) * 0.5;
            ui.label(
                egui::RichText::new(label)
                    .size(crate::layout::HEADER_FONT_SIZE)
                    .color(crate::theme::palette(ui.ctx()).secondary),
            );
            selector(ui)
        },
    )
    .inner
}

/// Shared track/layer/band single-letter toggle.
pub fn square_toggle(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    ui.scope(|ui| {
        // Keep the single-letter label compact without widening the button.
        ui.spacing_mut().button_padding.x = 3.0;
        ui.add(
            egui::Button::new(egui::RichText::new(label).small())
                .selected(selected)
                .min_size(Vec2::splat(22.0)),
        )
    })
    .inner
}
