use egui::{Align2, Context, Frame, RichText, TextStyle, Vec2, Window};

pub(super) const PADDING: i8 = 8;
const DEFAULT_WIDTH: f32 = 320.0;
const MAX_WIDTH: f32 = 560.0;

#[derive(Clone, Copy)]
enum Role {
    Accept,
    Cancel,
    Deny,
}

#[derive(Clone, Copy)]
pub struct FooterAction<'a, T> {
    role: Role,
    label: &'a str,
    value: T,
    enabled: bool,
}

impl<'a, T: Copy> FooterAction<'a, T> {
    const fn new(role: Role, label: &'a str, value: T) -> Self {
        Self {
            role,
            label,
            value,
            enabled: true,
        }
    }

    pub const fn accept(label: &'a str, value: T) -> Self {
        Self::new(Role::Accept, label, value)
    }

    pub const fn cancel(label: &'a str, value: T) -> Self {
        Self::new(Role::Cancel, label, value)
    }

    pub const fn deny(label: &'a str, value: T) -> Self {
        Self::new(Role::Deny, label, value)
    }

    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn show(self, ui: &mut egui::Ui) -> Option<T> {
        let button = match self.role {
            Role::Accept => accent_button(ui.ctx(), self.label),
            Role::Cancel | Role::Deny => egui::Button::new(self.label),
        };
        ui.add_enabled(self.enabled, button)
            .clicked()
            .then_some(self.value)
    }
}

/// Shared application dialog. The caller handles the selected action.
pub struct Dialog<'a, T> {
    title: &'a str,
    message: &'a str,
    actions: &'a [FooterAction<'a, T>],
    actions_enabled: bool,
    width: Option<f32>,
}

impl<'a, T: Copy> Dialog<'a, T> {
    pub fn new(title: &'a str, message: &'a str, actions: &'a [FooterAction<'a, T>]) -> Self {
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
        self.show_message(ctx, None)
    }

    pub(super) fn show_message(self, ctx: &Context, surface: Option<&mut egui::Ui>) -> Option<T> {
        let draw = |ui: &mut egui::Ui| {
            self.content(ui, |_| {});
            self.actions_footer(ui)
        };
        match surface {
            Some(ui) => draw(ui),
            None => self.embedded(ctx, draw),
        }
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
            let value = self.content(ui, |ui| {
                ui.add_enabled_ui(self.actions_enabled, content).inner
            });
            buttons(ui, value)
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
                content(ui)
            })
            .inner
    }

    fn body(&self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> Option<T> {
        self.content(ui, |ui| {
            ui.add_enabled_ui(self.actions_enabled, content).inner
        });
        self.actions_footer(ui)
    }

    fn actions_footer(&self, ui: &mut egui::Ui) -> Option<T> {
        if self.actions.is_empty() {
            return None;
        }
        ui.add_enabled_ui(self.actions_enabled, |ui| footer(ui, self.actions))
            .inner
    }
}

pub(super) fn footer<T: Copy>(ui: &mut egui::Ui, actions: &[FooterAction<'_, T>]) -> Option<T> {
    let footer = Frame::new().inner_margin(PADDING).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
            let deny = actions
                .iter()
                .find(|action| matches!(action.role, Role::Deny))
                .and_then(|action| action.show(ui));
            let right = ui
                .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let accept = actions
                        .iter()
                        .find(|action| matches!(action.role, Role::Accept))
                        .and_then(|action| action.show(ui));
                    let cancel = actions
                        .iter()
                        .find(|action| matches!(action.role, Role::Cancel))
                        .and_then(|action| action.show(ui));
                    accept.or(cancel)
                })
                .inner;
            deny.or(right)
        })
        .inner
    });
    let bounds = footer.response.rect;
    ui.painter().hline(
        bounds.x_range(),
        bounds.top() + 0.5,
        egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
    );
    footer.inner
}

fn accent_button(ctx: &Context, label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).color(egui::Color32::WHITE))
        .fill(crate::theme::accent(ctx))
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
                    Dialog::new("Test", "Message", &[FooterAction::accept("OK", ())])
                        .show_message(&ctx, Some(ui));
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

    #[test]
    fn footer_only_shows_supplied_actions_in_their_positions() {
        let accept = FooterAction::accept("Accept", ());
        let cancel = FooterAction::cancel("Cancel", ());
        let deny = FooterAction::deny("Discard", ());
        let width = 352.0;
        for actions in [
            &[accept][..],
            &[accept, cancel],
            &[cancel, deny, accept],
            &[cancel],
        ] {
            let ctx = Context::default();
            crate::theme::configure(&ctx);
            crate::theme::set_accent(&ctx, crate::settings::AccentColor::Amber.color());
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 100.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    native_client(ui, |ui| {
                        footer(ui, actions);
                    });
                },
            );
            output.textures_delta.clear();
            let position = |label| {
                output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
            };
            for label in ["Accept", "Cancel", "Discard"] {
                assert_eq!(
                    position(label).is_some(),
                    actions.iter().any(|action| action.label == label)
                );
            }
            if let Some(discard) = position("Discard") {
                assert!(discard.x < 70.0);
            }
            if let Some(cancel) = position("Cancel") {
                assert!(cancel.x > width - 130.0);
                if let Some(accept) = position("Accept") {
                    assert!(cancel.x < accept.x);
                }
            }
            if let Some(accept) = position("Accept") {
                assert!(accept.x > width - 70.0);
                let accent = crate::theme::accent(&ctx);
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.fill == accent && rect.rect.contains(accept)
                )));
                let label = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == "Accept" => Some(text),
                        _ => None,
                    })
                    .unwrap();
                let glyphs = &label.galley.rows[0].row.visuals;
                assert!(!glyphs.glyph_vertex_range.is_empty());
                assert!(
                    glyphs.mesh.vertices[glyphs.glyph_vertex_range.clone()]
                        .iter()
                        .all(|vertex| vertex.color == egui::Color32::WHITE)
                );
            }
        }
    }
}
