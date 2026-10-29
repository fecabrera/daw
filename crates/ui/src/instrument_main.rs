//! Fixed instrument output row. Component bodies, mix controls and catalog gestures are shared.
use super::*;
use daw_core::InstrumentBusSettings;

#[cfg(test)]
pub(super) struct MainBounds {
    pub row: Rect,
    pub label: Rect,
    pub icon: Rect,
    pub name: Rect,
    pub mix: [Rect; 1],
    pub collapse: Rect,
    pub toolbar: Rect,
    pub envelopes: Vec<Rect>,
    pub lfos: Vec<Rect>,
}
#[cfg(test)]
impl Default for MainBounds {
    fn default() -> Self {
        Self {
            row: Rect::NOTHING,
            label: Rect::NOTHING,
            icon: Rect::NOTHING,
            name: Rect::NOTHING,
            mix: [Rect::NOTHING; 1],
            collapse: Rect::NOTHING,
            toolbar: Rect::NOTHING,
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
                        .with_tag(panels::SHARED_LEFT_BORDER_TAG)
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
            self.instrument_ui.select_layer(bus.id);
            self.instrument_ui.track_effects_target = false;
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
                let toolbar = lane_toolbar::bounds(rect);
                let collapse = lane_toolbar::show(
                    ui, toolbar, &mut bus.collapsed,
                    !self.instrument_ui.has_layer_gesture() && !self.instrument_ui.editing_name()
                        && self.editing_enabled() && ui.input(|input| input.focused),
                    [icons::Lucide::ArrowUpFromLine, icons::Lucide::ArrowDownFromLine],
                    ["Expand Output", "Collapse Output"],
                );
                if collapse.clicked() { ui.ctx().request_repaint(); }
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
                let levels = self.output.as_ref().and_then(|o| o.layer_meter(track, bus.id)).map_or([(0.0, false); 2], |m| m.levels());
                let cleared = meters::stereo_panel(&mut header, ROW_HEIGHT - 14.0, levels, |header| {
                rows::centered(header, toolbars::CONTROL_HEIGHT, |ui| {
                    let icon = icons::header_icon_with_rotation(ui, icons::Lucide::Merge, Sense::hover(), std::f32::consts::FRAC_PI_2).on_hover_text("Instrument output");
                    icon.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), "Instrument output"));
                    let name = ui.label(egui::RichText::new(InstrumentBusSettings::DISPLAY_NAME).size(rows::NAME_FONT_SIZE));
                    #[cfg(test)] { bounds.icon = icon.rect; bounds.name = name.rect; }
                    #[cfg(not(test))] let _ = name;
                });
                let gain = rows::centered(header, knobs::SIZE, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| knobs::header_gain(ui, &mut bus.gain_db)).inner
                }).inner;
                #[cfg(test)] { bounds.mix = [gain.rect]; }
                #[cfg(not(test))] let _ = gain;
                });
                meters::reset_on_click(ui, self.output.as_ref().and_then(|o| o.layer_meter(track, bus.id)), cleared);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        self.instrument_ui.selected_layer == Some(bus.id),
                        "Select Output instrument bus",
                    )
                });
                #[cfg(test)]
                {
                    bounds.label = label;
                    bounds.collapse = collapse.rect;
                    bounds.toolbar = toolbar;
                }

                let components = egui::ScrollArea::horizontal()
                    .max_width((toolbar.left() - ui.next_widget_position().x).max(0.0))
                    .id_salt("main-units")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = -1.0;
                            let mut previous: Option<Rect> = None;
                            if self.component_drag.as_ref().is_some_and(|d| d.active) {
                                drag::scroll_drag(ui, drag::Axis::Horizontal, 360.0, self.edit_environment_available(), Rect::from_x_y_ranges(ui.clip_rect().x_range(), label.y_range()));
                            }
                            if self.effects_drag.as_ref().is_some_and(|d| d.active) {
                                drag::scroll_drag(ui, drag::Axis::Horizontal, 360.0, self.edit_environment_available(), Rect::from_x_y_ranges(ui.clip_rect().x_range(), label.y_range()));
                            }
                            for component in bus.visible_units() {
                                let selected = self.instrument_ui.selected_components.iter()
                                    .filter(|(id, _)| *id == bus.id)
                                    .map(|(_, component)| *component).collect();
                                let targets = crate::unit_selection::drag_group(
                                    &bus.visible_units(), &selected, component,
                                );
                                let menu = crate::unit_power::PowerMenu::new(
                                    targets.iter().filter_map(|c| crate::unit_power::bus_enabled(bus, *c).copied()),
                                );
                                let rect = ui.scope_builder(menu.scope(component), |ui| match component {
                                    daw_core::InstrumentComponent::Effect(id) => {
                                        let effect=bus.effects.iter_mut().find(|e|e.id==id).unwrap();
                                        Some(self.inline_layer_effect(ui,track,bus.id,effect,(height,compact),sources))
                                    }
                                    daw_core::InstrumentComponent::Envelope(id) => {
                                        let index = bus.envelopes.iter().position(|e| e.id == id).unwrap();
                                        {
                                        let id = bus.envelopes[index].id;
                                        crate::envelope_panel::unit(ui, &mut bus.envelopes[index], index + 1, true, height, compact, crate::envelope_panel::Controls { sources: provider_sources.iter().find(|(source, _)| source.layer == bus.id && source.unit == id).map_or(&[], |(_, sources)| sources.as_slice()), live: self.output.as_ref().and_then(|o| o.modulation_snapshot(daw_engine::ModulationTarget::Envelope { track, layer: bus.id, unit: id })) })
                                        }.map(|unit| {
                                            self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track, bus.id, component);
                                            #[cfg(test)] bounds.envelopes.push(unit.header.rect);
                                            unit.response.rect
                                        })
                                    }
                                    daw_core::InstrumentComponent::Lfo(id) => {
                                        let index = bus.lfos.iter().position(|l| l.id == id).unwrap();
                                        crate::lfo_panel::unit(ui, &mut bus.lfos[index], index + 1, height, compact, crate::lfo_panel::Controls {
                                            sources: provider_sources.iter().find(|(source, _)| source.layer == bus.id && source.unit == id).map_or(&[], |(_, sources)| sources.as_slice()),
                                            tempo: self.session.project.tempo_bpm,
                                            live: self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Lfo { track, layer: bus.id, unit: id })).map(|v| [v[0], v[1]]),
                                        }).map(|unit| {
                                            self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track, bus.id, component);
                                            #[cfg(test)] bounds.lfos.push(unit.header.rect);
                                            unit.response.rect
                                        })
                                    }
                                    daw_core::InstrumentComponent::Macros => {
                                        let live: Vec<_> = bus.macro_module.controls.iter().filter_map(|c|
                                            self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Macro { track, layer: bus.id, unit: c.id })).map(|v| (c.id, v[0]))).collect();
                                        crate::macro_panel::unit(ui, &mut bus.macro_module, height, compact, crate::macro_panel::Controls {
                                            layer: bus.id, sources, available: provider_sources, live: &live,
                                            name_edit: &mut self.instrument_ui.macro_name_edit,
                                        }).map(|unit| {
                                            self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track, bus.id, component);
                                            unit.response.rect
                                        })
                                    }
                                    _ => None,
                                }).inner;
                                if let Some(enable) = menu.action() {
                                    for target in targets {
                                        if let Some(state) = crate::unit_power::bus_enabled(bus, target) { *state = enable; }
                                    }
                                }
                                if let Some(rect) = rect {
                                    if !matches!(component,daw_core::InstrumentComponent::Effect(_)) {self.instrument_ui.component_slots.push((bus.id, component, rect));}
                                    if let Some(previous) = previous { panels::unit_divider(ui, previous.right() - 0.5, previous.top()..=rect.bottom()); }
                                    previous = Some(rect);
                                }
                            }
                            if self.effects_drag.is_some() {ui.allocate_exact_size(Vec2::new(crate::midi_editor::UNIT_STRIP_WIDTH,height),Sense::hover());}

                        });
                    });
                let viewport = Rect::from_min_size(components.inner_rect.min, Vec2::new(components.inner_rect.width(), height)).intersect(components.inner_rect).intersect(ui.clip_rect());
                self.finish_inline_layer_effects(ui, track, crate::layer_effects::InlineEffects { id: bus.id, effects: &mut bus.effects }, viewport);
                self.instrument_ui.component_viewports.push((bus.id,viewport));
                ui.advance_cursor_after_rect(toolbar);
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
