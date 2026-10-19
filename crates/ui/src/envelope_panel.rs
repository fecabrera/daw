use crate::{icons, knobs, midi_editor::UNIT_STRIP_WIDTH, panels, rows, theme};
use daw_core::{CurveShape, EnvelopeMode, EnvelopeSettings, EnvelopeStageSettings, EnvelopeTiming};
use egui::{Rect, Sense, Ui, Vec2};

use crate::modulation_panel;
use daw_core::ModulationParameter as Parameter;
use panels::unit_control_height as control_height;
#[derive(Default)]
pub(super) struct Controls<'a> {
    pub sources: &'a [modulation_panel::Source],
    pub live: Option<daw_engine::ModulationSnapshot>,
}

/// A generic note-driven modulator. Connections belong to the receiving unit.
pub(super) fn unit(
    ui: &mut Ui,
    envelope: &mut daw_core::EnvelopeInstance,
    number: usize,
    shared_only: bool,
    height: f32,
    compact: bool,
    controls: Controls<'_>,
) -> Option<panels::UnitResponse<[Rect; 4]>> {
    if !envelope.present {
        return None;
    }
    let name = format!("Envelope {number}");
    let mut enabled = envelope.enabled;
    let mut collapsed = envelope.collapsed;
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
                &mut enabled,
                &mut collapsed,
                |ui| body(ui, envelope, shared_only, 10, compact, &controls),
            )
        })
        .inner;
    envelope.enabled = enabled;
    envelope.collapsed = collapsed;
    modulation_panel::toggle(
        ui,
        &result.header,
        envelope.collapsed,
        &mut envelope.modulation_open,
    );
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
    envelope: &mut daw_core::EnvelopeInstance,
    shared_only: bool,
    margin: i8,
    compact: bool,
    controls: &Controls,
) -> [Rect; 4] {
    let defaults = EnvelopeSettings::default();
    let base = envelope.settings.parameter_values();
    let markers: Vec<_> = Parameter::ENVELOPE
        .iter()
        .enumerate()
        .map(|(index, &parameter)| {
            modulation_panel::marker(
                &envelope.modulations,
                controls.sources,
                parameter,
                base[index],
                envelope.enabled,
                None,
                [
                    controls.live.map(|v| v[index * 2]),
                    controls.live.map(|v| v[index * 2 + 1]),
                ],
            )
        })
        .collect();
    let open = envelope.modulation_open;
    let settings = &mut envelope.settings;
    let mode = &mut envelope.mode;
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
                        let local_inputs = envelope
                            .modulations
                            .iter()
                            .filter_map(|r| r.source)
                            .any(|id| {
                                controls.sources.iter().any(|s| {
                                    s.id == id && s.scope == daw_unit_core::ModulationScope::PerNote
                                })
                            });
                        note_mode(ui, mode, shared_only, local_inputs)
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
                                    relative.then_some(&mut settings.attack_note),
                                    [
                                        if relative {
                                            Parameter::EnvelopeAttackNote
                                        } else {
                                            Parameter::EnvelopeAttack
                                        },
                                        Parameter::EnvelopeAttackCurve,
                                    ],
                                    &markers,
                                    &mut envelope.modulation_open,
                                ),
                                1 => stage_controls(
                                    ui,
                                    &mut settings.decay,
                                    defaults.decay,
                                    relative.then_some(&mut settings.decay_note),
                                    [
                                        if relative {
                                            Parameter::EnvelopeDecayNote
                                        } else {
                                            Parameter::EnvelopeDecay
                                        },
                                        Parameter::EnvelopeDecayCurve,
                                    ],
                                    &markers,
                                    &mut envelope.modulation_open,
                                ),
                                3 => stage_controls(
                                    ui,
                                    &mut settings.release,
                                    defaults.release,
                                    None,
                                    [Parameter::EnvelopeRelease, Parameter::EnvelopeReleaseCurve],
                                    &markers,
                                    &mut envelope.modulation_open,
                                ),
                                _ => {
                                    parameter_knob(
                                        ui,
                                        &mut settings.sustain,
                                        Parameter::EnvelopeSustain,
                                        &markers,
                                        &mut envelope.modulation_open,
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
        if open {
            panels::unit_divider(ui, ui.cursor().left(), bounds[0].y_range().into());
            modulation_panel::body(
                ui,
                &mut envelope.modulations,
                controls.sources,
                &Parameter::ENVELOPE,
                (*mode).into(),
                compact,
            );
        }
    });
    bounds
}

fn parameter_knob(
    ui: &mut Ui,
    value: &mut f32,
    parameter: Parameter,
    markers: &[Option<knobs::ModulationMarker>],
    open: &mut bool,
) {
    let index = Parameter::ENVELOPE
        .iter()
        .position(|p| *p == parameter)
        .unwrap();
    knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, value| {
        ui.add(
            knobs::Knob::new(value, crate::parameter_knobs::destination(parameter))
                .modulated(markers[index].as_ref(), open),
        )
    });
}
fn stage_controls(
    ui: &mut Ui,
    stage: &mut EnvelopeStageSettings,
    reset: EnvelopeStageSettings,
    relative: Option<&mut f32>,
    parameters: [Parameter; 2],
    markers: &[Option<knobs::ModulationMarker>],
    open: &mut bool,
) {
    parameter_knob(
        ui,
        relative.unwrap_or(&mut stage.time_ms),
        parameters[0],
        markers,
        open,
    );
    let falling = !matches!(
        parameters[0],
        Parameter::EnvelopeAttack | Parameter::EnvelopeAttackNote
    );
    knobs::row(ui, &mut stage.shape, knobs::CHOICE_SIZE, |ui, value| {
        knobs::curve_shape(ui, value, reset.shape, falling)
    });
    ui.add_enabled_ui(stage.shape != CurveShape::Linear, |ui| {
        parameter_knob(ui, &mut stage.amount, parameters[1], markers, open)
    });
}

fn timing(ui: &mut Ui, timing: &mut EnvelopeTiming) -> egui::Response {
    let spec = knobs::KnobSpec::new("Envelope timing", 0.0..=1.0, 0.0, 1.0)
        .two_way_labels("Time", "Note")
        .format(|v| EnvelopeTiming::ALL[v.round() as usize].label().into());
    crate::lfo_controls::choice(ui, timing, &EnvelopeTiming::ALL, spec).on_hover_text(
        "Time: Attack and Decay use milliseconds. Note: use percentages of the triggering note's duration. Release always uses milliseconds.",
    )
}
fn note_mode(
    ui: &mut Ui,
    mode: &mut EnvelopeMode,
    shared_only: bool,
    local_inputs: bool,
) -> egui::Response {
    egui::ComboBox::from_id_salt("envelope-mode")
            .selected_text(egui::RichText::new(mode.label()).size(panels::HEADER_FONT_SIZE))
            .width(crate::modulation_panel::SELECTOR_WIDTH)
            .truncate()
            .show_ui(ui, |ui| {
                for choice in EnvelopeMode::ALL {
                    ui.add_enabled_ui((!shared_only || choice != EnvelopeMode::PerNote) && (!local_inputs || choice == EnvelopeMode::PerNote), |ui| {
                        ui.selectable_value(mode, choice, choice.label())
                    }).inner.on_disabled_hover_text(
                        "Per note is unavailable on Main or when shared destinations use this Envelope. Shared modes cannot consume Per note inputs.",
                    );
                }
            }).response.on_hover_text(
        "Per note: independent envelope and duration. Retrigger: restart for every note using its length. Legato: keep the first held note's envelope and duration until the last note ends.",
    )
}
