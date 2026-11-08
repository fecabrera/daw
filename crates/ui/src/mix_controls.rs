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
    ui.allocate_ui_with_layout(
        egui::Vec2::new(2.0 * size, size),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let border = ui.visuals().widgets.inactive.bg_stroke.width;
            let group = rows::input_group_with_height(ui, size, |ui| {
                ui.spacing_mut().button_padding = egui::Vec2::new(2.0, 0.0);
                ui.spacing_mut().interact_size = egui::Vec2::splat(size - 2.0 * border);
                std::array::from_fn(|index| {
                    let (label, name, selected) = if index == 0 {
                        ("M", "Mute", &mut *muted)
                    } else {
                        ("S", "Solo", &mut *soloed)
                    };
                    let text = if size == toolbars::CONTROL_HEIGHT {
                        egui::RichText::new(label).small()
                    } else {
                        egui::RichText::new(label).size(9.0)
                    };
                    let response = ui
                        .add_sized(
                            egui::Vec2::new(size - border, size - 2.0 * border),
                            egui::Button::new(text).selected(*selected),
                        )
                        .on_hover_text(tooltips[index]);
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
            });
            let buttons: [Response; 2] = group.inner;
            rows::input_group_outline(
                ui,
                group.response.rect,
                buttons[0].rect.right(),
                &[&buttons[0], &buttons[1]],
            );
            buttons
        },
    )
    .inner
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
    2.0 * BROWSER_BUTTON_SIZE + 2.0 * BROWSER_KNOB_SIZE + 2.0 * KNOB_GAP;
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Vec2};

    #[test]
    fn joined_mute_solo_keep_independent_toggles_and_disabled_guards() {
        for size in [toolbars::CONTROL_HEIGHT, BROWSER_BUTTON_SIZE] {
            for theme in [
                crate::settings::ColorTheme::Dark,
                crate::settings::ColorTheme::Light,
                crate::settings::ColorTheme::Mist,
            ] {
                let ctx = egui::Context::default();
                crate::theme::configure(&ctx);
                crate::theme::apply(
                    &ctx,
                    crate::settings::AppSettings {
                        theme,
                        ..Default::default()
                    },
                );
                let (mut muted, mut soloed) = (false, false);
                let mut bounds = [Rect::NOTHING; 2];
                let mut run = |events, enabled| {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(200.0, 100.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            let group = ui.add_enabled_ui(enabled, |ui| {
                                mute_solo_with_size(
                                    ui,
                                    &mut muted,
                                    &mut soloed,
                                    ["Mute", "Solo"],
                                    size,
                                )
                            });
                            bounds = group.inner.map(|r| r.rect);
                        },
                    );
                    output.textures_delta.clear();
                    (bounds, muted, soloed)
                };
                let (bounds, _, _) = run(vec![], true);
                assert!(
                    (bounds[0].right() - bounds[1].left()).abs() < 0.01,
                    "joined controls touch at their divider"
                );
                assert!((bounds[0].height() + 2.0 - size).abs() < 0.01);
                for (index, enabled, expected) in [
                    (0, true, (true, false)),
                    (1, true, (true, true)),
                    (0, true, (false, true)),
                    (1, false, (false, true)),
                ] {
                    let point = bounds[index].center();
                    let event = |pressed| egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    };
                    run(vec![egui::Event::PointerMoved(point), event(true)], enabled);
                    let (_, mute, solo) = run(vec![event(false)], enabled);
                    assert_eq!((mute, solo), expected);
                }
            }
        }
    }
}
