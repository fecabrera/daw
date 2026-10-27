use egui::{Color32, Context, Response, Ui, Vec2};
pub use egui_lucide::Lucide;

pub use daw_unit_ui::layout::COMPACT_BUTTON_SIZE;
pub const TOOLBAR_BUTTON_SIZE: f32 = 24.0;
pub const TOOLBAR_ICON_SIZE: f32 = 16.0;
pub(super) const HEADER_ICON_WIDTH: f32 = 28.0;

fn disclosure_icon(open: bool) -> Lucide {
    if open {
        Lucide::ChevronDown
    } else {
        Lucide::ChevronRight
    }
}

fn paint_header_icon(ui: &Ui, rect: egui::Rect, icon: Lucide, color: Color32) {
    icon.size(TOOLBAR_ICON_SIZE)
        .color(color)
        .image()
        .paint_at(ui, rect);
}

/// Shared borderless disclosure artwork for catalog folders and track groups.
pub(super) fn paint_disclosure(ui: &Ui, rect: egui::Rect, open: bool, color: Color32) {
    paint_header_icon(ui, rect, disclosure_icon(open), color);
}

/// Leading track/layer artwork shares geometry and colors with the group disclosure.
pub(super) fn header_icon(ui: &mut Ui, icon: Lucide, sense: egui::Sense) -> Response {
    header_icon_with_rotation(ui, icon, sense, 0.0)
}

/// Rotate header artwork clockwise without changing its layout or hit area.
pub(super) fn header_icon_with_rotation(
    ui: &mut Ui,
    icon: Lucide,
    sense: egui::Sense,
    angle: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(HEADER_ICON_WIDTH, crate::toolbars::CONTROL_HEIGHT),
        sense,
    );
    let palette = crate::theme::palette(ui.ctx());
    let color = if ui.is_enabled() && (response.hovered() || response.has_focus()) {
        palette.text
    } else {
        palette.secondary
    };
    let image = icon.size(TOOLBAR_ICON_SIZE).color(color).image();
    let image = if angle == 0.0 {
        image
    } else {
        image.rotate(angle, Vec2::splat(0.5))
    };
    image.paint_at(
        ui,
        egui::Rect::from_center_size(rect.center(), Vec2::splat(TOOLBAR_ICON_SIZE)),
    );
    response
}

pub(super) fn disclosure_button(ui: &mut Ui, open: bool, name: &str, enabled: bool) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let response = header_icon(ui, disclosure_icon(open), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::CollapsingHeader,
                ui.is_enabled(),
                open,
                name,
            )
        });
        response
            .on_hover_text(name)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

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

/// User-supplied fit-width geometry. A white SVG mask takes the same theme tint
/// as the Lucide controls while keeping one embedded image for both themes.
pub(super) fn fit_clip_button(ui: &mut Ui) -> Response {
    fit_width_button(ui, "Fit clip")
}

pub(super) fn fit_width_button(ui: &mut Ui, name: &str) -> Response {
    let image = egui::Image::new(egui::include_image!("../assets/icons/fit-clip.svg"))
        .fit_to_exact_size(Vec2::splat(TOOLBAR_ICON_SIZE))
        .tint(crate::theme::palette(ui.ctx()).text)
        .alt_text(name);
    image_button(
        ui,
        image,
        name,
        true,
        Vec2::splat(TOOLBAR_BUTTON_SIZE),
        None,
    )
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

/// Borderless toolbar toggle. Unselected buttons use the shared gray hover overlay.
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
    if !selected && ui.is_enabled() && (response.hovered() || response.is_pointer_button_down_on())
    {
        ui.painter()
            .rect_filled(surface, 2.0, crate::theme::TOOLBAR_HOVER_FILL);
        color = palette.text;
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
pub use daw_unit_ui::knobs::paint_dot_toggle;

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
    image_button(ui, image, name, enabled, min_size, selected)
}

fn image_button(
    ui: &mut Ui,
    image: egui::Image<'_>,
    name: &str,
    enabled: bool,
    min_size: Vec2,
    selected: Option<bool>,
) -> Response {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_toggles_preserve_selected_fill_on_hover_and_press() {
        for colors in [crate::theme::DARK, crate::theme::LIGHT, crate::theme::MIST] {
            for selected in [false, true] {
                for enabled in [false, true] {
                    let ctx = egui::Context::default();
                    crate::theme::configure(&ctx);
                    daw_unit_ui::theme::apply(
                        &ctx,
                        if colors == crate::theme::DARK {
                            egui::Theme::Dark
                        } else {
                            egui::Theme::Light
                        },
                        colors,
                        crate::theme::PLUGIN_ACCENT,
                    );
                    configure(&ctx);
                    let draw = |events| {
                        let mut rect = egui::Rect::NOTHING;
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::splat(200.0),
                                )),
                                focused: true,
                                events,
                                ..Default::default()
                            },
                            |ui| {
                                rect = ui
                                    .add_enabled_ui(enabled, |ui| {
                                        borderless_toggle(
                                            ui,
                                            Lucide::Library,
                                            "Library",
                                            selected,
                                            Vec2::splat(TOOLBAR_BUTTON_SIZE),
                                        )
                                    })
                                    .inner
                                    .rect;
                            },
                        );
                        output.textures_delta.clear();
                        (rect, output.shapes)
                    };
                    let (rect, _) = draw(vec![]);
                    let point = rect.center();
                    for pressed in [None, Some(true), Some(false)] {
                        let mut events = vec![egui::Event::PointerMoved(point)];
                        if let Some(pressed) = pressed {
                            events.push(egui::Event::PointerButton {
                                pos: point,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: Default::default(),
                            });
                        }
                        let (current, shapes) = draw(events);
                        assert_eq!(current, rect);
                        let fill = if selected {
                            crate::theme::toggle_colors(&ctx, true).0
                        } else {
                            Color32::TRANSPARENT
                        };
                        if enabled {
                            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                                egui::Shape::Rect(r) if r.rect == rect && r.fill == fill && r.stroke == egui::Stroke::NONE)),
                                "selected={selected}, enabled={enabled}, pressed={pressed:?}");
                        }
                        assert_eq!(shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(r) if r.rect == rect && r.fill == crate::theme::TOOLBAR_HOVER_FILL)),
                            enabled && !selected);
                    }
                }
            }
        }
    }
}
