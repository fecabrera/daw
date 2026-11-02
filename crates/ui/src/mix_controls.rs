//! Shared track/layer mixing controls.
use crate::{knobs, rows, toolbars};
use egui::{Rect, Response, Ui};

pub(super) const GROUP_GAP: f32 = 6.0;
pub(super) const KNOB_GAP: f32 = 4.0;

/// Shared mute/solo buttons; callers choose their row placement.
pub(super) fn mute_solo(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
) -> [Response; 2] {
    mute_solo_with_size(ui, muted, soloed, tooltips, toolbars::CONTROL_HEIGHT)
}
fn mute_solo_with_size(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
    size: f32,
) -> [Response; 2] {
    std::array::from_fn(|index| {
        let (label, name, selected) = if index == 0 {
            ("M", "Mute", &mut *muted)
        } else {
            ("S", "Solo", &mut *soloed)
        };
        let response = if size == toolbars::CONTROL_HEIGHT {
            toolbars::square_toggle(ui, label, *selected)
        } else {
            ui.scope(|ui| {
                ui.spacing_mut().button_padding = egui::Vec2::new(2.0, 0.0);
                ui.spacing_mut().interact_size = egui::Vec2::splat(size);
                ui.add_sized(
                    egui::Vec2::splat(size),
                    egui::Button::new(egui::RichText::new(label).size(9.0)).selected(*selected),
                )
            })
            .inner
        }
        .on_hover_text(tooltips[index]);
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
                ui.spacing_mut().item_spacing.x = KNOB_GAP;
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

/// Compact instrument row, using the same control sizes and gaps as track headers.
pub(super) fn named_row<R>(
    ui: &mut Ui,
    mix: (&mut f32, &mut f32),
    muted: &mut bool,
    soloed: &mut bool,
    name: impl FnOnce(&mut Ui) -> R,
) -> (R, GainPanResponse, [Response; 2]) {
    let mut header = crate::track_rows::HeaderRows::new(ui, true);
    let name = rows::centered(&mut header.name, knobs::SIZE, name).inner;
    header.mix.spacing_mut().item_spacing.x = GROUP_GAP;
    let (knobs, toggles) = header_row(
        &mut header.mix,
        mix.0,
        mix.1,
        muted,
        soloed,
        ["Mute subtrack", "Solo within this beat instrument"],
    );
    (name, knobs, toggles)
}

/// Fixed-width controls for the catalog-height Tracks row. No extra knob slots.
pub(super) const BROWSER_BUTTON_SIZE: f32 = 16.0;
pub(super) const BROWSER_KNOB_SIZE: f32 = 16.0;
pub(super) const BROWSER_WIDTH: f32 =
    2.0 * BROWSER_BUTTON_SIZE + 2.0 * BROWSER_KNOB_SIZE + 3.0 * KNOB_GAP;
pub(super) fn browser_row(
    ui: &mut Ui,
    gain: &mut f32,
    pan: &mut f32,
    muted: &mut bool,
    soloed: &mut bool,
) -> [Response; 4] {
    ui.spacing_mut().item_spacing.x = KNOB_GAP;
    let [mute, solo] =
        mute_solo_with_size(ui, muted, soloed, ["Mute", "Solo"], BROWSER_BUTTON_SIZE);
    let pan = knobs::pan_scaled(ui, pan, BROWSER_KNOB_SIZE / knobs::SIZE);
    let gain = knobs::gain_scaled(ui, gain, BROWSER_KNOB_SIZE / knobs::SIZE);
    [mute, solo, pan, gain]
}
