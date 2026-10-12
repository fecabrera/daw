use egui::{Color32, Frame, InnerResponse, Rect, Stroke, Ui, Vec2};

pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const HEADER_FONT_SIZE: f32 = 11.0;

/// Shared framing for outlined information panels. Content and commands stay with each panel.
pub struct PanelStyle {
    pub outline: Option<Color32>,
    pub margin: i8,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            outline: None,
            margin: 10,
        }
    }
}

impl PanelStyle {
    /// Base panel for every source, effect, and modulator; callers supply names and controls.
    /// Headers share the mixer dimensions and use the active accent.
    pub fn unit<R>(
        &self,
        ui: &mut Ui,
        name: &str,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        self.frame(ui.ctx(), false).inner_margin(0).show(ui, |ui| {
            let spacing = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = 0.0;
            let (header, _) =
                ui.allocate_exact_size(Vec2::new(0.0, HEADER_HEIGHT), egui::Sense::hover());
            let body = Frame::new().inner_margin(self.margin).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = spacing;
                content(ui)
            });
            let header = Rect::from_min_size(
                header.min,
                Vec2::new(body.response.rect.width(), HEADER_HEIGHT),
            );
            let accent = crate::theme::accent(ui.ctx());
            ui.painter().rect_filled(header, 0.0, accent);
            ui.put(
                header,
                egui::Label::new(
                    egui::RichText::new(name)
                        .size(HEADER_FONT_SIZE)
                        .color(crate::theme::unit_header_text(accent)),
                )
                .truncate(),
            );
            body.inner
        })
    }

    /// Track rows share their horizontal borders instead of stacking two strokes.
    pub fn track_frame(&self, ctx: &egui::Context) -> Frame {
        Frame::new()
            .fill(crate::theme::palette(ctx).panel)
            .inner_margin(self.margin + 1)
    }

    pub fn frame(&self, ctx: &egui::Context, selected: bool) -> Frame {
        Frame::new()
            .fill(crate::theme::palette(ctx).panel)
            .stroke(Stroke::new(
                1.0_f32,
                if selected {
                    crate::theme::accent(ctx)
                } else {
                    self.outline.unwrap_or(crate::theme::palette(ctx).border)
                },
            ))
            .inner_margin(self.margin)
    }
}
