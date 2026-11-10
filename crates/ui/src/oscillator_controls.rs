//! Compact joined Unison/Detune input; host settings/history own publication.
use super::*;

/// Single-strip input footprint, centered over a standard knob column.
pub(super) fn input_bounds(rect: Rect) -> Rect {
    let width =
        daw_unit_ui::layout::UNIT_STRIP_WIDTH - 2.0 * f32::from(daw_unit_ui::layout::PANEL_MARGIN);
    Rect::from_center_size(rect.center(), Vec2::new(width, rect.height()))
}

pub(super) fn unison(
    ui: &mut egui::Ui,
    settings: &mut daw_core::OscillatorSettings,
) -> egui::Response {
    let (rect, row_response) =
        ui.allocate_exact_size(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(input_bounds(rect))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    let ui = &mut child;
    let before = (settings.unison, settings.detune_cents);
    let input = daw_unit_ui::rows::selector_row(ui, "Unison", |ui| {
        let group = rows::input_group(ui, |ui| {
            rows::input_group_edge(ui, false);
            // The joined fields share their outer frame; keep space at the divider.
            ui.spacing_mut().button_padding.x = 2.0;
            ui.spacing_mut().interact_size.x = 30.0;
            ui.style_mut().drag_value_text_style = egui::TextStyle::Small;
            let text = match settings.unison {
                daw_core::OscillatorUnison::Off => "Off".into(),
                mode => mode.voices().to_string(),
            };
            let mode = menus::Dropdown::from_id_salt("unison")
                .width(40.0)
                .truncate()
                .small_items()
                .selected_text(
                    egui::RichText::new(text).size(daw_unit_ui::layout::HEADER_FONT_SIZE),
                )
                .show_ui(ui, |ui| {
                    for mode in daw_core::OscillatorUnison::ALL {
                        ui.selectable_value(&mut settings.unison, mode, mode.name());
                    }
                })
                .response
                .on_hover_text("Unison voices: Off, 2, 4 or 8");
            rows::input_group_edge(ui, true);
            let detune = ui
                .add_enabled_ui(settings.unison != daw_core::OscillatorUnison::Off, |ui| {
                    let font = fonts::numeric(egui::TextStyle::Small.resolve(ui.style()).size);
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Small, font);
                    rows::staged_input(ui, "detune", &mut settings.detune_cents, |ui, value| {
                        ui.add_sized(
                            [30.0, ui.spacing().interact_size.y],
                            egui::DragValue::new(value)
                                .range(0.0..=100.0)
                                .speed(0.1)
                                .max_decimals(1)
                                .update_while_editing(false),
                        )
                        .on_hover_text("Unison detune in cents (0–100). Drag or click to type.")
                    })
                    .0
                })
                .inner;
            mode.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, mode.enabled(), "Unison mode")
            });
            detune.widget_info(|| {
                let mut info = egui::WidgetInfo::drag_value(
                    detune.enabled(),
                    f64::from(settings.detune_cents),
                );
                info.label = Some("Unison detune".into());
                info
            });
            (mode, detune)
        });
        rows::input_group_separator(ui, group.response.rect, group.inner.1.rect.left());
        group.response | group.inner.0 | group.inner.1
    });
    let mut response = row_response | input;
    if before != (settings.unison, settings.detune_cents) {
        response.mark_changed();
    }
    response
}
