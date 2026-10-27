//! Logical geometry shared by unit shells and headerless plugin bodies.
pub const UNIT_STRIP_WIDTH: f32 = 90.0;
pub const PANEL_MARGIN: i8 = 10;
pub const COMPACT_BUTTON_SIZE: f32 = 18.0;
pub const HEADER_FONT_SIZE: f32 = 11.0;
/// Three standard knob rows and a footer, before margins and any host header.
pub const fn unit_control_height() -> f32 {
    3.0 * crate::knobs::CHOICE_SIZE
        + 3.0 * crate::theme::TOOLBAR_PADDING as f32
        + COMPACT_BUTTON_SIZE
}

use egui::{Frame, Stroke, Ui, Vec2};

pub fn unit_strip<R>(
    ui: &mut Ui,
    height: f32,
    builder: egui::UiBuilder,
    body: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<egui::InnerResponse<R>> {
    unit_strip_with_width(ui, height, UNIT_STRIP_WIDTH, builder, body)
}

pub const fn unit_column_gap() -> f32 {
    (UNIT_STRIP_WIDTH - crate::knobs::CHOICE_SIZE) * 0.5
}

pub const fn unit_columns_width(columns: usize) -> f32 {
    columns as f32 * crate::knobs::CHOICE_SIZE + (columns + 1) as f32 * unit_column_gap()
}

pub fn unit_columns(ui: &mut Ui, columns: usize, mut body: impl FnMut(&mut Ui, usize)) {
    ui.horizontal_top(|ui| {
        let gap = unit_column_gap();
        ui.spacing_mut().item_spacing.x = gap;
        ui.add_space(gap - f32::from(PANEL_MARGIN));
        for column in 0..columns {
            ui.push_id(column, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(crate::knobs::CHOICE_SIZE, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(crate::knobs::CHOICE_SIZE);
                        body(ui, column);
                    },
                );
            });
        }
    });
}

pub fn unit_strip_with_width<R>(
    ui: &mut Ui,
    height: f32,
    width: f32,
    builder: egui::UiBuilder,
    body: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<egui::InnerResponse<R>> {
    let margin = PANEL_MARGIN;
    Frame::new().inner_margin(margin).show(ui, |ui| {
        ui.scope_builder(
            builder.layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_width(width - 2.0 * f32::from(margin));
                ui.set_min_height(height);
                body(ui)
            },
        )
    })
}

pub fn unit_divider(ui: &Ui, x: f32, range: std::ops::RangeInclusive<f32>) {
    ui.painter().vline(
        x,
        range,
        Stroke::new(1.0, crate::theme::unit_separator(ui.ctx())),
    );
}

/// The same full-strip Plus control is used by built-in variable-band units and plugins.
pub fn unit_add_strip(ui: &mut Ui, height: f32, name: &str) -> egui::InnerResponse<egui::Response> {
    let margin = f32::from(PANEL_MARGIN);
    ui.scope(|ui| {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(COMPACT_BUTTON_SIZE, height) + Vec2::splat(2.0 * margin),
            egui::Sense::click(),
        );
        let visuals = &ui.style().visuals;
        let hovered = ui.is_enabled()
            && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on());
        if hovered {
            ui.painter()
                .rect_filled(rect, 0.0, visuals.widgets.hovered.weak_bg_fill);
        }
        let circle = egui::Rect::from_center_size(rect.center(), Vec2::splat(COMPACT_BUTTON_SIZE));
        let palette = crate::theme::palette(ui.ctx());
        ui.painter().circle(
            circle.center(),
            (COMPACT_BUTTON_SIZE - 1.0) * 0.5,
            if hovered {
                visuals.widgets.hovered.weak_bg_fill
            } else {
                egui::Color32::TRANSPARENT
            },
            Stroke::new(1.0, palette.secondary),
        );
        let icon = COMPACT_BUTTON_SIZE * 14.0 / 22.0;
        let half = icon * 7.0 / 24.0;
        for axis in [egui::vec2(half, 0.0), egui::vec2(0.0, half)] {
            ui.painter().line_segment(
                [circle.center() - axis, circle.center() + axis],
                Stroke::new(icon * 2.0 / 24.0, palette.secondary),
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name)
        });
        response
            .on_hover_text(name)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
}
