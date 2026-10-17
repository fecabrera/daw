//! Fixed instrument output row. Component bodies, mix controls and catalog gestures are shared.
use super::*;
use daw_core::InstrumentBusSettings;

#[cfg(test)]
pub(super) struct MainBounds {
    pub row: Rect,
    pub label: Rect,
    pub mix: [Rect; 4],
    pub collapse: Rect,
    pub envelopes: Vec<Rect>,
    pub lfos: Vec<Rect>,
}
#[cfg(test)]
impl Default for MainBounds {
    fn default() -> Self {
        Self {
            row: Rect::NOTHING,
            label: Rect::NOTHING,
            mix: [Rect::NOTHING; 4],
            collapse: Rect::NOTHING,
            envelopes: Vec::new(),
            lfos: Vec::new(),
        }
    }
}
impl DawUi {
    pub(super) fn instrument_main(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        bus: &mut InstrumentBusSettings,
        rect: Rect,
        sources: &[crate::modulation_panel::Source],
        provider_sources: &[(
            daw_core::ModulationSourceId,
            Vec<crate::modulation_panel::Source>,
        )],
    ) {
        let compact = bus.collapsed;
        let height = panels::unit_height(compact);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((track, bus.id, "main"))
                .ui_stack_info(
                    egui::UiStackInfo::default()
                        .with_tag(panels::SHARED_BOTTOM_BORDER_TAG)
                        .with_tag(panels::SHARED_TOP_BORDER_TAG),
                )
                .max_rect(rect),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        // Main must keep its own Macro editor enabled while it has keyboard focus.
        let enabled = self.component_drag.as_ref().is_none_or(|d| !d.active)
            && !self.instrument_ui.dragging()
            && (!self.instrument_ui.editing_name()
                || self
                    .instrument_ui
                    .macro_name_edit
                    .as_ref()
                    .is_some_and(|edit| edit.on_layer(bus.id)));
        // Component presses select their owner just as they do on regular layer rows.
        if enabled
            && !self.instrument_ui.editing_name()
            && ui.is_enabled()
            && self.edit_environment_available()
            && ui.rect_contains_pointer(rect)
            && ui.input(|i| {
                i.focused
                    && i.pointer.button_pressed(egui::PointerButton::Primary)
                    && i.pointer
                        .press_origin()
                        .is_some_and(|p| rect.intersect(ui.clip_rect()).contains(p))
            })
        {
            self.instrument_ui.selected_layer = Some(bus.id);
            ui.ctx().request_repaint();
        }
        #[cfg(test)]
        let mut bounds = MainBounds {
            row: rect,
            ..Default::default()
        };
        child.add_enabled_ui(enabled, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = -1.0;
                let (label, response) =
                    ui.allocate_exact_size(Vec2::new(TRACK_WIDTH, height), Sense::click());
                ui.painter()
                    .rect_filled(label, 0.0, theme::palette(ui.ctx()).panel);
                let mut header = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(label.shrink(7.0))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                header.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                let collapse = rows::centered(&mut header, toolbars::CONTROL_HEIGHT, |ui| {
                    ui.label(egui::RichText::new("Main").size(rows::NAME_FONT_SIZE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        icons::small_button(
                            ui,
                            if compact {
                                icons::Lucide::ArrowUpFromLine
                            } else {
                                icons::Lucide::ArrowDownFromLine
                            },
                            if compact {
                                "Expand Main"
                            } else {
                                "Collapse Main"
                            },
                            ui.input(|i| i.focused),
                            theme::palette(ui.ctx()).text,
                        )
                    })
                    .inner
                })
                .inner;
                if collapse.clicked() {
                    bus.collapsed = !compact;
                    ui.ctx().request_repaint();
                }
                let inputs = self
                    .instrument_ui
                    .inputs
                    .entry(bus.id)
                    .or_insert_with(|| Inputs {
                        gain: faders::format_gain(bus.gain_db),
                        pan: format!("{:.2}", bus.pan),
                    });
                let mixing =
                    mix_controls::gain_pan(&mut header, &mut bus.gain_db, &mut bus.pan, inputs);
                if let Some(error) = mixing.error {
                    self.fail(error);
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        self.instrument_ui.selected_layer == Some(bus.id),
                        "Select Main instrument bus",
                    )
                });
                #[cfg(test)]
                {
                    bounds.label = label;
                    bounds.collapse = collapse.rect;
                    bounds.mix = mixing.bounds;
                }
                egui::ScrollArea::horizontal()
                    .id_salt("main-units")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = -1.0;
                            let mut previous: Option<Rect> = None;
                            for (i, e) in bus.envelopes.iter_mut().enumerate() {
                                if let Some(unit) =
                                    crate::envelope_panel::unit(ui, e, i + 1, true, height, compact)
                                {
                                    self.start_installed_component_drag(
                                        ui,
                                        &unit.header,
                                        &[unit.power.rect],
                                        track,
                                        bus.id,
                                        daw_core::InstrumentComponent::Envelope(e.id),
                                    );
                                    let rect = unit.response.rect;
                                    if let Some(prev) = previous {
                                        panels::unit_divider(
                                            ui,
                                            prev.right() - 0.5,
                                            prev.top()..=rect.bottom(),
                                        );
                                    }
                                    previous = Some(rect);
                                    #[cfg(test)]
                                    bounds.envelopes.push(unit.header.rect);
                                }
                            }
                            for (i, l) in bus.lfos.iter_mut().enumerate() {
                                if let Some(unit) = crate::lfo_panel::unit(
                                    ui,
                                    l,
                                    i + 1,
                                    height,
                                    compact,
                                    crate::lfo_panel::Controls {
                                        sources: provider_sources
                                            .iter()
                                            .find(|(id, _)| id.layer == bus.id && id.unit == l.id)
                                            .map_or(&[], |(_, sources)| sources.as_slice()),
                                        tempo: self.session.project.tempo_bpm,
                                        live: self
                                            .output
                                            .as_ref()
                                            .and_then(|o| {
                                                o.modulation_values(
                                                    daw_engine::ModulationTarget::Lfo {
                                                        track,
                                                        layer: bus.id,
                                                        unit: l.id,
                                                    },
                                                )
                                            })
                                            .map(|v| [v[0], v[1]]),
                                    },
                                ) {
                                    self.start_installed_component_drag(
                                        ui,
                                        &unit.header,
                                        &[unit.power.rect],
                                        track,
                                        bus.id,
                                        daw_core::InstrumentComponent::Lfo(l.id),
                                    );
                                    let rect = unit.response.rect;
                                    if let Some(prev) = previous {
                                        panels::unit_divider(
                                            ui,
                                            prev.right() - 0.5,
                                            prev.top()..=rect.bottom(),
                                        );
                                    }
                                    previous = Some(rect);
                                    #[cfg(test)]
                                    bounds.lfos.push(unit.header.rect);
                                }
                            }
                            let macro_live: Vec<_> = bus
                                .macro_module
                                .controls
                                .iter()
                                .filter_map(|c| {
                                    self.output
                                        .as_ref()
                                        .and_then(|o| {
                                            o.modulation_values(
                                                daw_engine::ModulationTarget::Macro {
                                                    track,
                                                    layer: bus.id,
                                                    unit: c.id,
                                                },
                                            )
                                        })
                                        .map(|v| (c.id, v[0]))
                                })
                                .collect();
                            if let Some(unit) = crate::macro_panel::unit(
                                ui,
                                &mut bus.macro_module,
                                height,
                                compact,
                                crate::macro_panel::Controls {
                                    layer: bus.id,
                                    sources,
                                    available: provider_sources,
                                    live: &macro_live,
                                    name_edit: &mut self.instrument_ui.macro_name_edit,
                                },
                            ) {
                                self.start_installed_component_drag(
                                    ui,
                                    &unit.header,
                                    &[unit.power.rect],
                                    track,
                                    bus.id,
                                    daw_core::InstrumentComponent::Macros,
                                );
                                let rect = unit.response.rect;
                                if let Some(prev) = previous {
                                    panels::unit_divider(
                                        ui,
                                        prev.right() - 0.5,
                                        prev.top()..=rect.bottom(),
                                    );
                                }
                            }
                        });
                    });
            });
        });
        panels::ControlPanel::pinned_row_divider(ui, rect);
        ui.advance_cursor_after_rect(rect);
        #[cfg(test)]
        {
            self.instrument_ui.main_bounds = bounds;
        }
    }
}
