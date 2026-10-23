use crate::{fonts, theme};
use egui::{Frame, InnerResponse, RichText, Ui, Vec2};

pub const CONTROL_HEIGHT: f32 = 22.0;
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
    ui.scope(|ui| {
        // Keep the single-letter label compact without widening the button.
        ui.spacing_mut().button_padding.x = 3.0;
        ui.add(
            egui::Button::new(RichText::new(label).small())
                .selected(selected)
                .min_size(Vec2::splat(CONTROL_HEIGHT)),
        )
    })
    .inner
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
            ui.spacing_mut().slider_rail_height = 4.0;
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
            response.on_hover_text("Horizontal zoom: drag left to zoom out, right to zoom in")
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
    .on_hover_text("Horizontal zoom");
    response
}
