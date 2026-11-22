use egui::{Align2, Context, Frame, RichText, TextStyle, Vec2, Window};

pub(super) const PADDING: i8 = 8;
const DEFAULT_WIDTH: f32 = 320.0;
const MAX_WIDTH: f32 = 560.0;

/// Shared application dialog. The caller handles the selected action.
pub struct Dialog<'a, T> {
    title: &'a str,
    message: &'a str,
    actions: &'a [(&'a str, T)],
    actions_enabled: bool,
    width: Option<f32>,
}

impl<'a, T: Copy> Dialog<'a, T> {
    pub fn new(title: &'a str, message: &'a str, actions: &'a [(&'a str, T)]) -> Self {
        Self {
            title,
            message,
            actions,
            actions_enabled: true,
            width: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn actions_enabled(mut self, enabled: bool) -> Self {
        self.actions_enabled = enabled;
        self
    }

    pub fn show(self, ctx: &Context) -> Option<T> {
        self.show_with_content(ctx, |_| {})
    }

    pub fn show_with_content(
        self,
        ctx: &Context,
        content: impl FnOnce(&mut egui::Ui),
    ) -> Option<T> {
        self.embedded(ctx, |ui| self.body(ui, content))
    }

    fn embedded(&self, ctx: &Context, body: impl FnOnce(&mut egui::Ui) -> Option<T>) -> Option<T> {
        let mut selected = None;
        let shown = Window::new(RichText::new(self.title).text_style(TextStyle::Body))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(self.width.unwrap_or(DEFAULT_WIDTH))
            .max_width(self.width.unwrap_or(MAX_WIDTH))
            .frame(Frame::window(&ctx.global_style()).inner_margin(0))
            .show(ctx, |ui| selected = body(ui));
        if let Some(shown) = shown {
            ctx.memory_mut(|memory| memory.set_modal_layer(shown.response.layer_id));
        }
        selected
    }

    pub(super) fn show_surface(
        self,
        ctx: &Context,
        surface: Option<&mut egui::Ui>,
        content: impl FnOnce(&mut egui::Ui),
    ) -> Option<T> {
        match surface {
            Some(ui) => self.body(ui, content),
            None => self.show_with_content(ctx, content),
        }
    }

    pub(super) fn show_with_footer<R>(
        &self,
        ctx: &Context,
        surface: Option<&mut egui::Ui>,
        content: impl FnOnce(&mut egui::Ui) -> R,
        buttons: impl FnOnce(&mut egui::Ui, R) -> Option<T>,
    ) -> Option<T> {
        let draw = |ui: &mut egui::Ui| {
            let value = self.content(ui, content);
            footer(ui, |ui| buttons(ui, value))
        };
        match surface {
            Some(ui) => draw(ui),
            None => self.embedded(ctx, draw),
        }
    }

    fn content<R>(&self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
        ui.spacing_mut().item_spacing.y = 0.0;
        Frame::new()
            .inner_margin(PADDING)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(f32::from(PADDING));
                if !self.message.is_empty() {
                    ui.add(egui::Label::new(self.message).wrap());
                }
                ui.add_enabled_ui(self.actions_enabled, content).inner
            })
            .inner
    }

    pub(super) fn body(&self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> Option<T> {
        self.content(ui, content);
        if self.actions.is_empty() {
            return None;
        }
        footer(ui, |ui| {
            ui.add_enabled_ui(self.actions_enabled, |ui| {
                let mut selected = None;
                for &(label, action) in self.actions {
                    if ui.button(label).clicked() {
                        selected = Some(action);
                    }
                }
                selected
            })
            .inner
        })
    }
}

pub(super) fn footer<R>(ui: &mut egui::Ui, buttons: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let footer = Frame::new().inner_margin(PADDING).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, buttons).inner
    });
    let bounds = footer.response.rect;
    ui.painter().hline(
        bounds.x_range(),
        bounds.top() + 0.5,
        egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
    );
    footer.inner
}

/// Return the fitted height of the content and footer, including their padding.
pub(super) fn native_client(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> u32 {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(crate::theme::palette(ui.ctx()).panel))
        .show(ui, |ui| {
            ui.scope(content).response.rect.height().ceil() as u32
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_dialog_fits_content_and_buttons_below_a_full_width_divider() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(352.0, 160.0));
        let mut height = 0;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(viewport),
                ..Default::default()
            },
            |ui| {
                height = native_client(ui, |ui| {
                    Dialog::new("Test", "Message", &[("OK", ())]).body(ui, |_| {});
                });
            },
        );
        output.textures_delta.clear();
        let y = |label| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos.y + text.galley.rect.center().y)
                    }
                    _ => None,
                })
                .unwrap()
        };
        let message = y("Message");
        let button = y("OK");
        let border = crate::theme::palette(&ctx).border;
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, stroke }
                if (points[0].x - viewport.left()).abs() <= 1.0
                    && (points[1].x - viewport.right()).abs() <= 1.0
                    && points[0].y == points[1].y
                    && (message..button).contains(&points[0].y)
                    && stroke.color == border
        )));
        assert!((button + f32::from(PADDING)..viewport.bottom()).contains(&(height as f32)));
    }
}
