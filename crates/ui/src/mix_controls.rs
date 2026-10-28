//! Shared track/layer mixing controls.
use crate::{knobs, rows, toolbars};
use egui::{Rect, Response, Ui};

/// Shared mute/solo buttons; callers choose their row placement.
pub(super) fn mute_solo(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
) -> [Response; 2] {
    std::array::from_fn(|index| {
        let (label, name, selected) = if index == 0 {
            ("M", "Mute", &mut *muted)
        } else {
            ("S", "Solo", &mut *soloed)
        };
        let response = toolbars::square_toggle(ui, label, *selected).on_hover_text(tooltips[index]);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), *selected, name)
        });
        if response.clicked() {
            *selected = !*selected;
        }
        response
    })
}

/// Track/layer mixing row: mute/solo at left, pan then gain at right.
pub(super) fn header_row(
    ui: &mut Ui,
    gain: &mut f32,
    pan: &mut f32,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
) -> (GainPanResponse, [Response; 2]) {
    rows::centered(ui, knobs::SIZE, |ui| {
        let toggles = mute_solo(ui, muted, soloed, tooltips);
        let knobs = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let gain = knobs::header_gain(ui, gain);
                let pan = knobs::header_pan(ui, pan);
                GainPanResponse {
                    bounds: [gain.rect, pan.rect],
                }
            })
            .inner;
        (knobs, toggles)
    })
    .inner
}

pub(super) struct GainPanResponse {
    pub bounds: [Rect; 2],
}
