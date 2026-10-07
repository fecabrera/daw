use egui::{Color32, Frame, Stroke};

/// Shared framing for outlined information panels. Content and commands stay with each panel.
pub struct PanelStyle {
    pub outline: Color32,
    pub margin: i8,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            outline: crate::theme::BORDER,
            margin: 10,
        }
    }
}

impl PanelStyle {
    /// Track rows share their horizontal borders instead of stacking two strokes.
    pub fn track_frame(&self) -> Frame {
        Frame::new()
            .fill(crate::theme::PANEL)
            .inner_margin(self.margin + 1)
    }

    pub fn frame(&self, selected: bool) -> Frame {
        Frame::new()
            .fill(crate::theme::PANEL)
            .stroke(Stroke::new(
                1.0_f32,
                if selected {
                    crate::theme::ACCENT
                } else {
                    self.outline
                },
            ))
            .inner_margin(self.margin)
    }
}
