use crate::{icons, knobs, midi_editor::UNIT_STRIP_WIDTH, panels, rows, theme};
use daw_core::{CurveShape, EnvelopeSettings, EnvelopeStageSettings};
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
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt(envelope.id)
                .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-note-selector")),
            |ui| {
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
                    |ui| body(ui, &mut envelope.settings, 10, compact),
                )
            },
        )
        .inner;
    result.header.context_menu(|ui| {
        if ui.button("Remove envelope").clicked() {
            envelope.present = false;
            ui.close();
        }
    });
    if !envelope.collapsed {
        let rect = Rect::from_min_max(
            egui::pos2(
                result.header.rect.right() - 88.0,
                result.header.rect.top() + 2.0,
            ),
            egui::pos2(
                result.header.rect.right() - 2.0,
                result.header.rect.top() + panels::HEADER_HEIGHT - 1.0,
            ),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .id_salt((envelope.id, "note-mode")),
        );
        egui::ComboBox::from_id_salt("mode")
            .selected_text(egui::RichText::new(envelope.mode.label()).size(panels::HEADER_FONT_SIZE)).width(80.0).truncate()
            .show_ui(&mut child, |ui| {
                for mode in daw_core::EnvelopeMode::ALL {
                    ui.add_enabled_ui(!shared_only || mode!=daw_core::EnvelopeMode::PerNote, |ui| {
                        ui.selectable_value(&mut envelope.mode, mode, mode.label())
                    }).inner.on_disabled_hover_text("Disconnect this envelope from track effects before selecting Per note.");
                }
            }).response.on_hover_text("Per note: independent ADSR. Retrigger: restart on every note. Legato: restart only for the first held note. Shared modes release after the last held note.");
    }
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

/// Four stage containers share the oscillator's row, footer, and padding geometry.
pub(super) fn body(
    ui: &mut Ui,
    settings: &mut EnvelopeSettings,
    margin: i8,
    compact: bool,
) -> [Rect; 4] {
    let defaults = EnvelopeSettings::default();
    let mut bounds = [Rect::NOTHING; 4];
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
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
                                ),
                                1 => stage_controls(
                                    ui,
                                    &mut settings.decay,
                                    defaults.decay,
                                    "Decay time",
                                    true,
                                ),
                                3 => stage_controls(
                                    ui,
                                    &mut settings.release,
                                    defaults.release,
                                    "Release time",
                                    true,
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
) {
    knobs::row(ui, &mut stage.time_ms, knobs::CHOICE_SIZE, |ui, value| {
        knobs::envelope_time(ui, value, name, reset.time_ms)
    });
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
