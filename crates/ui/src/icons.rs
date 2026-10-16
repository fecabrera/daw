use egui::{Color32, Context, Response, Ui, Vec2};
pub use egui_lucide::Lucide;

pub const COMPACT_BUTTON_SIZE: f32 = 18.0;
pub const TOOLBAR_BUTTON_SIZE: f32 = 24.0;
pub const TOOLBAR_ICON_SIZE: f32 = 16.0;

pub fn configure(ctx: &Context) {
    egui_extras::install_image_loaders(ctx);
}

/// A compact, unframed reorder handle with the app's standard icon styling.
pub fn grip(ui: &mut Ui) -> Response {
    reorder_grip(ui, "track")
}

pub fn reorder_grip(ui: &mut Ui, item: &str) -> Response {
    let name = format!("Reorder {item}");
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
        .alt_text(&name)
        .paint_at(
            ui,
            egui::Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name)
    });
    let cursor = if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response
        .on_hover_text(format!("Drag to reorder {item}"))
        .on_hover_cursor(cursor)
}

/// Shared icon button with a visible tooltip and an accessibility name.
pub fn button(ui: &mut Ui, icon: Lucide, name: &str, enabled: bool, color: Color32) -> Response {
    draw_button(ui, icon, name, enabled, color, None, TOOLBAR_ICON_SIZE)
}

/// Match small-text toggles while retaining the standard button and hit area.
pub fn small_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    enabled: bool,
    color: Color32,
) -> Response {
    let icon_size = egui::TextStyle::Small.resolve(ui.style()).size;
    draw_button(ui, icon, name, enabled, color, None, icon_size)
}

/// Borderless toolbar toggle. A shared gray overlay changes contrast in either theme.
pub fn borderless_toggle(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    selected: bool,
    size: Vec2,
) -> Response {
    let fill = crate::theme::toggle_colors(ui.ctx(), true).0;
    borderless_toggle_with_fill(ui, icon, name, selected, size, fill)
}

/// Borderless toggle with a semantic active-state fill and contrasting icon.
pub fn borderless_toggle_with_fill(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    selected: bool,
    size: Vec2,
    active_fill: Color32,
) -> Response {
    borderless_toggle_with_colors(
        ui,
        icon,
        name,
        selected,
        size,
        (active_fill, crate::theme::contrast_color(active_fill)),
    )
}

/// Borderless toggle with caller-supplied active background and foreground.
pub fn borderless_toggle_with_colors(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    selected: bool,
    size: Vec2,
    active_colors: (Color32, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.is_pointer_button_down_on() || response.clicked() {
        response.request_focus();
    }
    let surface = rect;
    let palette = crate::theme::palette(ui.ctx());
    let (fill, mut color) = if selected {
        active_colors
    } else {
        (Color32::TRANSPARENT, palette.secondary)
    };
    ui.painter().rect_filled(surface, 2.0, fill);
    if ui.is_enabled() && (response.hovered() || response.is_pointer_button_down_on()) {
        ui.painter()
            .rect_filled(surface, 2.0, crate::theme::TOOLBAR_HOVER_FILL);
        if !selected {
            color = palette.text;
        }
    }
    if !ui.is_enabled() {
        color = palette.secondary;
    }
    icon.size(TOOLBAR_ICON_SIZE)
        .stroke_width(2.0)
        .color(color)
        .image()
        .alt_text(name)
        .paint_at(
            ui,
            egui::Rect::from_center_size(rect.center(), Vec2::splat(TOOLBAR_ICON_SIZE)),
        );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, name)
    });
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
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

/// Header dot toggle with the same painting as unit enable controls.
pub fn dot_toggle(
    ui: &mut Ui,
    name: &str,
    selected: bool,
    hit_size: Vec2,
    diameter: f32,
    color: egui::Color32,
) -> Response {
    let (_, response) = ui.allocate_exact_size(hit_size, egui::Sense::click());
    paint_dot_toggle(ui, &response, diameter, selected, color);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, name)
    });
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Paint into an existing response so unit shells retain their stable interaction IDs.
pub fn paint_dot_toggle(
    ui: &Ui,
    response: &Response,
    diameter: f32,
    selected: bool,
    color: egui::Color32,
) {
    let color = if ui.is_enabled() {
        color
    } else {
        color.gamma_multiply(0.5)
    };
    if response.hovered() || response.has_focus() {
        ui.painter().circle_filled(
            response.rect.center(),
            diameter * 0.5 + 3.0,
            color.gamma_multiply(0.15),
        );
    }
    ui.painter().circle(
        response.rect.center(),
        (diameter - 1.0) * 0.5,
        if selected {
            color
        } else {
            egui::Color32::TRANSPARENT
        },
        egui::Stroke::new(1.0, color),
    );
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
        TOOLBAR_ICON_SIZE,
    )
}

fn draw_button(
    ui: &mut Ui,
    icon: Lucide,
    name: &str,
    enabled: bool,
    color: Color32,
    selected: Option<bool>,
    icon_size: f32,
) -> Response {
    let image = icon
        .size(icon_size)
        .stroke_width(2.0)
        .color(color)
        .image()
        .alt_text(name);
    let mut min_size = Vec2::new(28.0, crate::toolbars::CONTROL_HEIGHT);
    if icon_size < TOOLBAR_ICON_SIZE {
        // Reserve the standard icon's space even when drawing a smaller icon.
        let font = egui::FontSelection::default().resolve(ui.style());
        let regular_size = TOOLBAR_ICON_SIZE.min(ui.fonts_mut(|fonts| fonts.row_height(&font)));
        min_size = min_size.max(Vec2::splat(regular_size) + 2.0 * ui.spacing().button_padding);
    }
    let mut button = egui::Button::image(image).min_size(min_size);
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
