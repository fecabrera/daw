use crate::{fonts, theme};
use egui::{Frame, InnerResponse, RichText, Ui, Vec2};

pub const CONTROL_HEIGHT: f32 = 22.0;
pub(super) const SLIDER_RAIL_HEIGHT: f32 = 4.0;
pub const HEIGHT: f32 = CONTROL_HEIGHT + 2.0 * theme::TOOLBAR_PADDING as f32;

/// Menu shortcuts use Command's symbol on macOS even when other modifier glyphs are absent.
pub fn format_menu_shortcut(ctx: &egui::Context, shortcut: &egui::KeyboardShortcut) -> String {
    if ctx.os().is_mac() {
        shortcut.format(
            &egui::ModifierNames {
                is_short: true,
                mac_cmd: "⌘",
                ctrl: "Ctrl+",
                shift: "Shift+",
                alt: "Alt+",
                mac_alt: "Option+",
                concat: "",
            },
            true,
        )
    } else {
        ctx.format_shortcut(shortcut)
    }
}

/// Keep shortcut letters in Outfit and size Command's fallback glyph to match them.
pub fn menu_shortcut_text(ctx: &egui::Context, shortcut: &egui::KeyboardShortcut) -> RichText {
    let text = RichText::new(format_menu_shortcut(ctx, shortcut));
    if ctx.os().is_mac() {
        text.family(fonts::menu_shortcut_family())
    } else {
        text
    }
}

/// Square single-letter toggles share the standard control height in both views.
pub fn square_toggle(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    daw_unit_ui::rows::square_toggle(ui, label, selected)
}

/// Shared toolbar frame and control spacing. Callers supply their controls.
pub struct Toolbar;

impl Toolbar {
    pub fn show<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        Self::show_with_control_height(ui, CONTROL_HEIGHT, content)
    }

    fn show_with_control_height<R>(
        ui: &mut Ui,
        height: f32,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let padding = ((HEIGHT - height) * 0.5).max(0.0) as i8;
        Frame::new()
            .fill(theme::palette(ui.ctx()).panel)
            .inner_margin(padding)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(height);
                ui.spacing_mut().item_spacing = Vec2::new(f32::from(padding), 5.0);
                ui.spacing_mut().interact_size.y = CONTROL_HEIGHT;
                content(ui)
            })
    }

    pub fn row<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        Self::row_with_control_height(ui, CONTROL_HEIGHT, content)
    }

    pub fn row_with_control_height<R>(
        ui: &mut Ui,
        height: f32,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        Self::show_with_control_height(ui, height, |ui| {
            crate::rows::centered(ui, height, content).inner
        })
    }
}

/// Standard compact horizontal zoom slider and icon, laid out right to left by the host.
pub fn horizontal_zoom(
    ui: &mut Ui,
    zoom: &mut f32,
    limits: std::ops::RangeInclusive<f32>,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = 56.0;
            ui.spacing_mut().slider_rail_height = SLIDER_RAIL_HEIGHT;
            ui.spacing_mut().interact_size.y = 14.0;
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(11.0));
            let response = ui.add(
                egui::Slider::new(zoom, limits)
                    .logarithmic(true)
                    .show_value(false)
                    .trailing_fill(false)
                    .handle_shape(egui::style::HandleShape::Circle),
            );
            response.widget_info(|| {
                egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*zoom), "Horizontal zoom")
            });
            response.on_hover_text(format!("Horizontal zoom: {:.1}", *zoom))
        })
        .inner;
    ui.add(
        crate::icons::Lucide::MoveHorizontal
            .size(16.0)
            .stroke_width(2.0)
            .color(theme::palette(ui.ctx()).text)
            .image()
            .alt_text("Horizontal zoom"),
    )
    .on_hover_text(format!("Horizontal zoom: {:.1}", *zoom));
    response
}

/// Continuous Timeline track height, using the horizontal zoom control's geometry.
pub(super) fn track_size(ui: &mut Ui, size: &mut f32) -> egui::Response {
    let response = ui
        .push_id("track-height-slider", |ui| {
            ui.spacing_mut().slider_width = 56.0;
            ui.spacing_mut().slider_rail_height = SLIDER_RAIL_HEIGHT;
            ui.spacing_mut().interact_size.y = 14.0;
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(11.0));
            let pointer_input = ui.input(|i| {
                i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
            });
            let response = ui.add(
                egui::Slider::from_get_set(1.0..=4.0, |value| {
                    if let Some(value) = value {
                        let value = value as f32;
                        let whole = value.round();
                        // Quantize before painting so the handle stays at the snapped value.
                        *size = if pointer_input && (value - whole).abs() <= 0.15 {
                            whole
                        } else {
                            value
                        };
                    }
                    f64::from(*size)
                })
                .step_by(0.01)
                .smart_aim(false)
                .show_value(false)
                .trailing_fill(false)
                .handle_shape(egui::style::HandleShape::Circle),
            );
            if response.interact_pointer_pos().is_some() {
                response.request_focus();
            }
            let label = format!("{size:.2}");
            let label = label.trim_end_matches('0').trim_end_matches('.');
            response.widget_info(|| {
                egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*size), "Track height")
            });
            response.on_hover_text(format!("Track height: {label}"))
        })
        .inner;
    ui.add(
        crate::icons::Lucide::MoveVertical
            .size(16.0)
            .stroke_width(2.0)
            .color(theme::palette(ui.ctx()).text)
            .image()
            .alt_text("Track height"),
    )
    .on_hover_text(format!(
        "Track height: {}",
        format!("{size:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
    ));
    response
}

/// Text only: callers retain ownership of action dispatch and keyboard bindings.
pub fn action_tooltip(
    ctx: &egui::Context,
    name: &str,
    shortcut: Option<egui::KeyboardShortcut>,
) -> String {
    let formatted = shortcut.map(|shortcut| format_menu_shortcut(ctx, &shortcut));
    daw_unit_ui::tooltips::action(name, formatted.as_deref())
}
