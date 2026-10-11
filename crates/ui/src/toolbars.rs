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

/// Shared toolbar frame and control spacing. Callers supply their controls.
pub struct Toolbar;

impl Toolbar {
    pub fn show<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        Frame::new()
            .fill(theme::palette(ui.ctx()).panel)
            .inner_margin(theme::TOOLBAR_PADDING)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(CONTROL_HEIGHT);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                ui.spacing_mut().interact_size.y = CONTROL_HEIGHT;
                content(ui)
            })
    }

    pub fn row<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        Self::show(ui, |ui| {
            crate::rows::centered(ui, CONTROL_HEIGHT, content).inner
        })
    }
}
