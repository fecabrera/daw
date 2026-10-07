use egui::{Align2, Context, Frame, RichText, TextStyle, Vec2, Window};

const PADDING: i8 = 8;
const DEFAULT_WIDTH: f32 = 320.0;
const MAX_WIDTH: f32 = 560.0;

/// Shared application dialog. The caller handles the selected action.
pub struct Dialog<'a, T> {
    title: &'a str,
    message: &'a str,
    actions: &'a [(&'a str, T)],
    actions_enabled: bool,
}

impl<'a, T: Copy> Dialog<'a, T> {
    pub fn new(title: &'a str, message: &'a str, actions: &'a [(&'a str, T)]) -> Self {
        Self {
            title,
            message,
            actions,
            actions_enabled: true,
        }
    }

    pub fn actions_enabled(mut self, enabled: bool) -> Self {
        self.actions_enabled = enabled;
        self
    }

    pub fn show(self, ctx: &Context) -> Option<T> {
        let mut selected = None;
        Window::new(RichText::new(self.title).text_style(TextStyle::Body))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(DEFAULT_WIDTH)
            .max_width(MAX_WIDTH)
            .frame(Frame::window(&ctx.global_style()).inner_margin(PADDING))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(f32::from(PADDING));
                ui.add(egui::Label::new(self.message).wrap());
                ui.add_enabled_ui(self.actions_enabled, |ui| {
                    crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                        for &(label, action) in self.actions {
                            if ui.button(label).clicked() {
                                selected = Some(action);
                            }
                        }
                    });
                });
            });
        selected
    }
}
