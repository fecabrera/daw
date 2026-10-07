use egui::{Color32, Context, Response, Ui, Vec2};
pub use egui_lucide::Lucide;

pub fn configure(ctx: &Context) {
    egui_extras::install_image_loaders(ctx);
}

/// Shared icon button with a visible tooltip and an accessibility name.
pub fn button(ui: &mut Ui, icon: Lucide, name: &str, enabled: bool, color: Color32) -> Response {
    draw_button(ui, icon, name, enabled, color, None)
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
            crate::theme::ACCENT
        } else {
            crate::theme::TEXT
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
