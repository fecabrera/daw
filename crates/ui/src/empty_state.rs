use crate::{theme, toolbars};
use egui::{Align, FontId, Pos2, Rect, Response, Stroke, TextStyle, Ui, Vec2};

pub(super) const MARGIN: f32 = 20.0;
pub(super) const WIDTH: f32 = 240.0;

pub(super) struct EmptyStateResponse {
    pub area: Rect,
    pub action: Response,
}

/// An inset, dashed card with descriptive text and one explicit action.
pub(super) fn show(
    ui: &mut Ui,
    height: f32,
    title: &str,
    description: &str,
    action: &str,
) -> EmptyStateResponse {
    let (area, _) = ui.allocate_exact_size(Vec2::new(WIDTH, height), egui::Sense::hover());
    let card = area.shrink(MARGIN);
    let palette = theme::palette(ui.ctx());
    dashed_outline(ui, card.shrink(0.5), Stroke::new(1.0, palette.border));

    let title_font = TextStyle::Heading.resolve(ui.style());
    let body_font = TextStyle::Body.resolve(ui.style());
    let width = card.width() - 24.0;
    let title_height = text_height(ui, title, title_font.clone(), width);
    let body_height = text_height(ui, description, body_font.clone(), width);
    let content_height = title_height + body_height + toolbars::CONTROL_HEIGHT + 24.0;
    let content = Rect::from_center_size(card.center(), Vec2::new(width, content_height));
    let mut body = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("empty-state", action))
            .max_rect(content)
            .layout(egui::Layout::top_down(Align::Center)),
    );
    body.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);
    body.add(
        egui::Label::new(
            egui::RichText::new(title)
                .font(title_font)
                .color(palette.text),
        )
        .wrap()
        .halign(Align::Center),
    );
    body.add(
        egui::Label::new(
            egui::RichText::new(description)
                .font(body_font)
                .color(palette.secondary),
        )
        .wrap()
        .halign(Align::Center),
    );
    let action =
        body.add(egui::Button::new(action).min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)));
    EmptyStateResponse { area, action }
}

fn text_height(ui: &Ui, text: &str, font: FontId, width: f32) -> f32 {
    ui.painter()
        .layout(text.to_owned(), font, theme::palette(ui.ctx()).text, width)
        .size()
        .y
}

fn dashed_outline(ui: &Ui, rect: Rect, stroke: Stroke) {
    let radius = 7.5;
    let mut points = Vec::with_capacity(29);
    for (center, start) in [
        (
            Pos2::new(rect.right() - radius, rect.top() + radius),
            -std::f32::consts::FRAC_PI_2,
        ),
        (
            Pos2::new(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            Pos2::new(rect.left() + radius, rect.bottom() - radius),
            std::f32::consts::FRAC_PI_2,
        ),
        (
            Pos2::new(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
    ] {
        for step in 0..=6 {
            let angle = start + step as f32 * std::f32::consts::FRAC_PI_2 / 6.0;
            points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
        }
    }
    points.push(points[0]);
    ui.painter()
        .extend(egui::Shape::dashed_line(&points, stroke, 6.0, 4.0));
}
