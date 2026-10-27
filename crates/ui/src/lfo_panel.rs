use crate::{
    envelope_panel, icons,
    knobs::{self, Knob, KnobSpec},
    lfo_controls, modulation_panel, panels, rows, theme,
};
use daw_unit_core::{LfoInstance, LfoPolarity, LfoRestart};
use egui::{Rect, Ui};

pub(super) struct Controls<'a> {
    pub sources: &'a [modulation_panel::Source],
    pub tempo: f32,
    pub live: Option<[f32; 2]>,
}
impl Default for Controls<'_> {
    fn default() -> Self {
        Self {
            sources: &[],
            tempo: 120.0,
            live: None,
        }
    }
}
pub(super) fn unit(
    ui: &mut Ui,
    lfo: &mut LfoInstance,
    number: usize,
    height: f32,
    compact: bool,
    controls: Controls<'_>,
) -> Option<panels::UnitResponse<[[Rect; 3]; 3]>> {
    if !lfo.present {
        return None;
    }
    let name = format!("LFO {number}");
    let mut enabled = lfo.enabled;
    let mut collapsed = lfo.collapsed;
    let open = lfo.modulation_open;
    let base_rate = lfo.settings.base_rate_hz(controls.tempo) as f32;
    let mut rate_marker = modulation_panel::marker(
        &lfo.modulations,
        controls.sources,
        daw_core::ModulationParameter::LfoRate,
        base_rate,
        lfo.enabled,
        None,
        [controls.live.map(|v| v[1]), None],
    );
    if let Some(marker) = &mut rate_marker {
        let limits = lfo.settings.rate_bounds(controls.tempo).map(|v| v as f32);
        modulation_panel::bound_marker(marker, limits, |v| format!("{v:.2} Hz"));
    }
    let depth_marker = modulation_panel::marker(
        &lfo.modulations,
        controls.sources,
        daw_core::ModulationParameter::LfoDepth,
        lfo.settings.depth,
        lfo.enabled,
        None,
        [controls.live.map(|v| v[0]), None],
    );
    let result = ui
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt(lfo.id)
                .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-note-selector").with_tag("unit-modulation-button")),
            |ui| {
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
                    |ui| {
                        let mut bounds = [[Rect::NOTHING; 3]; 3];
                        ui.horizontal_top(|ui| {
                            let body_top = ui.cursor().top();
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (strip, label) in ["Wave", "Rate", "Depth"].into_iter().enumerate() {
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
                                                        lfo,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, lfo| { let s = &mut lfo.settings; match (strip, row) {
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
                                                            (1, 1) => lfo_controls::modulated_rate(
                                                                ui,
                                                                s.rate_mode,
                                                                &mut s.rate_hz,
                                                                &mut s.note_rate,
                                                                lfo_controls::RateModulation { marker: rate_marker.as_ref(), open: &mut lfo.modulation_open, tempo: controls.tempo },
                                                            ),
                                                            (2, 0) => ui.add(Knob::new(&mut s.depth, crate::parameter_knobs::destination(daw_core::ModulationParameter::LfoDepth)).modulated(depth_marker.as_ref(), &mut lfo.modulation_open)),
                                                            (2, _) => ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), knobs::CHOICE_SIZE), egui::Sense::hover()).1,
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
                                                        } },
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
                                if strip < 2 {
                                    panels::unit_divider(
                                        ui,
                                        frame.response.rect.right(),
                                        frame.response.rect.y_range().into(),
                                    );
                                }
                            }
                            if open {
                                panels::unit_divider(ui, ui.cursor().left(), body_top..=(body_top + envelope_panel::body_height(compact) + 20.0));
                                modulation_panel::body(ui, &mut lfo.modulations, controls.sources,
                                    <LfoInstance as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS,
                                    <LfoInstance as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                            }
                        });
                        bounds
                    },
                )
            },
        )
        .inner;
    lfo.enabled = enabled;
    lfo.collapsed = collapsed;
    modulation_panel::toggle(ui, &result.header, lfo.collapsed, &mut lfo.modulation_open);
    let power_menu = crate::unit_power::PowerMenu::from_ui(ui);
    crate::menus::ContextMenu::new(&result.header).show(|ui| {
        if let Some(menu) = &power_menu {
            menu.show(ui);
        }
        if ui.button("Remove LFO").clicked() {
            lfo.present = false;
            ui.close();
        }
    });
    if power_menu
        .as_ref()
        .is_some_and(|menu| menu.action().is_some())
    {
        crate::unit_power::schedule_focus(ui, &result.header);
    }
    if !lfo.collapsed {
        let rect = Rect::from_min_max(
            egui::pos2(
                result.header.rect.right() - 114.0,
                result.header.rect.top() + 2.0,
            ),
            egui::pos2(
                result.header.rect.right() - 28.0,
                result.header.rect.top() + panels::HEADER_HEIGHT - 1.0,
            ),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .id_salt((lfo.id, "restart")),
        );
        crate::menus::Dropdown::from_id_salt("mode")
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
