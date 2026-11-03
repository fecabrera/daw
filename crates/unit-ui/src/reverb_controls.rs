//! Discrete reverb selectors share labels, geometry and host gesture capture.
use crate::{control_capture, layout, menus::Dropdown, rows};
use egui::{Response, RichText, Ui, WidgetInfo, WidgetType};

pub(crate) fn switch(
    ui: &mut Ui,
    value: &mut bool,
    id: &'static str,
    label: &'static str,
    choices: [&'static str; 2],
    enabled: bool,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        rows::selector_row(ui, label, |ui| {
            let before = *value;
            let mut response = Dropdown::from_id_salt(id)
                .small_items()
                .truncate()
                .selected_text(
                    RichText::new(choices[usize::from(*value)]).size(layout::HEADER_FONT_SIZE),
                )
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, false, choices[0]);
                    ui.selectable_value(value, true, choices[1]);
                })
                .response
                .on_hover_text(label);
            if before != *value {
                response.mark_changed();
            }
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::ComboBox, ui.is_enabled(), *value, label)
            });
            control_capture::record(ui, &response);
            response
        })
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};
    #[test]
    fn reverb_selectors_use_full_strip_width_and_standard_label_above() {
        fn labels(shape: &egui::Shape, output: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(text) => output.push((
                    text.galley.text().to_owned(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, output)
                    }
                }
                _ => {}
            }
        }
        for plate in [true, false] {
            for scale in [1.0, 1.5, 2.0] {
                let ctx = egui::Context::default();
                crate::theme::configure(&ctx);
                crate::fonts::configure(&ctx);
                ctx.set_pixels_per_point(scale);
                let mut controls = Vec::new();
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(
                                if plate {
                                    crate::plate_reverb::STRIP_COUNT as f32
                                        * layout::UNIT_STRIP_WIDTH
                                } else {
                                    270.0
                                },
                                218.0,
                            ),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        if plate {
                            let mut settings = daw_unit_core::PlateReverbSettings::default();
                            let b = crate::plate_reverb::show(
                                ui,
                                &mut settings,
                                false,
                                layout::unit_control_height(),
                                &std::array::from_fn(|_| None),
                                0.0.into(),
                            );
                            for (slot, label) in [(8, "Tail"), (9, "Color"), (11, "Quality")] {
                                controls.push((label, b.controls[slot]));
                            }
                        } else {
                            let mut settings = daw_unit_core::AmbientReverbSettings::default();
                            let b = crate::ambient_reverb::show(
                                ui,
                                &mut settings,
                                false,
                                layout::unit_control_height(),
                                &std::array::from_fn(|_| None),
                                0.0.into(),
                            );
                            controls.push(("Quality", b.controls[7]));
                        }
                    },
                );
                let mut texts = Vec::new();
                for shape in &out.shapes {
                    labels(&shape.shape, &mut texts);
                }
                for (name, control) in controls {
                    assert!((control.width() - 70.0).abs() < 0.6, "{name}: {control:?}");
                    assert!(
                        texts.iter().any(|(label, rect)| label == name
                            && rect.bottom() <= control.top() + 0.5
                            && (rect.center().x - control.center().x).abs() < 0.6),
                        "{name}: label must be centered above its selector"
                    );
                }
                out.textures_delta.clear();
            }
        }
    }
}
