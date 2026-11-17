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
        crate::fonts::semibold(crate::layout::HEADER_FONT_SIZE),
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
    let font = crate::fonts::semibold(crate::layout::HEADER_FONT_SIZE);
    let color = if on { palette.text } else { palette.secondary };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let size = crate::layout::COMPACT_BUTTON_SIZE;
    // Retain the full label when only the usual gap exceeds the strip width.
    let gap = f32::from(crate::theme::TOOLBAR_PADDING)
        .min((ui.available_width() - size - galley.size().x).max(0.0));
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
        fill = crate::theme::borderless_control_fill(ui.ctx());
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
            ui.add(
                egui::Label::new(
                    egui::RichText::new(label)
                        .size(crate::layout::HEADER_FONT_SIZE)
                        .color(crate::theme::palette(ui.ctx()).secondary),
                )
                .truncate(),
            )
            .on_hover_text(label);
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

/// Filled, centered square choice, matching the joined control surface.
pub fn filled_square_toggle(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    grouped_square_toggle(ui, label, selected, egui::CornerRadius::same(5))
}
fn grouped_square_toggle(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    corners: egui::CornerRadius,
) -> egui::Response {
    let rect = egui::Rect::from_min_size(ui.available_rect_before_wrap().min, Vec2::splat(22.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
        |ui| {
            let fill = crate::theme::unit_header_fill(ui.ctx(), false);
            let widgets = &mut ui.visuals_mut().widgets;
            for widget in [
                &mut widgets.noninteractive,
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
            ] {
                widget.bg_stroke = egui::Stroke::NONE;
                widget.corner_radius = corners;
            }
            ui.visuals_mut().widgets.inactive.weak_bg_fill = fill;
            square_toggle(ui, label, selected)
        },
    )
    .inner
}

/// Joined vertical choices. Only the outside corners are rounded; gaps are dividers.
/// The caller owns selection and each returned response's action/tooltip.
pub fn vertical_toggle_group(
    ui: &mut Ui,
    labels: &[(egui::Id, String)],
    selected: Option<usize>,
) -> InnerResponse<Vec<egui::Response>> {
    egui::Frame::new()
        .fill(crate::theme::unit_separator(ui.ctx()))
        .corner_radius(5)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                labels
                    .iter()
                    .enumerate()
                    .map(|(index, (id, label))| {
                        ui.push_id(*id, |ui| {
                            let corners = egui::CornerRadius {
                                nw: if index == 0 { 5 } else { 0 },
                                ne: if index == 0 { 5 } else { 0 },
                                sw: if index + 1 == labels.len() { 5 } else { 0 },
                                se: if index + 1 == labels.len() { 5 } else { 0 },
                            };
                            grouped_square_toggle(ui, label, selected == Some(index), corners)
                        })
                        .inner
                    })
                    .collect()
            })
            .inner
        })
}

/// Temporary band audition control; this response is never a parameter edit.
pub fn band_solo(ui: &mut egui::Ui, label: &str, selected: &mut bool) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.y = crate::layout::COMPACT_BUTTON_SIZE;
            ui.spacing_mut().button_padding = egui::vec2(3.0, 0.0);
            ui.add(
                egui::Button::new(egui::RichText::new("S").small())
                    .selected(*selected)
                    .min_size(egui::Vec2::splat(crate::layout::COMPACT_BUTTON_SIZE)),
            )
        })
        .inner
        .on_hover_text(if *selected {
            "Stop band audition"
        } else {
            "Audition band"
        });
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), *selected, label)
    });
    if response.clicked() {
        *selected = !*selected;
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_footer_labels_use_the_semibold_font() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_width(200.0);
            unit_label(ui, "Output");
            power_label(ui, "Color", "Toggle color", true);
        });
        let expected = crate::fonts::semibold(crate::layout::HEADER_FONT_SIZE);
        for label in ["Output", "Color"] {
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing footer label: {label}"));
            assert!(
                text.galley
                    .job
                    .sections
                    .iter()
                    .all(|section| section.format.font_id == expected),
                "Footer label does not use the semibold font: {label}"
            );
        }
        output.textures_delta.clear();
    }
}
