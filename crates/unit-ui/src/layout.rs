//! Logical geometry shared by unit shells and headerless plugin bodies.
pub const UNIT_STRIP_WIDTH: f32 = 90.0;
pub const PANEL_MARGIN: i8 = 10;
pub const COMPACT_BUTTON_SIZE: f32 = 18.0;
pub const HEADER_FONT_SIZE: f32 = 11.0;
pub const UNIT_STRIP_CORNER_RADIUS: u8 = 5;

/// Visible separation between neighboring strip cards.
pub const UNIT_STRIP_GAP: f32 = 4.0;
pub const UNIT_STRIP_VERTICAL_MARGIN: f32 = 4.0;

/// Inset the surface while retaining strip control, drag and reorder geometry.
pub fn unit_strip_rect(slot: egui::Rect) -> egui::Rect {
    slot.shrink2(Vec2::new(UNIT_STRIP_GAP * 0.5, UNIT_STRIP_VERTICAL_MARGIN))
}

/// Shared strip surface with padding independent of its visible card inset.
pub struct UnitStripFrame {
    frame: Frame,
    fill: egui::Color32,
}

pub fn unit_strip_frame(ui: &Ui) -> UnitStripFrame {
    UnitStripFrame {
        frame: Frame::new(),
        fill: crate::theme::palette(ui.ctx()).panel,
    }
}

impl UnitStripFrame {
    pub fn inner_margin(mut self, margin: impl Into<egui::Margin>) -> Self {
        self.frame = self.frame.inner_margin(margin);
        self
    }

    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        // Reserve the background before painting any child controls.
        let background = ui.painter().add(egui::Shape::Noop);
        let response = self.frame.show(ui, body);
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                unit_strip_rect(response.response.rect),
                UNIT_STRIP_CORNER_RADIUS,
                self.fill,
            ),
        );
        response
    }
}
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
    unit_strip_frame(ui).inner_margin(margin).show(ui, |ui| {
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

/// Transparent full-strip Add control shared by DAW units and standalone bodies.
pub fn unit_add_button(ui: &mut Ui, name: &str, size: Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let highlighted = ui.is_enabled()
        && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on());
    let palette = crate::theme::palette(ui.ctx());
    let foreground = if highlighted {
        palette.text
    } else {
        palette.secondary
    };
    let circle = egui::Rect::from_center_size(rect.center(), Vec2::splat(COMPACT_BUTTON_SIZE));
    ui.painter().circle(
        circle.center(),
        (COMPACT_BUTTON_SIZE - 1.0) * 0.5,
        egui::Color32::TRANSPARENT,
        Stroke::new(1.0, foreground),
    );
    let icon = COMPACT_BUTTON_SIZE * 14.0 / 22.0;
    let half = icon * 7.0 / 24.0;
    for axis in [egui::vec2(half, 0.0), egui::vec2(0.0, half)] {
        ui.painter().line_segment(
            [circle.center() - axis, circle.center() + axis],
            Stroke::new(icon * 2.0 / 24.0, foreground),
        );
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name));
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn unit_add_strip(ui: &mut Ui, height: f32, name: &str) -> egui::InnerResponse<egui::Response> {
    ui.scope(|ui| {
        unit_add_button(
            ui,
            name,
            Vec2::new(COMPACT_BUTTON_SIZE, height) + Vec2::splat(2.0 * f32::from(PANEL_MARGIN)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_cards_keep_geometry_and_use_borderless_rounded_panel_surfaces() {
        for (base, palette) in [
            (egui::Theme::Dark, crate::theme::DARK),
            (egui::Theme::Light, crate::theme::LIGHT),
            (egui::Theme::Light, crate::theme::MIST),
        ] {
            for height in [COMPACT_BUTTON_SIZE, unit_control_height()] {
                let ctx = egui::Context::default();
                crate::theme::configure(&ctx);
                crate::theme::apply(&ctx, base, palette, crate::theme::ACCENT);
                let mut cards = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(800.0, 400.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for width in [UNIT_STRIP_WIDTH, unit_columns_width(2)] {
                                cards.push(
                                    unit_strip_with_width(
                                        ui,
                                        height,
                                        width,
                                        egui::UiBuilder::new(),
                                        |_| {},
                                    )
                                    .response
                                    .rect,
                                );
                            }
                            cards.push(unit_add_strip(ui, height, "Add strip").response.rect);
                        });
                    },
                );
                output.textures_delta.clear();
                for (index, rect) in cards.iter().enumerate() {
                    assert_eq!(rect.height(), height + 2.0 * f32::from(PANEL_MARGIN));
                    let card = unit_strip_rect(*rect);
                    assert_eq!(card.top() - rect.top(), 4.0);
                    assert_eq!(rect.bottom() - card.bottom(), 4.0);
                    if index > 0 {
                        assert_eq!(card.left() - unit_strip_rect(cards[index - 1]).right(), 4.0);
                    }
                    if index < 2 {
                        assert!(output.shapes.iter().any(|shape| matches!(
                            &shape.shape, egui::Shape::Rect(card)
                            if card.rect == unit_strip_rect(*rect) && card.fill == palette.panel
                                && card.stroke.is_empty()
                                && card.corner_radius == egui::CornerRadius::same(5)
                        )));
                    } else {
                        assert!(!output.shapes.iter().any(|shape| matches!(
                            &shape.shape, egui::Shape::Rect(card)
                            if rect.contains_rect(card.rect) && card.fill != egui::Color32::TRANSPARENT
                        )));
                    }
                }
                assert_ne!(palette.panel, crate::theme::unit_separator(&ctx));
                assert!(!output.shapes.iter().any(|shape| matches!(
                    &shape.shape, egui::Shape::LineSegment { points, stroke }
                    if stroke.color == crate::theme::unit_separator(&ctx)
                        && (points[1].y - points[0].y).abs() >= height
                )));
            }
        }
    }
    #[test]
    fn add_buttons_stay_transparent_and_brighten_only_when_enabled() {
        for (base, palette) in [
            (egui::Theme::Dark, crate::theme::DARK),
            (egui::Theme::Light, crate::theme::LIGHT),
            (egui::Theme::Light, crate::theme::MIST),
        ] {
            for enabled in [false, true] {
                let ctx = egui::Context::default();
                crate::theme::configure(&ctx);
                crate::theme::apply(&ctx, base, palette, crate::theme::ACCENT);
                let draw = |events| {
                    let mut button = None;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(500.0, 400.0),
                            )),
                            events,
                            focused: true,
                            ..Default::default()
                        },
                        |ui| {
                            button = Some(
                                ui.add_enabled_ui(enabled, |ui| {
                                    unit_add_strip(ui, unit_control_height(), "Add test strip")
                                })
                                .inner
                                .inner,
                            );
                        },
                    );
                    output.textures_delta.clear();
                    (output.shapes, button.unwrap())
                };
                let paint = |shapes: &[egui::epaint::ClippedShape], rect: egui::Rect| {
                    assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(r) if rect.contains_rect(r.rect) && r.fill != egui::Color32::TRANSPARENT)));
                    let circle = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Circle(circle) if circle.center == rect.center() => {
                                Some(circle)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(circle.fill, egui::Color32::TRANSPARENT);
                    let plus: Vec<_> = shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::LineSegment { points, stroke }
                                if points.iter().all(|point| rect.contains(*point)) =>
                            {
                                Some(stroke.color)
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(plus.len(), 2);
                    assert!(plus.iter().all(|color| *color == circle.stroke.color));
                    circle.stroke.color
                };
                let (shapes, idle) = draw(vec![]);
                let idle_color = paint(&shapes, idle.rect);
                if enabled {
                    assert_eq!(idle_color, palette.secondary);
                }
                let point = idle.rect.min + Vec2::splat(2.0);
                let (shapes, hover) = draw(vec![egui::Event::PointerMoved(point)]);
                assert_eq!(hover.rect, idle.rect);
                assert_eq!(
                    paint(&shapes, hover.rect),
                    if enabled { palette.text } else { idle_color }
                );
                for pressed in [true, false] {
                    let (shapes, response) = draw(vec![egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    }]);
                    assert_eq!(
                        paint(&shapes, response.rect),
                        if enabled { palette.text } else { idle_color }
                    );
                    assert_eq!(response.clicked(), enabled && !pressed);
                }
                if enabled {
                    idle.request_focus();
                    let (shapes, focused) =
                        draw(vec![egui::Event::PointerMoved(egui::pos2(450.0, 350.0))]);
                    assert!(focused.has_focus());
                    assert_eq!(paint(&shapes, focused.rect), palette.text);
                }
            }
        }
    }
}
