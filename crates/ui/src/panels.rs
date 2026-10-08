use egui::{Color32, Frame, Stroke};

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
