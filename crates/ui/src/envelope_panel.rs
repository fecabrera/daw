use crate::{icons, knobs, midi_editor::UNIT_STRIP_WIDTH, panels, rows, theme};
use daw_core::{CurveShape, EnvelopeMode, EnvelopeSettings, EnvelopeStageSettings, EnvelopeTiming};
use egui::{Rect, Sense, Ui, Vec2};

use panels::unit_control_height as control_height;

/// A generic note-driven modulator. Connections belong to the receiving unit.
pub(super) fn unit(
    ui: &mut Ui,
    envelope: &mut daw_core::EnvelopeInstance,
    number: usize,
    shared_only: bool,
    height: f32,
    compact: bool,
) -> Option<panels::UnitResponse<[Rect; 4]>> {
    if !envelope.present {
        return None;
    }
    let name = format!("Envelope {number}");
    let result = ui
        .scope_builder(egui::UiBuilder::new().id_salt(envelope.id), |ui| {
            panels::PanelStyle {
                margin: 0,
                ..Default::default()
            }
            .unit(
                ui,
                (&name, compact),
                height,
                &mut envelope.enabled,
                &mut envelope.collapsed,
                |ui| {
                    body(
                        ui,
                        &mut envelope.settings,
                        &mut envelope.mode,
                        shared_only,
                        10,
                        compact,
                    )
                },
            )
        })
        .inner;
    result.header.context_menu(|ui| {
        if ui.button("Remove envelope").clicked() {
            envelope.present = false;
            ui.close();
        }
    });
    Some(result)
}

pub(super) fn body_height(compact: bool) -> f32 {
    if compact {
        crate::ROW_HEIGHT
            - panels::HEADER_HEIGHT
            - 2.0
            - 2.0 * f32::from(panels::PanelStyle::default().margin)
    } else {
        control_height()
    }
}

/// One timing/mode column precedes the four stage containers; all share the oscillator geometry.
pub(super) fn body(
    ui: &mut Ui,
    settings: &mut EnvelopeSettings,
    mode: &mut EnvelopeMode,
    shared_only: bool,
    margin: i8,
    compact: bool,
) -> [Rect; 4] {
    let defaults = EnvelopeSettings::default();
    let mut bounds = [Rect::NOTHING; 4];
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        let strip = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(margin));
                ui.set_min_height(body_height(compact));
                if !compact {
                    knobs::row(ui, &mut settings.timing, knobs::CHOICE_SIZE, timing);
                    crate::modulation_panel::selector_row(ui, "Mode", |ui| {
                        note_mode(ui, mode, shared_only)
                    });
                    ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), knobs::CHOICE_SIZE),
                        Sense::hover(),
                    );
                }
                rows::centered(
                    ui,
                    if compact {
                        body_height(true)
                    } else {
                        icons::COMPACT_BUTTON_SIZE
                    },
                    |ui| rows::unit_label(ui, "Timing"),
                );
            });
        });
        panels::unit_divider(
            ui,
            strip.response.rect.right(),
            strip.response.rect.y_range().into(),
        );
        let relative = settings.timing == EnvelopeTiming::Note;
        for (index, name) in ["Attack", "Decay", "Sustain", "Release"]
            .into_iter()
            .enumerate()
        {
            let strip = ui.push_id(("envelope-stage", index), |ui| {
                egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(margin));
                        ui.set_min_height(body_height(compact));
                        if !compact {
                            match index {
                                0 => stage_controls(
                                    ui,
                                    &mut settings.attack,
                                    defaults.attack,
                                    "Attack time",
                                    false,
                                    relative.then_some((
                                        &mut settings.attack_note,
                                        defaults.attack_note,
                                    )),
                                ),
                                1 => stage_controls(
                                    ui,
                                    &mut settings.decay,
                                    defaults.decay,
                                    "Decay time",
                                    true,
                                    relative
                                        .then_some((&mut settings.decay_note, defaults.decay_note)),
                                ),
                                3 => stage_controls(
                                    ui,
                                    &mut settings.release,
                                    defaults.release,
                                    "Release time",
                                    true,
                                    None,
                                ),
                                _ => {
                                    knobs::row(
                                        ui,
                                        &mut settings.sustain,
                                        knobs::CHOICE_SIZE,
                                        knobs::sustain,
                                    );
                                    for _ in 0..2 {
                                        ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), knobs::CHOICE_SIZE),
                                            Sense::hover(),
                                        );
                                    }
                                }
                            }
                        }
                        let footer_height = if compact {
                            body_height(true)
                        } else {
                            icons::COMPACT_BUTTON_SIZE
                        };
                        rows::centered(ui, footer_height, |ui| {
                            ui.add_sized(
                                Vec2::new(ui.available_width(), icons::COMPACT_BUTTON_SIZE),
                                egui::Label::new(
                                    egui::RichText::new(name).size(panels::HEADER_FONT_SIZE),
                                )
                                .halign(egui::Align::Center),
                            );
                        });
                    })
                    .inner
                })
            });
            bounds[index] = strip.response.rect;
            if index > 0 {
                panels::unit_divider(
                    ui,
                    strip.response.rect.left(),
                    bounds[index - 1].top()..=strip.response.rect.bottom(),
                );
            }
        }
    });
    bounds
}

fn stage_controls(
    ui: &mut Ui,
    stage: &mut EnvelopeStageSettings,
    reset: EnvelopeStageSettings,
    name: &'static str,
    falling: bool,
    relative: Option<(&mut f32, f32)>,
) {
    if let Some((value, reset)) = relative {
        knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, value| {
            let spec = knobs::KnobSpec::new(name, 0.0..=1.0, reset, 0.01)
                .value_label(note_percent)
                .format(note_percent)
                .endpoint_labels("0%", "100%");
            ui.add(knobs::Knob::new(value, spec)).on_hover_text(
                "Percentage of the triggering note's full duration. Unknown note lengths use the retained Time setting.",
            )
        });
    } else {
        knobs::row(ui, &mut stage.time_ms, knobs::CHOICE_SIZE, |ui, value| {
            knobs::envelope_time(ui, value, name, reset.time_ms)
        });
    }
    knobs::row(ui, &mut stage.shape, knobs::CHOICE_SIZE, |ui, value| {
        knobs::curve_shape(ui, value, reset.shape, falling)
    });
    ui.add_enabled_ui(stage.shape != CurveShape::Linear, |ui| {
        knobs::row(
            ui,
            &mut stage.amount,
            knobs::CHOICE_SIZE,
            knobs::curve_amount,
        );
    });
}

fn note_percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}
fn timing(ui: &mut Ui, timing: &mut EnvelopeTiming) -> egui::Response {
    let spec = knobs::KnobSpec::new("Envelope timing", 0.0..=1.0, 0.0, 1.0)
        .two_way_labels("Time", "Note")
        .format(|v| EnvelopeTiming::ALL[v.round() as usize].label().into());
    crate::lfo_controls::choice(ui, timing, &EnvelopeTiming::ALL, spec).on_hover_text(
        "Time: Attack and Decay use milliseconds. Note: use percentages of the triggering note's duration. Release always uses milliseconds.",
    )
}
fn note_mode(ui: &mut Ui, mode: &mut EnvelopeMode, shared_only: bool) -> egui::Response {
    egui::ComboBox::from_id_salt("envelope-mode")
            .selected_text(egui::RichText::new(mode.label()).size(panels::HEADER_FONT_SIZE))
            .width(crate::modulation_panel::SELECTOR_WIDTH)
            .truncate()
            .show_ui(ui, |ui| {
                for choice in EnvelopeMode::ALL {
                    ui.add_enabled_ui(!shared_only || choice != EnvelopeMode::PerNote, |ui| {
                        ui.selectable_value(mode, choice, choice.label())
                    }).inner.on_disabled_hover_text(
                        "Per note is unavailable on Main and for shared modulation consumers.",
                    );
                }
            }).response.on_hover_text(
        "Per note: independent envelope and duration. Retrigger: restart for every note using its length. Legato: keep the first held note's envelope and duration until the last note ends.",
    )
}
