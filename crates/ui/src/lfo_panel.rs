use crate::{
    envelope_panel, icons,
    knobs::{self, Knob, KnobSpec},
    lfo_controls, panels, rows, theme,
};
use daw_unit_core::{LfoInstance, LfoPolarity, LfoRestart};
use egui::{Rect, Ui};

pub(super) fn unit(
    ui: &mut Ui,
    lfo: &mut LfoInstance,
    number: usize,
    height: f32,
    compact: bool,
) -> Option<panels::UnitResponse<[[Rect; 3]; 2]>> {
    if !lfo.present {
        return None;
    }
    let name = format!("LFO {number}");
    let result = ui
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt(lfo.id)
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
                    &mut lfo.enabled,
                    &mut lfo.collapsed,
                    |ui| {
                        let mut bounds = [[Rect::NOTHING; 3]; 2];
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (strip, label) in ["Wave", "Rate"].into_iter().enumerate() {
                                let frame = egui::Frame::new().inner_margin(10).show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.y =
                                                f32::from(theme::TOOLBAR_PADDING);
                                            ui.set_width(
                                                crate::midi_editor::UNIT_STRIP_WIDTH - 20.0,
                                            );
                                            ui.set_min_height(envelope_panel::body_height(compact));
                                            if !compact {
                                                for (row, bounds) in
                                                    bounds[strip].iter_mut().enumerate()
                                                {
                                                    *bounds = knobs::row(
                                                        ui,
                                                        &mut lfo.settings,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, s| match (strip, row) {
                                                            (0, 0) => {
                                                                lfo_controls::wave(ui, &mut s.wave)
                                                            }
                                                            (0, 1) => ui.add(Knob::new(
                                                                &mut s.phase_degrees,
                                                                lfo_controls::phase_spec(
                                                                    "LFO phase",
                                                                ),
                                                            )),
                                                            (0, 2) => {
                                                                let mut value = f32::from(
                                                                    s.polarity
                                                                        == LfoPolarity::Bipolar,
                                                                );
                                                                let response = ui.add(Knob::new(
                                                                    &mut value,
                                                                    KnobSpec::new(
                                                                        "LFO polarity",
                                                                        0.0..=1.0,
                                                                        0.0,
                                                                        1.0,
                                                                    )
                                                                    .two_way_labels("Uni", "Bi")
                                                                    .format(|v| {
                                                                        if v < 0.5 {
                                                                            "Unipolar: 0 to 1"
                                                                                .into()
                                                                        } else {
                                                                            "Bipolar: -1 to 1"
                                                                                .into()
                                                                        }
                                                                    }),
                                                                ));
                                                                if response.changed() {
                                                                    s.polarity = if value < 0.5 {
                                                                        LfoPolarity::Unipolar
                                                                    } else {
                                                                        LfoPolarity::Bipolar
                                                                    };
                                                                }
                                                                response
                                                            }
                                                            (1, 0) => lfo_controls::mode(
                                                                ui,
                                                                &mut s.rate_mode,
                                                            ),
                                                            (1, 1) => lfo_controls::rate(
                                                                ui,
                                                                s.rate_mode,
                                                                &mut s.rate_hz,
                                                                &mut s.note_rate,
                                                            ),
                                                            _ => ui.add(Knob::new(
                                                                &mut s.smooth_ms,
                                                                KnobSpec::new(
                                                                    "LFO smooth",
                                                                    0.0..=100.0,
                                                                    1.0,
                                                                    0.1,
                                                                )
                                                                .value_label(|v| {
                                                                    format!("{v:.1} ms")
                                                                })
                                                                .format(|v| format!("{v:.1} ms"))
                                                                .endpoint_labels("0", "100"),
                                                            )),
                                                        },
                                                    )
                                                    .rect;
                                                }
                                            }
                                            rows::centered(
                                                ui,
                                                if compact {
                                                    envelope_panel::body_height(true)
                                                } else {
                                                    icons::COMPACT_BUTTON_SIZE
                                                },
                                                |ui| rows::unit_label(ui, label),
                                            );
                                        },
                                    );
                                });
                                if strip == 0 {
                                    panels::unit_divider(
                                        ui,
                                        frame.response.rect.right(),
                                        frame.response.rect.y_range().into(),
                                    );
                                }
                            }
                        });
                        bounds
                    },
                )
            },
        )
        .inner;
    result.header.context_menu(|ui| {
        if ui.button("Remove LFO").clicked() {
            lfo.present = false;
            ui.close();
        }
    });
    if !lfo.collapsed {
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
                .id_salt((lfo.id, "restart")),
        );
        egui::ComboBox::from_id_salt("mode")
            .selected_text(egui::RichText::new(lfo.settings.restart.label()).size(panels::HEADER_FONT_SIZE))
            .width(80.0)
            .truncate()
            .show_ui(&mut child, |ui| {
                for mode in LfoRestart::ALL {
                    ui.selectable_value(&mut lfo.settings.restart, mode, mode.label());
                }
            })
            .response
            .on_hover_text("Transport follows project time. Retrigger restarts on every note. Legato restarts on the first note of a phrase. All modes provide a shared signal.");
    }
    Some(result)
}
