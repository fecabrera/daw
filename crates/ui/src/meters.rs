use crate::theme;
use egui::{Pos2, Rect, Response, Sense, Ui, Vec2};

/// Keep controls beside the shared full-height stereo meter rail.
pub fn stereo_panel(
    ui: &mut Ui,
    height: f32,
    levels: [(f32, bool); 2],
    content: impl FnOnce(&mut Ui),
) -> [bool; 2] {
    ui.horizontal_top(|ui| {
        let content_width = (ui.available_width() - 26.0).max(1.0);
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, height),
            egui::Layout::top_down(egui::Align::Min),
            content,
        );
        ui.spacing_mut().item_spacing.x = 4.0;
        [
            channel(ui, "L", height, levels[0].0, levels[0].1).clicked(),
            channel(ui, "R", height, levels[1].0, levels[1].1).clicked(),
        ]
    })
    .inner
}

/// Shared channel monitor for tracks and the master output.
pub fn channel(ui: &mut Ui, label: &str, height: f32, peak: f32, clipped: bool) -> Response {
    let level = if peak > 0.0 {
        format!("{:.1} dBFS", 20.0 * peak.log10())
    } else {
        "Silent".into()
    };
    let progress = peak.clamp(0.0, 1.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(8.0, height), Sense::click());
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, ui.is_enabled(), label);
        info.value = Some(f64::from(progress) * 100.0);
        info
    });
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, theme::INPUT);
    if progress > 0.0 {
        let fill = Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() - rect.height() * progress),
            rect.max,
        );
        painter.rect_filled(fill, 1.0, if clipped { theme::ERROR } else { theme::METER });
    }
    if clipped {
        let warning = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0));
        painter.rect_filled(warning, 1.0, theme::ERROR);
    }
    response.on_hover_text(if clipped {
        format!("{label}: {level}. Clipping detected. Click to clear.")
    } else {
        format!("{label}: {level}")
    })
}
