//! Custom header presentation and geometry consumed by the desktop window adapter.

use crate::{DawUi, theme, toolbars};
use egui::{Pos2, Rect, Vec2};

/// Native caption bounds in egui points. Updated by the desktop adapter for DPI changes.
#[derive(Clone, Copy, Debug)]
pub struct CaptionMetrics {
    pub width: f32,
    pub height: f32,
    /// Compensate for the off-screen resize frame of a maximized Windows window.
    pub top_inset: f32,
}

impl Default for CaptionMetrics {
    fn default() -> Self {
        Self {
            width: 138.0,
            height: toolbars::HEIGHT,
            top_inset: 0.0,
        }
    }
}

/// Only the title and unused header space are native window drag targets.
#[derive(Clone, Debug)]
pub struct HeaderLayout {
    pub bounds: Rect,
    pub controls: Vec<Rect>,
    pub blocked: bool,
    pub captions: [Rect; 3],
}

impl Default for HeaderLayout {
    fn default() -> Self {
        Self {
            bounds: Rect::NOTHING,
            controls: Vec::new(),
            blocked: true,
            captions: [Rect::NOTHING; 3],
        }
    }
}

impl HeaderLayout {
    pub fn can_drag(&self, point: Pos2) -> bool {
        !self.blocked
            && self.bounds.contains(point)
            && !self.controls.iter().any(|rect| rect.contains(point))
            && !self.captions.iter().any(|rect| rect.contains(point))
    }
}

impl DawUi {
    pub fn set_caption_metrics(&mut self, metrics: CaptionMetrics) {
        self.caption_metrics = metrics;
    }

    pub fn header_layout(&self, ctx: &egui::Context) -> HeaderLayout {
        let mut layout = self.header_layout.clone();
        // Popups and dialogs can cover the header. Their clicks must stay in egui.
        layout.blocked |= egui::Popup::is_any_open(ctx)
            || self.settings_open
            || self.project_settings.is_some()
            || self.pending.is_some()
            || self.error.is_some();
        layout
    }

    pub(super) fn windows_header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(self.caption_metrics.top_inset);
        let mut controls = Vec::new();
        let height = toolbars::HEIGHT.max(self.caption_metrics.height);
        let original_clip = ui.clip_rect();
        ui.set_clip_rect(original_clip.intersect(Rect::from_min_max(
            ui.min_rect().min,
            Pos2::new(
                ui.max_rect().right() - self.caption_metrics.width,
                ui.max_rect().bottom(),
            ),
        )));
        let response = toolbars::Toolbar::show(ui, |ui| {
            ui.set_min_height(height - 2.0 * f32::from(theme::TOOLBAR_PADDING));
            // Reserve the caption region for the DWM-owned caption buttons.
            let width = (ui.available_width() - self.caption_metrics.width).max(0.0);
            ui.allocate_ui_with_layout(
                Vec2::new(width, ui.min_rect().height()),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let menu = self.application_menu(ui);
                    controls.push(menu.rect);
                    // Bound long names so the view controls and native buttons stay visible.
                    let style = ui.ctx().global_style();
                    let views_width: f32 = crate::ViewMode::ALL
                        .iter()
                        .map(|mode| {
                            ui.painter()
                                .layout_no_wrap(
                                    mode.label().into(),
                                    egui::TextStyle::Button.resolve(&style),
                                    theme::palette(ui.ctx()).text,
                                )
                                .size()
                                .x
                                + 2.0 * style.spacing.button_padding.x
                        })
                        .sum();
                    let title_width = ui
                        .painter()
                        .layout_no_wrap(
                            self.window_title.clone(),
                            egui::TextStyle::Body.resolve(ui.style()),
                            theme::palette(ui.ctx()).text,
                        )
                        .size()
                        .x
                        .min(
                            (ui.available_width()
                                - views_width
                                - 4.0 * ui.spacing().item_spacing.x)
                                .max(0.0),
                        );
                    ui.allocate_ui_with_layout(
                        Vec2::new(title_width, toolbars::CONTROL_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add(egui::Label::new(&self.window_title).truncate())
                                .on_hover_text(&self.window_title);
                        },
                    );
                    ui.separator();
                    controls.push(ui.scope(|ui| self.view_controls(ui)).response.rect);
                },
            );
        });
        let bounds = response.response.rect;
        let captions = std::array::from_fn(|index| {
            Rect::from_min_max(
                Pos2::new(
                    bounds.right() - self.caption_metrics.width
                        + index as f32 * self.caption_metrics.width / 3.0,
                    bounds.top(),
                ),
                Pos2::new(
                    bounds.right() - self.caption_metrics.width
                        + (index + 1) as f32 * self.caption_metrics.width / 3.0,
                    bounds.bottom(),
                ),
            )
        });
        ui.set_clip_rect(original_clip);
        self.header_layout = HeaderLayout {
            bounds,
            controls,
            blocked: !ui.is_enabled(),
            captions,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_dialogs_and_space_outside_the_header_are_not_drag_targets() {
        let mut layout = HeaderLayout {
            bounds: Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 38.0)),
            controls: vec![Rect::from_min_size(Pos2::ZERO, Vec2::new(32.0, 38.0))],
            blocked: false,
            captions: [Rect::NOTHING; 3],
        };
        assert!(layout.can_drag(Pos2::new(200.0, 20.0)));
        assert!(!layout.can_drag(Pos2::new(15.0, 20.0)));
        assert!(!layout.can_drag(Pos2::new(200.0, 50.0)));
        layout.blocked = true;
        assert!(!layout.can_drag(Pos2::new(200.0, 20.0)));
    }
}
