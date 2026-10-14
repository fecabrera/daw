use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum EffectTarget {
    Track(Id),
    Master,
}

#[cfg(test)]
pub(crate) struct EffectsUiBounds {
    pub library: Rect,
    pub toolbar: Rect,
    pub library_toggle: Rect,
    pub lane_toggle: Rect,
    pub header: Rect,
    pub strip: Rect,
    pub scrollbar: Rect,
    pub entry: Rect,
    pub drive_entry: Rect,
    pub auto_filter_entry: Rect,
    pub compressor_entry: Rect,
    pub ott_entry: Rect,
    pub lane_toolbar: Rect,
    pub lane_compact: Rect,
    pub chain: Vec<(Id, Rect)>,
    pub lane_viewport: Rect,
    pub lane_scroll: f32,
    pub units: Vec<crate::equalizer_panel::Bounds>,
    pub drive_units: Vec<crate::drive_panel::Bounds>,
    pub auto_filter_units: Vec<crate::auto_filter_panel::Bounds>,
    pub compressor_units: Vec<crate::compressor_panel::Bounds>,
    pub ott_units: Vec<crate::ott_panel::Bounds>,
}

#[cfg(test)]
impl Default for EffectsUiBounds {
    fn default() -> Self {
        Self {
            library: Rect::NOTHING,
            toolbar: Rect::NOTHING,
            library_toggle: Rect::NOTHING,
            lane_toggle: Rect::NOTHING,
            header: Rect::NOTHING,
            strip: Rect::NOTHING,
            scrollbar: Rect::NOTHING,
            entry: Rect::NOTHING,
            drive_entry: Rect::NOTHING,
            auto_filter_entry: Rect::NOTHING,
            compressor_entry: Rect::NOTHING,
            ott_entry: Rect::NOTHING,
            lane_toolbar: Rect::NOTHING,
            lane_compact: Rect::NOTHING,
            chain: vec![],
            lane_viewport: Rect::NOTHING,
            lane_scroll: 0.0,
            units: vec![],
            drive_units: vec![],
            auto_filter_units: vec![],
            compressor_units: vec![],
            ott_units: vec![],
        }
    }
}

impl DawUi {
    pub(super) fn effects_target(&self) -> Option<EffectTarget> {
        if self.view_mode == ViewMode::Instrument {
            return self
                .selected_track
                .filter(|id| {
                    self.session
                        .project
                        .tracks
                        .iter()
                        .any(|track| track.id == *id && track.kind == daw_core::TrackKind::Midi)
                })
                .map(EffectTarget::Track);
        }
        if self.master_selected {
            Some(EffectTarget::Master)
        } else {
            self.selected_track
                .filter(|id| self.session.project.tracks.iter().any(|t| t.id == *id))
                .map(EffectTarget::Track)
        }
    }

    pub(super) fn effect_chain(
        &self,
        target: EffectTarget,
    ) -> Option<(&[daw_core::EffectInstance], bool)> {
        match target {
            EffectTarget::Master => Some((
                &self.session.project.master.effects,
                self.session.project.master.effects_compact,
            )),
            EffectTarget::Track(id) => self
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == id)
                .map(|t| (t.effects.as_slice(), t.effects_compact)),
        }
    }

    fn effects_state(&self, target: Option<EffectTarget>) -> (bool, Vec<daw_core::EffectInstance>) {
        target
            .and_then(|target| self.effect_chain(target))
            .map(|(effects, compact)| (compact, effects.to_vec()))
            .unwrap_or_default()
    }

    pub(super) fn effects_editing_enabled(&self, ui: &egui::Ui) -> bool {
        // Knob focus remains editable; text editors and other active gestures block changes.
        ui.is_enabled()
            && self.edit_environment_available()
            && self.drag.is_none()
            && self.track_drag.is_none()
            && !self.midi_editor.dragging()
            && !self.instrument_ui.dragging()
            && ui.input(|input| input.focused)
            && !self.instrument_ui.editing_name()
            && self.track_name_edit.is_none()
            && self.tempo_edit.is_none()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.loop_drag.is_none()
            && self.marquee.is_none()
            && self.selection_drag.iter().all(Option::is_none)
            && self.file_hover.is_none()
    }

    pub(super) fn effects_workspace(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        self.selected_effects.retain(|id| {
            self.session
                .project
                .all_effects()
                .any(|effect| effect.id == *id)
        });
        let mut selected = self.effects_target();
        let (mut compact, mut effects) = self.effects_state(selected);
        let allowed = self.effects_editing_enabled(ui);
        let mut original_compact = compact;
        let mut original_effects = effects.clone();
        #[cfg(test)]
        {
            self.effects_bounds.units.clear();
            self.effects_bounds.drive_units.clear();
            self.effects_bounds.auto_filter_units.clear();
            self.effects_bounds.compressor_units.clear();
            self.effects_bounds.ott_units.clear();
        }
        let border = Stroke::new(1.0, theme::palette(ui.ctx()).border);
        ui.painter()
            .hline(ui.max_rect().x_range(), ui.max_rect().top() + 0.5, border);
        ui.add_space(1.0);
        let bounds = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
        let below_track = if self.view_mode == ViewMode::Instrument {
            (bounds.height() - toolbars::HEIGHT).max(0.0)
        } else {
            (bounds.height() - RULER_HEIGHT - track_overview::STRIP_HEIGHT - SCROLLBAR_HEIGHT)
                .max(0.0)
        };
        let lane_visible = self.timeline_effects_visible;
        let strip_height = if lane_visible {
            panels::unit_height(compact).min(below_track)
        } else {
            0.0
        };
        let strip = Rect::from_min_max(
            Pos2::new(bounds.left(), bounds.bottom() - strip_height),
            bounds.max,
        );
        let upper = Rect::from_min_max(bounds.min, Pos2::new(bounds.right(), strip.top()));
        let catalog_visible = self.timeline_catalog_visible;
        let library = catalog_visible.then(|| panels::ControlPanel::begin(ui, upper));
        #[cfg(test)]
        if !catalog_visible {
            self.effects_bounds.library = Rect::NOTHING;
            self.effects_bounds.header = Rect::NOTHING;
            self.effects_bounds.entry = Rect::NOTHING;
            self.effects_bounds.drive_entry = Rect::NOTHING;
            self.effects_bounds.auto_filter_entry = Rect::NOTHING;
            self.effects_bounds.compressor_entry = Rect::NOTHING;
            self.effects_bounds.ott_entry = Rect::NOTHING;
        }
        ui.horizontal_top(|ui| {
            if let Some(library) = library.as_ref() {
            ui.allocate_ui_with_layout(
                Vec2::new(TRACK_WIDTH, upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(TRACK_WIDTH);
                    ui.set_min_height(upper.height());
                    let catalog_rect = Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, upper.height()));
                    self.effects_catalog.begin_frame(ui, catalog_rect);
                    let header = library.header(ui, "Effects", |_| {});
                    panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
                    self.effects_catalog.empty_region(ui, header.response.rect.intersect(catalog_rect), 0, allowed);
                    let catalog_allowed = allowed && self.effects_drag.is_none() && selected.is_some()
                        && !ui.input(|input| input.pointer.primary_down());
                    self.effects_catalog.retain_focus_filter(ui);
                    let catalog_action = self.effects_catalog.keyboard(ui, catalog_allowed);
                    if catalog_action == effects_catalog::Action::Add {
                        effects.extend(effects_catalog::ENTRIES.iter()
                            .filter(|(type_id, _, _)| self.effects_catalog.selection.contains(type_id))
                            .map(|(_, _, make)| make()));
                    }
                    let mut entry_rects = [Rect::NOTHING; effects_catalog::ENTRIES.len()];
                    for (index, (type_id, name, make)) in
                        effects_catalog::ENTRIES.into_iter().enumerate()
                    {
                        let response = ui
                            .add_enabled_ui(
                                allowed && self.effects_drag.as_ref().is_none_or(|drag| drag.is_library() && !drag.active) && selected.is_some(),
                                |ui| {
                                    library
                                        .catalog_item(
                                            ui,
                                            name,
                                            self.effects_catalog.selection.contains(type_id),
                                        )
                                        .on_hover_text(format!(
                                            "Double-click to add {name}. Enter adds selected effects. Shift+click toggles selection."
                                        ))
                                },
                            )
                            .inner;
                        entry_rects[index] = response.rect;
                        if allowed && lane_visible {
                            self.start_library_effect_drag(ui, &response, selected, index, &effects);
                        }
                        let pointer_clicked = response.clicked() && ui.rect_contains_pointer(response.rect) && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary));
                        if pointer_clicked {
                            self.effects_catalog
                                .click(index, ui.input(|input| input.modifiers.shift));
                            self.effects_catalog.focus_row(ui, &response);
                        }
                        if catalog_action == effects_catalog::Action::Move && self.effects_catalog.is_cursor(index) {
                            self.effects_catalog.focus_row(ui, &response);
                        }
                        if pointer_clicked && response.double_clicked() {
                            effects.push(make());
                        }
                        #[cfg(test)]
                        if type_id == daw_core::EQ_TYPE_ID {
                            self.effects_bounds.entry = response.rect;
                        }
                        #[cfg(test)]
                        if type_id == daw_core::DRIVE_TYPE_ID {
                            self.effects_bounds.drive_entry = response.rect;
                        }
                        #[cfg(test)]
                        if type_id == daw_core::AUTO_FILTER_TYPE_ID {
                            self.effects_bounds.auto_filter_entry = response.rect;
                        }
                        #[cfg(test)]
                        if type_id == daw_core::OTT_TYPE_ID { self.effects_bounds.ott_entry = response.rect; }
                        #[cfg(test)]
                        if type_id == daw_core::COMPRESSOR_TYPE_ID {
                            self.effects_bounds.compressor_entry = response.rect;
                        }
                    }
                    let empty_body = Rect::from_min_max(
                        Pos2::new(catalog_rect.left(), entry_rects.last().unwrap().bottom()), catalog_rect.max,
                    );
                    self.effects_catalog.empty_region(ui, empty_body, effects_catalog::ENTRIES.len(), allowed);
                    if allowed && self.effects_drag.is_none()
                        && ui.rect_contains_pointer(catalog_rect)
                        && !entry_rects.iter().any(|rect| ui.rect_contains_pointer(*rect))
                        && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary))
                    {
                        self.effects_catalog.clear();
                        ui.ctx().request_repaint();
                    }
                    #[cfg(test)]
                    {
                        self.effects_bounds.library =
                            Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, upper.height()));
                        self.effects_bounds.header = header.response.rect;
                    }
                },
            );
            }
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(upper.height());
                    let content_bounds = Rect::from_min_max(ui.max_rect().min, upper.max);
                    let mut content_ui = ui.new_child(egui::UiBuilder::new()
                        .id_salt(("effects-workspace-content", self.view_mode.label()))
                        .max_rect(content_bounds)
                        .layout(egui::Layout::top_down(egui::Align::Min)));
                    content_ui.set_clip_rect(content_bounds.intersect(ui.clip_rect()));
                    content_ui.add_enabled_ui(self.effects_drag.is_none(), |ui| {
                        if self.view_mode == ViewMode::Instrument {
                            self.timeline_bounds = Rect::NOTHING;
                            self.lane_bounds.clear();
                            self.file_drop_target = None;
                            self.instrument_view(ui);
                        } else {
                            self.timeline_workspace(ui);
                        }
                    });
                    #[cfg(test)]
                    { self.effects_bounds.scrollbar = if self.view_mode == ViewMode::Instrument { Rect::NOTHING } else { self.scrollbar_bounds }; }

                },
            );
        });
        if let Some(library) = library {
            library.finish(ui);
        }
        // Workspace controls can change the processing target while content is drawn.
        if self.effects_target() != selected {
            selected = self.effects_target();
            (compact, effects) = self.effects_state(selected);
            original_compact = compact;
            original_effects = effects.clone();
            self.selected_effects.clear();
            self.effects_catalog.clear();
            self.effects_drag = None;
        }
        if lane_visible {
            ui.painter()
                .rect_filled(strip, 0.0, theme::palette(ui.ctx()).background);
            // The status panel owns the single separator below the effects lane.
            panels::ControlPanel::row_dividers(ui, strip, true, false);
            let toolbar = effects_toolbar::bounds(strip);
            let response = effects_toolbar::show(
                ui,
                toolbar,
                &mut compact,
                allowed
                    && self.effects_drag.as_ref().is_none_or(|drag| !drag.active)
                    && selected.is_some(),
            );
            #[cfg(test)]
            {
                self.effects_bounds.lane_toolbar = toolbar;
                self.effects_bounds.lane_compact = response.rect;
            }
            #[cfg(not(test))]
            let _ = response;
            self.effects_lane(ui, strip, selected, &mut effects, compact, allowed);
        }
        if let Some(target) = selected
            && (effects != original_effects || compact != original_compact)
        {
            self.edit(match target {
                EffectTarget::Track(track_id) => Edit::SetEffects {
                    track_id,
                    effects,
                    compact,
                },
                EffectTarget::Master => Edit::SetMasterEffects { effects, compact },
            });
        }
        #[cfg(test)]
        {
            self.effects_bounds.strip = strip;
            if !lane_visible {
                self.effects_bounds.strip = Rect::NOTHING;
                self.effects_bounds.lane_toolbar = Rect::NOTHING;
                self.effects_bounds.lane_compact = Rect::NOTHING;
                self.effects_bounds.chain.clear();
                self.effects_bounds.lane_viewport = Rect::NOTHING;
                self.effects_bounds.lane_scroll = 0.0;
            }
        }
    }

    fn effects_lane(
        &mut self,
        ui: &mut egui::Ui,
        strip: Rect,
        target: Option<EffectTarget>,
        effects: &mut Vec<daw_core::EffectInstance>,
        compact: bool,
        allowed: bool,
    ) {
        let mut remove = None;
        let mut slots = Vec::with_capacity(effects.len());
        let order: Vec<_> = effects.iter().map(|effect| effect.id).collect();
        let content = Rect::from_min_max(strip.min, effects_toolbar::bounds(strip).left_bottom());
        let mut lane = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("effects-lane", target))
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lane.set_clip_rect(content.intersect(ui.clip_rect()));
        self.effects_lane_focus.set(&lane, strip, true);
        // A zero-area keyboard target retains pane focus without covering unit hits.
        let keyboard_focus = lane.interact(
            Rect::from_center_size(strip.center(), Vec2::ZERO),
            lane.id().with("effect-lane-keyboard-focus"),
            Sense::focusable_noninteractive(),
        );
        let scrolled = lane
            .add_enabled_ui(allowed, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("effect-chain")
                    .auto_shrink([false, false])
                    .scroll_source(egui::scroll_area::ScrollSource {
                        drag: egui::scroll_area::DragScroll::Never,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        self.scroll_effect_drag(ui);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = -1.0;
                            for effect in effects.iter_mut() {
                                let mut unit_scope = egui::UiBuilder::new().id_salt(effect.id);
                                if slots.is_empty() {
                                    unit_scope = unit_scope.ui_stack_info(
                                        egui::UiStackInfo::default()
                                            .with_tag(panels::SHARED_LEFT_BORDER_TAG),
                                    );
                                }
                                let unit = ui.scope_builder(unit_scope, |ui| {
                                    let interactive =
                                        self.effects_drag.as_ref().is_none_or(|drag| !drag.active);
                                    let (header, power) = ui
                                        .add_enabled_ui(interactive, |ui| match &mut effect.unit {
                                            daw_core::EffectSettings::Eq3(settings) => {
                                                let (_bounds, header) =
                                                    crate::equalizer_panel::show(
                                                        ui,
                                                        settings,
                                                        compact,
                                                        strip.height(),
                                                    );
                                                let power = _bounds.power;
                                                #[cfg(test)]
                                                self.effects_bounds.units.push(_bounds);
                                                (header, power)
                                            }
                                            daw_core::EffectSettings::Drive(settings) => {
                                                let (_bounds, header) = crate::drive_panel::show(
                                                    ui,
                                                    settings,
                                                    compact,
                                                    strip.height(),
                                                );
                                                let power = _bounds.power;
                                                #[cfg(test)]
                                                self.effects_bounds.drive_units.push(_bounds);
                                                (header, power)
                                            }
                                            daw_core::EffectSettings::AutoFilter(settings) => {
                                                let (_bounds, header) =
                                                    crate::auto_filter_panel::show(
                                                        ui,
                                                        settings,
                                                        compact,
                                                        strip.height(),
                                                    );
                                                let power = _bounds.power;
                                                #[cfg(test)]
                                                self.effects_bounds.auto_filter_units.push(_bounds);
                                                (header, power)
                                            }
                                            daw_core::EffectSettings::Ott(settings) => {
                                                let gains = self
                                                    .output
                                                    .as_ref()
                                                    .map_or([0.0; 3], |output| {
                                                        output.ott_gain_change_db(effect.id)
                                                    });
                                                let (_bounds, header) = crate::ott_panel::show(
                                                    ui,
                                                    settings,
                                                    compact,
                                                    strip.height(),
                                                    gains,
                                                );
                                                let power = _bounds.power;
                                                #[cfg(test)]
                                                self.effects_bounds.ott_units.push(_bounds);
                                                (header, power)
                                            }
                                            daw_core::EffectSettings::Missing {
                                                type_id, ..
                                            } => {
                                                let response = ui.add(
                                                    egui::Label::new(format!(
                                                        "Missing effect: {type_id} (bypassed)"
                                                    ))
                                                    .sense(Sense::click_and_drag()),
                                                );
                                                (response, Rect::NOTHING)
                                            }
                                            daw_core::EffectSettings::Compressor(settings) => {
                                                let reduction =
                                                    self.output.as_ref().map_or(0.0, |output| {
                                                        output.gain_reduction_db(effect.id)
                                                    });
                                                let (_bounds, header) =
                                                    crate::compressor_panel::show(
                                                        ui,
                                                        settings,
                                                        compact,
                                                        strip.height(),
                                                        reduction,
                                                    );
                                                let power = _bounds.power;
                                                #[cfg(test)]
                                                self.effects_bounds.compressor_units.push(_bounds);
                                                (header, power)
                                            }
                                        })
                                        .inner;
                                    if allowed && interactive {
                                        self.start_effect_drag(
                                            ui, &header, power, target, effect.id, &order,
                                        );
                                        header
                                            .on_hover_cursor(egui::CursorIcon::Grab)
                                            .context_menu(|ui| {
                                                if ui
                                                    .add_enabled(
                                                        self.effects_drag.is_none(),
                                                        egui::Button::new("Remove effect"),
                                                    )
                                                    .clicked()
                                                {
                                                    remove = Some(effect.id);
                                                    ui.close();
                                                }
                                            });
                                    }
                                });
                                let rect = unit.response.rect;
                                if allowed
                                    && self.effects_drag.as_ref().is_none_or(|drag| !drag.active)
                                    && ui.rect_contains_pointer(rect)
                                    && ui.input(|input| {
                                        input.focused
                                            && input
                                                .pointer
                                                .button_clicked(egui::PointerButton::Primary)
                                    })
                                {
                                    self.select_effect(
                                        effect.id,
                                        ui.input(|input| input.modifiers.shift),
                                    );
                                    if !self.effects_lane_focus.owns(ui.ctx()) {
                                        unit.response.request_focus();
                                    }
                                }
                                slots.push((effect.id, rect));
                            }
                        });
                        // Paint last so adjacent units cannot cover a selected outline.
                        for (id, rect) in &slots {
                            if self.selected_effects.contains(id) {
                                panels::selection_border(ui, *rect);
                            }
                        }
                    })
            })
            .inner;
        #[cfg(test)]
        {
            self.effects_bounds.chain = slots.clone();
            self.effects_bounds.lane_viewport = scrolled.inner_rect.intersect(strip);
            self.effects_bounds.lane_scroll = scrolled.state.offset.x;
        }
        if allowed
            && self.effects_drag.is_none()
            && lane.rect_contains_pointer(scrolled.inner_rect.intersect(strip))
            && lane.input(|input| {
                input.focused && input.pointer.button_clicked(egui::PointerButton::Primary)
            })
            && !slots
                .iter()
                .any(|(_, rect)| lane.rect_contains_pointer(*rect))
        {
            self.selected_effects.clear();
        }
        self.delete_effect_selection(&lane, effects, allowed, &keyboard_focus);
        self.finish_effect_drag(
            &lane,
            effects,
            &slots,
            scrolled.inner_rect.intersect(strip),
            allowed,
        );

        if let Some(id) = remove {
            effects.retain(|effect| effect.id != id);
        }
    }

    fn delete_effect_selection(
        &mut self,
        ui: &egui::Ui,
        effects: &mut Vec<daw_core::EffectInstance>,
        allowed: bool,
        keyboard_focus: &egui::Response,
    ) {
        if !ui.input(|input| {
            input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
        }) || self.effects_drag.is_some()
            || ui.input(|input| input.pointer.any_down())
            || self.effects_lane_focus.lost(ui.ctx(), allowed, false)
        {
            return;
        }
        let delete = ui.input_mut(|input| {
            let mut delete = false;
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key: egui::Key::Delete | egui::Key::Backspace,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                    && !modifiers.alt
                    && !modifiers.ctrl
                    && !modifiers.command
                    && !modifiers.mac_cmd
                {
                    // Consume repeats too, so the same key cannot reach clip shortcuts.
                    delete |= !repeat;
                    return false;
                }
                true
            });
            delete
        });
        if delete && !self.selected_effects.is_empty() {
            effects.retain(|effect| !self.selected_effects.contains(&effect.id));
            self.selected_effects.clear();
            // Keep focus on the pane after its focused unit disappears. A held Delete
            // must not fall through to the selected clip in the track overview.
            keyboard_focus.request_focus();
            ui.ctx().request_repaint();
        }
    }
}
