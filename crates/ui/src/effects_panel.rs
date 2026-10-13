use super::*;

#[cfg(test)]
pub(crate) struct EffectsUiBounds {
    pub library: Rect,
    pub header: Rect,
    pub strip: Rect,
    pub scrollbar: Rect,
    pub entry: Rect,
    pub drive_entry: Rect,
    pub auto_filter_entry: Rect,
    pub compact: Rect,
    pub chain: Vec<(Id, Rect)>,
    pub lane_viewport: Rect,
    pub lane_scroll: f32,
    pub units: Vec<crate::equalizer_panel::Bounds>,
    pub drive_units: Vec<crate::drive_panel::Bounds>,
    pub auto_filter_units: Vec<crate::auto_filter_panel::Bounds>,
}

#[cfg(test)]
impl Default for EffectsUiBounds {
    fn default() -> Self {
        Self {
            library: Rect::NOTHING,
            header: Rect::NOTHING,
            strip: Rect::NOTHING,
            scrollbar: Rect::NOTHING,
            entry: Rect::NOTHING,
            drive_entry: Rect::NOTHING,
            auto_filter_entry: Rect::NOTHING,
            compact: Rect::NOTHING,
            chain: vec![],
            lane_viewport: Rect::NOTHING,
            lane_scroll: 0.0,
            units: vec![],
            drive_units: vec![],
            auto_filter_units: vec![],
        }
    }
}

impl DawUi {
    pub(super) fn effects_view(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        self.selected_effects.retain(|id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|track| track.effects.iter().any(|effect| effect.id == *id))
        });
        let selected = self
            .selected_track
            .and_then(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
            })
            .cloned();
        let mut compact = selected.as_ref().is_some_and(|track| track.effects_compact);
        let mut effects = selected
            .as_ref()
            .map(|track| track.effects.clone())
            .unwrap_or_default();
        // Knob focus must remain editable; text editors and other active gestures still block changes.
        let allowed = ui.is_enabled()
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
            && self.file_hover.is_none();
        let original_compact = compact;
        let original_effects = effects.clone();
        #[cfg(test)]
        {
            self.effects_bounds.units.clear();
            self.effects_bounds.drive_units.clear();
            self.effects_bounds.auto_filter_units.clear();
        }
        let border = Stroke::new(1.0, theme::palette(ui.ctx()).border);
        ui.painter()
            .hline(ui.max_rect().x_range(), ui.max_rect().top() + 0.5, border);
        ui.add_space(1.0);
        let bounds = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
        let below_track = (bounds.height() - RULER_HEIGHT - track_overview::STRIP_HEIGHT).max(0.0);
        let strip_height =
            panels::unit_height(compact).min((below_track - SCROLLBAR_HEIGHT).max(0.0));
        let strip = Rect::from_min_max(
            Pos2::new(bounds.left(), bounds.bottom() - strip_height),
            bounds.max,
        );
        let upper = Rect::from_min_max(bounds.min, Pos2::new(bounds.right(), strip.top()));
        let library = panels::ControlPanel::begin(ui, upper);
        let (track, assets) = self.track_overview_content(ui);
        self.prepare_track_overview(track.as_ref());
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(TRACK_WIDTH, upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(TRACK_WIDTH);
                    ui.set_min_height(upper.height());
                    let header = library.header(ui, "Effects", |ui| {
                        let response = ui
                            .add_enabled_ui(
                                allowed && self.effects_drag.is_none() && selected.is_some(),
                                |ui| {
                                    icons::button(
                                        ui,
                                        if compact {
                                            icons::Lucide::ArrowDownFromLine
                                        } else {
                                            icons::Lucide::ArrowUpFromLine
                                        },
                                        if compact {
                                            "Expand effects"
                                        } else {
                                            "Collapse effects"
                                        },
                                        true,
                                        theme::palette(ui.ctx()).text,
                                    )
                                },
                            )
                            .inner;
                        if response.clicked() {
                            compact = !compact;
                        }
                        #[cfg(test)]
                        {
                            self.effects_bounds.compact = response.rect;
                        }
                    });
                    panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
                    for (type_id, name, make) in [
                        (
                            daw_core::EQ_TYPE_ID,
                            "3-band EQ",
                            daw_core::EffectInstance::equalizer as fn() -> daw_core::EffectInstance,
                        ),
                        (
                            daw_core::DRIVE_TYPE_ID,
                            "Drive",
                            daw_core::EffectInstance::drive,
                        ),
                        (
                            daw_core::AUTO_FILTER_TYPE_ID,
                            "Auto-filter",
                            daw_core::EffectInstance::auto_filter,
                        ),
                    ] {
                        let response = ui
                            .add_enabled_ui(
                                allowed && self.effects_drag.is_none() && selected.is_some(),
                                |ui| {
                                    library
                                        .catalog_item(
                                            ui,
                                            name,
                                            self.effects_catalog_selection == Some(type_id),
                                        )
                                        .on_hover_text(format!(
                                            "Double-click to add {name} to this track."
                                        ))
                                },
                            )
                            .inner;
                        if response.clicked() {
                            self.effects_catalog_selection = Some(type_id);
                        }
                        if response.double_clicked() {
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
                    }
                    #[cfg(test)]
                    {
                        self.effects_bounds.library =
                            Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, upper.height()));
                        self.effects_bounds.header = header.response.rect;
                    }
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(upper.height());
                    let overview = Rect::from_min_size(
                        ui.max_rect().min + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
                        Vec2::new(
                            (ui.available_width() - track_overview::LABEL_WIDTH).max(0.0),
                            (upper.height() - SCROLLBAR_HEIGHT).max(0.0),
                        ),
                    );
                    self.timeline_pinch(ui, overview);
                    self.timeline_swipe(ui, overview);
                    self.track_overview_ruler(ui);
                    if let Some(track) = &track {
                        self.track_overview_strip(ui, track, &assets);
                    } else {
                        self.lane_bounds.clear();
                        self.timeline_bounds = Rect::NOTHING;
                        #[cfg(test)]
                        {
                            self.midi_editor.track_bounds = Rect::NOTHING;
                            self.midi_editor.clip_bounds.clear();
                        }
                        rows::centered(ui, track_overview::STRIP_HEIGHT, |ui| {
                            ui.add_space(track_overview::LABEL_WIDTH);
                            ui.label("Select a track to configure its effects.");
                        });
                    }
                    self.finish_clip_drag(ui);
                    let (remaining, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), ui.available_height().max(0.0)),
                        Sense::hover(),
                    );
                    let scrollbar = Rect::from_min_max(
                        Pos2::new(
                            remaining.left() + track_overview::LABEL_WIDTH,
                            strip.top() - SCROLLBAR_HEIGHT,
                        ),
                        Pos2::new(remaining.right(), strip.top()),
                    );
                    let grid = Rect::from_min_max(
                        Pos2::new(scrollbar.left(), remaining.top()),
                        scrollbar.right_top(),
                    );
                    if grid.is_positive() {
                        let painter = ui.painter_at(grid);
                        self.paint_grid(&painter, grid);
                        self.paint_track_overview_playhead(&painter, grid);
                    }
                    self.timeline_scrollbar_at(ui, scrollbar, "effects_timeline_scrollbar");
                    #[cfg(test)]
                    {
                        self.effects_bounds.scrollbar = scrollbar;
                    }
                },
            );
        });
        library.finish(ui);
        ui.painter()
            .rect_filled(strip, 0.0, theme::palette(ui.ctx()).background);
        // The status panel owns the single separator below the effects lane.
        panels::ControlPanel::row_dividers(ui, strip, true, false);
        self.effects_lane(
            ui,
            strip,
            selected.as_ref().map(|track| track.id),
            &mut effects,
            compact,
            allowed,
        );
        if let Some(track) = selected
            && (effects != original_effects || compact != original_compact)
        {
            self.edit(Edit::SetEffects {
                track_id: track.id,
                effects,
                compact,
            });
        }
        #[cfg(test)]
        {
            self.effects_bounds.strip = strip;
        }
    }

    fn effects_lane(
        &mut self,
        ui: &mut egui::Ui,
        strip: Rect,
        track: Option<Id>,
        effects: &mut Vec<daw_core::EffectInstance>,
        compact: bool,
        allowed: bool,
    ) {
        let mut remove = None;
        let mut slots = Vec::with_capacity(effects.len());
        let order: Vec<_> = effects.iter().map(|effect| effect.id).collect();
        let mut lane = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("effects-lane", track))
                .max_rect(strip)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lane.set_clip_rect(strip.intersect(ui.clip_rect()));
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
                                let unit = ui.push_id(effect.id, |ui| {
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
                                        })
                                        .inner;
                                    if allowed && interactive {
                                        self.start_effect_drag(
                                            ui, &header, power, track, effect.id, &order,
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
}
