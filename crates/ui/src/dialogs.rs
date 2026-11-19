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
    footer_separator: bool,
}

impl<'a, T: Copy> Dialog<'a, T> {
    pub fn new(title: &'a str, message: &'a str, actions: &'a [(&'a str, T)]) -> Self {
        Self {
            title,
            message,
            actions,
            actions_enabled: true,
            width: None,
            footer_separator: false,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn footer_separator(mut self, enabled: bool) -> Self {
        self.footer_separator = enabled;
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
        let mut selected = None;
        Window::new(RichText::new(self.title).text_style(TextStyle::Body))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(self.width.unwrap_or(DEFAULT_WIDTH))
            .max_width(self.width.unwrap_or(MAX_WIDTH))
            .frame(Frame::window(&ctx.global_style()).inner_margin(PADDING))
            .show(ctx, |ui| selected = self.body(ui, content));
        selected
    }

    pub(super) fn body(self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> Option<T> {
        let mut selected = None;
        ui.spacing_mut().item_spacing = Vec2::splat(f32::from(PADDING));
        if !self.message.is_empty() {
            ui.add(egui::Label::new(self.message).wrap());
        }
        ui.add_enabled_ui(self.actions_enabled, |ui| {
            content(ui);
            if !self.actions.is_empty() {
                if self.footer_separator {
                    let (separator, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 1.0),
                        egui::Sense::hover(),
                    );
                    let padding = f32::from(PADDING);
                    ui.painter()
                        .with_clip_rect(ui.clip_rect().expand(padding))
                        .hline(
                            (separator.left() - padding)..=(separator.right() + padding),
                            separator.center().y,
                            egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
                        );
                }
                crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                    for &(label, action) in self.actions {
                        if ui.button(label).clicked() {
                            selected = Some(action);
                        }
                    }
                });
            }
        });
        selected
    }
}

/// Return the client height needed for the dialog content and its outer padding.
pub(super) fn native_client(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> u32 {
    egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(crate::theme::palette(ui.ctx()).panel)
                .inner_margin(PADDING),
        )
        .show(ui, |ui| {
            // Measure the body separately from the panel that fills the viewport.
            (ui.scope(content).response.rect.height() + 2.0 * f32::from(PADDING)).ceil() as u32
        })
        .inner
}
