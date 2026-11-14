//! Phase-domain oscillator shape, independent of note pitch and playback.
use daw_core::Waveform;
use egui::{Response, Sense, Stroke, Ui, Vec2};

fn shape(waveform: Waveform, phase: f32) -> f32 {
    match waveform {
        Waveform::Sine => (phase * std::f32::consts::TAU).sin(),
        Waveform::Saw => 2.0 * phase - 1.0,
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
    }
}

pub(super) fn show(ui: &mut Ui, waveform: &mut Waveform, enabled: bool) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::splat(crate::knobs::CHOICE_SIZE), Sense::hover());
    let graph = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 26.0));
    let palette = crate::theme::palette(ui.ctx());
    let painter = ui.painter_at(graph);
    painter.rect_filled(graph, 2.0, palette.background);
    painter.hline(
        graph.x_range(),
        graph.center().y,
        Stroke::new(1.0, palette.divider),
    );
    let points = (0..=128)
        .map(|i| {
            let phase = i as f32 / 128.0;
            egui::pos2(
                graph.left() + phase * graph.width(),
                graph.center().y - shape(*waveform, phase) * graph.height() * 0.4,
            )
        })
        .collect();
    let color = if enabled {
        crate::theme::accent(ui.ctx())
    } else {
        palette.secondary
    };
    painter.add(egui::Shape::line(points, Stroke::new(1.0, color)));
    ui.interact(graph, response.id.with("preview"), Sense::hover())
        .on_hover_text("Waveform");
    let before = *waveform;
    let input = crate::oscillator_controls::input_bounds(rect);
    let mut selector = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("waveform-selector")
            .max_rect(egui::Rect::from_min_max(
                input.min + egui::vec2(0.0, 30.0),
                input.max,
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let dropdown = crate::menus::Dropdown::from_id_salt("waveform")
        .small_items()
        .selected_text(
            egui::RichText::new(waveform.label()).size(daw_unit_ui::layout::HEADER_FONT_SIZE),
        )
        .width(input.width())
        .truncate()
        .show_ui(&mut selector, |ui| {
            for choice in Waveform::ALL {
                if ui
                    .selectable_value(waveform, choice, choice.label())
                    .clicked()
                {
                    ui.close();
                }
            }
        })
        .response;
    dropdown.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ComboBox,
            dropdown.enabled(),
            "Oscillator waveform",
        )
    });
    response |= dropdown;
    if *waveform != before {
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapes_cover_one_period_with_the_expected_polarity_and_extrema() {
        assert!(shape(Waveform::Sine, 0.0).abs() < 1e-6);
        assert!((shape(Waveform::Sine, 0.25) - 1.0).abs() < 1e-6);
        assert!(shape(Waveform::Sine, 0.5).abs() < 1e-6);
        assert!((shape(Waveform::Sine, 0.75) + 1.0).abs() < 1e-6);
        assert!(shape(Waveform::Sine, 1.0).abs() < 1e-6);
        assert_eq!(
            [shape(Waveform::Saw, 0.0), shape(Waveform::Saw, 1.0)],
            [-1.0, 1.0]
        );
        assert_eq!(
            [shape(Waveform::Square, 0.25), shape(Waveform::Square, 0.75)],
            [1.0, -1.0]
        );
        assert_eq!(
            [
                shape(Waveform::Triangle, 0.0),
                shape(Waveform::Triangle, 0.5),
                shape(Waveform::Triangle, 1.0)
            ],
            [-1.0, 1.0, -1.0]
        );
    }
}
