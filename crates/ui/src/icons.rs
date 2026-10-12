use egui::{Color32, Context, Response, Ui, Vec2};
pub use egui_lucide::Lucide;

pub const COMPACT_BUTTON_SIZE: f32 = 18.0;

pub fn configure(ctx: &Context) {
    egui_extras::install_image_loaders(ctx);
}

/// A compact, unframed reorder handle with the app's standard icon styling.
pub fn grip(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(crate::toolbars::CONTROL_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    Lucide::Grip
        .size(16.0)
        .stroke_width(2.0)
        .color(if ui.is_enabled() {
            crate::theme::palette(ui.ctx()).secondary
        } else {
            crate::theme::palette(ui.ctx()).border
        })
        .image()
        .alt_text("Reorder track")
        .paint_at(
            ui,
            egui::Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Reorder track")
    });
    let cursor = if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response
        .on_hover_text("Drag to reorder track")
        .on_hover_cursor(cursor)
}

/// Shared icon button with a visible tooltip and an accessibility name.
pub fn button(ui: &mut Ui, icon: Lucide, name: &str, enabled: bool, color: Color32) -> Response {
    draw_button(ui, icon, name, enabled, color, None)
}

/// Shared circular control with accent fill when on and matching icon/border colors when off.
pub fn circular_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    selected: Option<bool>,
    size: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    draw_circular_button(ui, icon, name, selected, rect, response)
}

/// Full-strip interaction with the same centered circular control as standalone buttons.
pub fn circular_strip_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    size: Vec2,
    diameter: f32,
) -> Response {
    let (strip, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_enabled()
        && (response.hovered() || response.is_pointer_button_down_on() || response.has_focus())
    {
        ui.painter()
            .rect_filled(strip, 0.0, ui.style().visuals.widgets.hovered.weak_bg_fill);
    }
    let circle = egui::Rect::from_center_size(strip.center(), Vec2::splat(diameter));
    draw_circular_button(ui, icon, name, None, circle, response)
}

fn draw_circular_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    selected: Option<bool>,
    rect: egui::Rect,
    response: Response,
) -> Response {
    let palette = crate::theme::palette(ui.ctx());
    let (mut fill, _) =
        crate::theme::toggle_colors(ui.ctx(), selected == Some(true) && ui.is_enabled());
    if selected != Some(true)
        && ui.is_enabled()
        && (response.hovered() || response.is_pointer_button_down_on() || response.has_focus())
    {
        fill = ui.style().visuals.widgets.hovered.weak_bg_fill;
    }
    let color = if ui.is_enabled() && selected == Some(true) {
        crate::theme::accent(ui.ctx())
    } else {
        palette.secondary
    };
    ui.painter().circle(
        rect.center(),
        // Keep the painted outer diameter equal to the allocated control size.
        (rect.width() - 1.0) * 0.5,
        fill,
        egui::Stroke::new(1.0, color),
    );
    let icon_size = rect.width() * 14.0 / crate::toolbars::CONTROL_HEIGHT;
    icon.size(icon_size)
        .stroke_width(2.0)
        .color(color)
        .image()
        .alt_text(name)
        .paint_at(
            ui,
            egui::Rect::from_center_size(rect.center(), Vec2::splat(icon_size)),
        );
    response.widget_info(|| match selected {
        Some(on) => egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, name),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name),
    });
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Shared icon toggle with a selected appearance and accessible on/off state.
pub fn toggle_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    enabled: bool,
    selected: bool,
) -> Response {
    draw_button(
        ui,
        icon,
        name,
        enabled,
        if selected {
            crate::theme::accent(ui.ctx())
        } else {
            crate::theme::palette(ui.ctx()).text
        },
        Some(selected),
    )
}

fn draw_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    enabled: bool,
    color: Color32,
    selected: Option<bool>,
) -> Response {
    let image = icon
        .size(16.0)
        .stroke_width(2.0)
        .color(color)
        .image()
        .alt_text(name);
    let mut button =
        egui::Button::image(image).min_size(Vec2::new(28.0, crate::toolbars::CONTROL_HEIGHT));
    if let Some(selected) = selected {
        button = button.selected(selected);
    }
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| match selected {
        Some(selected) => egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled() && enabled,
            selected,
            name,
        ),
        None => {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && enabled, name)
        }
    });
    response.on_hover_text(name)
}
