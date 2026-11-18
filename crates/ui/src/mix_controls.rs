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
    mute_solo_arm_with_size(ui, muted, soloed, tooltips, size, None)
}
fn mute_solo_arm_with_size(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
    size: f32,
    arm: Option<&mut crate::midi::Arm>,
) -> [Response; 2] {
    if let Some(arm) = arm {
        return ui
            .allocate_ui_with_layout(
                egui::Vec2::new(3.0 * size + rows::GROUP_DIVIDER_WIDTH + GROUP_GAP, size),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = GROUP_GAP;
                    let toggles = ui
                        .add_enabled_ui(arm.editable, |ui| {
                            mute_solo_arm_with_size(ui, muted, soloed, tooltips, size, None)
                        })
                        .inner;
                    ui.scope(|ui| {
                        ui.spacing_mut().button_padding = egui::Vec2::new(2.0, 0.0);
                        ui.spacing_mut().interact_size = egui::Vec2::splat(size);
                        rows::panel_button_style(ui);
                        let font_size = if size == toolbars::CONTROL_HEIGHT {
                            egui::TextStyle::Small.resolve(ui.style()).size
                        } else {
                            9.0
                        };
                        let mut text =
                            egui::RichText::new("R").font(crate::fonts::semibold(font_size));
                        if arm.fallback && !arm.armed {
                            text = text.color(ui.visuals().selection.stroke.color);
                        }
                        let response = ui
                            .add_sized(
                                egui::Vec2::splat(size),
                                egui::Button::new(text)
                                    .selected(arm.armed)
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(5),
                            )
                            .on_hover_text(if arm.armed {
                                "Disable MIDI reception"
                            } else {
                                "Enable MIDI reception"
                            });
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                arm.armed,
                                "Receive MIDI and recording",
                            )
                        });
                        if response.clicked() {
                            arm.armed = !arm.armed;
                        }
                    });
                    toggles
                },
            )
            .inner;
    }
    ui.allocate_ui_with_layout(
        egui::Vec2::new(2.0 * size + rows::GROUP_DIVIDER_WIDTH, size),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let background = crate::theme::unit_separator(ui.ctx());
            let group = egui::Frame::new()
                .fill(background)
                .corner_radius(5)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = rows::GROUP_DIVIDER_WIDTH;
                    ui.spacing_mut().button_padding = egui::Vec2::new(2.0, 0.0);
                    ui.spacing_mut().interact_size = egui::Vec2::splat(size);
                    rows::panel_button_style(ui);
                    ui.horizontal(|ui| {
                        std::array::from_fn(|index| {
                            let (label, name, selected) = if index == 0 {
                                ("M", "Mute", &mut *muted)
                            } else {
                                ("S", "Solo", &mut *soloed)
                            };
                            let font_size = if size == toolbars::CONTROL_HEIGHT {
                                egui::TextStyle::Small.resolve(ui.style()).size
                            } else {
                                9.0
                            };
                            let text =
                                egui::RichText::new(label).font(crate::fonts::semibold(font_size));
                            let response = ui
                                .add_sized(
                                    egui::Vec2::splat(size),
                                    egui::Button::new(text)
                                        .selected(*selected)
                                        .stroke(egui::Stroke::NONE)
                                        .corner_radius(if index == 0 {
                                            egui::CornerRadius {
                                                nw: 5,
                                                sw: 5,
                                                ne: 0,
                                                se: 0,
                                            }
                                        } else {
                                            egui::CornerRadius {
                                                nw: 0,
                                                sw: 0,
                                                ne: 5,
                                                se: 5,
                                            }
                                        }),
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
                    })
                    .inner
                });
            let buttons: [Response; 2] = group.inner;
            rows::group_separator(ui, group.response.rect, buttons[1].rect.left());
            buttons
        },
    )
    .inner
}

pub(super) fn track_mute_solo(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    arm: Option<&mut crate::midi::Arm>,
) -> [Response; 2] {
    mute_solo_arm_with_size(
        ui,
        muted,
        soloed,
        ["Mute", "Solo"],
        toolbars::CONTROL_HEIGHT,
        arm,
    )
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
    header_row_with_arm(ui, gain, pan, muted, soloed, tooltips, None)
}
fn header_row_with_arm(
    ui: &mut Ui,
    gain: &mut f32,
    pan: &mut f32,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
    arm: Option<&mut crate::midi::Arm>,
) -> (GainPanResponse, [Response; 2]) {
    let editable = arm.as_ref().is_none_or(|a| a.editable);
    rows::centered(ui, knobs::SIZE, |ui| {
        let toggles =
            mute_solo_arm_with_size(ui, muted, soloed, tooltips, toolbars::CONTROL_HEIGHT, arm);
        let knobs = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = KNOB_GAP;
                let gain = ui
                    .add_enabled_ui(editable, |ui| knobs::header_gain(ui, gain))
                    .inner;
                let pan = ui
                    .add_enabled_ui(editable, |ui| knobs::header_pan(ui, pan))
                    .inner;
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

/// Compact Timeline/Editor headers expose only Mute and Solo.
pub(super) fn track_header_row(
    ui: &mut Ui,
    mix: (&mut f32, &mut f32),
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
    compact: bool,
    arm: Option<&mut crate::midi::Arm>,
) -> (GainPanResponse, [Response; 2]) {
    if compact {
        rows::centered(ui, knobs::SIZE, |ui| {
            (
                GainPanResponse {
                    bounds: [Rect::NOTHING; 2],
                },
                mute_solo_arm_with_size(ui, muted, soloed, tooltips, toolbars::CONTROL_HEIGHT, arm),
            )
        })
        .inner
    } else {
        header_row_with_arm(ui, mix.0, mix.1, muted, soloed, tooltips, arm)
    }
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

/// Fixed-width M/S controls for the catalog-height Tracks row.
pub(super) const BROWSER_BUTTON_SIZE: f32 = 16.0;
pub(super) const BROWSER_WIDTH: f32 = 2.0 * BROWSER_BUTTON_SIZE + rows::GROUP_DIVIDER_WIDTH;
pub(super) fn browser_row(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    arm: Option<&mut crate::midi::Arm>,
) -> [Response; 2] {
    mute_solo_arm_with_size(
        ui,
        muted,
        soloed,
        ["Mute", "Solo"],
        BROWSER_BUTTON_SIZE,
        arm,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Vec2};

    #[test]
    fn midi_arm_uses_a_separate_group_on_the_mute_solo_row() {
        for (size, fallback) in [
            (toolbars::CONTROL_HEIGHT, false),
            (toolbars::CONTROL_HEIGHT, true),
            (BROWSER_BUTTON_SIZE, false),
            (BROWSER_BUTTON_SIZE, true),
        ] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            crate::theme::configure(&ctx);
            let mut arm = crate::midi::Arm {
                armed: false,
                fallback,
                editable: true,
            };
            let mut buttons = [Rect::NOTHING; 2];
            let mut output = ctx.run_ui(Default::default(), |ui| {
                buttons = mute_solo_arm_with_size(
                    ui,
                    &mut false,
                    &mut false,
                    ["Mute", "Solo"],
                    size,
                    Some(&mut arm),
                )
                .map(|r| r.rect);
            });
            output.textures_delta.clear();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if ["M", "S", "R"].contains(&text.galley.text()) => {
                        Some(text)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(labels.len(), 3);
            assert!((labels[1].pos.y - labels[2].pos.y).abs() < 0.1);
            if fallback {
                assert_eq!(
                    labels[2].galley.job.sections[0].format.color,
                    ctx.global_style().visuals.selection.stroke.color
                );
            }
            let r = labels[2].pos + labels[2].galley.rect.center().to_vec2();
            let surface = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.contains(r) && (rect.rect.width() - size).abs() < 0.1 =>
                    {
                        Some(rect)
                    }
                    _ => None,
                })
                .expect("R has its own button surface");
            assert_eq!(surface.rect.size(), Vec2::splat(size));
            assert_eq!(surface.rect.center().y, buttons[1].center().y);
            assert!(
                (surface.rect.left() - buttons[1].right() - GROUP_GAP).abs() < 0.1,
                "R group gap: {}",
                surface.rect.left() - buttons[1].right()
            );
            assert_ne!(surface.fill, ctx.global_style().visuals.selection.bg_fill);
            assert_eq!(surface.corner_radius, egui::CornerRadius::same(5));
            output.textures_delta.clear();
        }
    }

    #[test]
    fn joined_mute_solo_keep_independent_toggles_and_disabled_guards() {
        for size in [toolbars::CONTROL_HEIGHT, BROWSER_BUTTON_SIZE] {
            for theme in [
                crate::settings::ColorTheme::Dark,
                crate::settings::ColorTheme::Light,
                crate::settings::ColorTheme::Mist,
            ] {
                let ctx = egui::Context::default();
                crate::fonts::configure(&ctx);
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
                    let pair = bounds[0].union(bounds[1]);
                    assert!(
                        output.shapes.iter().all(|shape| match &shape.shape {
                            egui::Shape::Text(text) if ["M", "S"].contains(&text.galley.text()) =>
                                text.galley.job.sections.iter().all(|section| section
                                    .format
                                    .font_id
                                    .family
                                    == crate::fonts::semibold(11.0).family),
                            _ => true,
                        }),
                        "M/S labels use the shared semibold font"
                    );
                    assert!(
                        output.shapes.iter().all(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.rect.intersects(pair) =>
                                rect.stroke.is_empty(),
                            _ => true,
                        }),
                        "M/S controls have no outer, hover or active outline"
                    );
                    if enabled {
                        assert!(
                            output.shapes.iter().any(|shape| matches!(&shape.shape,
                                egui::Shape::Rect(rect) if rect.rect.contains_rect(pair)
                                    && rect.fill == crate::theme::unit_separator(&ctx)
                                    && rect.stroke.is_empty()
                                && rect.corner_radius == egui::CornerRadius::same(5)
                            )),
                            "the pair uses the unit content background, not the strip card"
                        );
                        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::LineSegment { points, stroke }
                                if points[0].x == bounds[0].right() + 0.5
                                    && points[1].x == bounds[0].right() + 0.5
                                    && points[0].y <= pair.top()
                                    && points[1].y >= pair.bottom()
                                    && *stroke == egui::Stroke::new(1.0, crate::theme::unit_header_fill(&ctx, false))
                        )), "the separator spans the pair in the unit-header color");
                    }
                    output.textures_delta.clear();
                    (bounds, muted, soloed)
                };
                let (bounds, _, _) = run(vec![], true);
                assert!(
                    (bounds[1].left() - bounds[0].right() - 1.0).abs() < 0.01,
                    "the divider occupies its own point between the buttons"
                );
                assert!((bounds[0].height() - size).abs() < 0.01);
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
                    run(vec![egui::Event::PointerMoved(point)], enabled);
                    run(vec![], enabled);
                    run(vec![egui::Event::PointerMoved(point), event(true)], enabled);
                    run(vec![], enabled);
                    let (_, mute, solo) = run(vec![event(false)], enabled);
                    assert_eq!((mute, solo), expected);
                    run(vec![], enabled);
                }
            }
        }
    }
}
