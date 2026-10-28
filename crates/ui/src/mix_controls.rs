//! Shared track/layer mixing controls.
use crate::{knobs, rows, toolbars};
use egui::{Rect, Response, Ui};

/// Reserve the existing header columns and align the buttons with the second-row knobs.
pub(super) fn second_row_mute_solo(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
    horizontal_offset: f32,
) -> [Response; 2] {
    let spacing = ui.spacing().item_spacing;
    let size = egui::Vec2::new(
        2.0 * toolbars::CONTROL_HEIGHT + spacing.x,
        toolbars::CONTROL_HEIGHT,
    );
    second_row(ui, size, horizontal_offset, |ui| {
        std::array::from_fn(|index| {
            let (label, name, selected) = if index == 0 {
                ("M", "Mute", &mut *muted)
            } else {
                ("S", "Solo", &mut *soloed)
            };
            let response =
                toolbars::square_toggle(ui, label, *selected).on_hover_text(tooltips[index]);
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    *selected,
                    name,
                )
            });
            if response.clicked() {
                *selected = !*selected;
            }
            response
        })
    })
}

/// Keep a header control's column while aligning it with the second-row knobs.
fn second_row<R>(
    ui: &mut Ui,
    size: egui::Vec2,
    horizontal_offset: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let (slot, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let offset = toolbars::CONTROL_HEIGHT
        + ui.spacing().item_spacing.y
        + (knobs::SIZE - toolbars::CONTROL_HEIGHT) / 2.0;
    let mut controls = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(slot.translate(egui::Vec2::new(horizontal_offset, offset)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    add(&mut controls)
}

pub(super) struct GainPanResponse {
    pub bounds: [Rect; 2],
}

/// Header knobs apply immediately; values remain available in their shared tooltips.
pub(super) fn gain_pan(ui: &mut Ui, gain: &mut f32, pan: &mut f32) -> GainPanResponse {
    let bounds = rows::centered(ui, knobs::SIZE, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let gain = knobs::header_gain(ui, gain);
        let pan = knobs::header_pan(ui, pan);
        [gain.rect, pan.rect]
    })
    .inner;
    GainPanResponse { bounds }
}
