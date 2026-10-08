use crate::theme;
use egui::{Frame, InnerResponse, Ui, Vec2};

pub const CONTROL_HEIGHT: f32 = 22.0;
pub const HEIGHT: f32 = CONTROL_HEIGHT + 2.0 * theme::TOOLBAR_PADDING as f32;

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
