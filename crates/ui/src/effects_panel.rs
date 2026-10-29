use super::*;

pub(super) use daw_core::EffectChainTarget as EffectTarget;

pub(super) struct UnitLane<'a> {
    pub compact: bool,
    pub height: f32,
    pub target: Option<EffectTarget>,
    pub allowed: bool,
    pub sources: &'a [crate::modulation_panel::Source],
}

struct LaneInteraction {
    allowed: bool,
    request_focus: bool,
}

#[cfg(test)]
pub(crate) struct EffectsUiBounds {
    pub library: Rect,
    pub toolbar: Rect,
    pub library_toggle: Rect,
    pub clip_settings_toggle: Rect,
    pub clip_settings_toolbar: Rect,
    pub clip_settings: Rect,
    pub clip_name: Rect,
    pub clip_source: Rect,
    pub clip_gain: Rect,
    pub clip_transpose: Rect,
    pub lane_toggle: Rect,
    pub header: Rect,
    pub folders: [Rect; effects_catalog::CATEGORY_COUNT],
    pub strip: Rect,
    pub scrollbar: Rect,
    pub entry: Rect,
    pub drive_entry: Rect,
    pub drive_v2_entry: Rect,
    pub auto_filter_entry: Rect,
    pub filter_entry: Rect,
    pub compressor_entry: Rect,
    pub ott_entry: Rect,
    pub limiter_entry: Rect,
    pub direction_mixer_entry: Rect,
    pub stereo_spread_entry: Rect,
    pub saturator_entry: Rect,
    pub bitcrusher_entry: Rect,
    pub chorus_entry: Rect,
    pub lane_toolbar: Rect,
    pub lane_compact: Rect,
    pub chain: Vec<(Id, Rect)>,
    pub lane_viewport: Rect,
    pub lane_scroll: f32,
    pub units: Vec<crate::equalizer_panel::Bounds>,
    pub multiband_eq_units: Vec<crate::multiband_eq_panel::Bounds>,
    pub drive_units: Vec<crate::drive_panel::Bounds>,
    pub drive_v2_units: Vec<crate::drive_v2_panel::Bounds>,
    pub auto_filter_units: Vec<crate::auto_filter_panel::Bounds>,
    pub filter_units: Vec<crate::auto_filter_panel::Bounds>,
    pub compressor_units: Vec<crate::compressor_panel::Bounds>,
    pub ott_units: Vec<crate::ott_panel::Bounds>,
    pub limiter_units: Vec<crate::limiter_panel::Bounds>,
    pub direction_mixer_units: Vec<crate::direction_mixer_panel::Bounds>,
    pub stereo_spread_units: Vec<crate::stereo_spread_panel::Bounds>,
    pub saturator_units: Vec<crate::saturator_panel::Bounds>,
    pub bitcrusher_units: Vec<crate::bitcrusher_panel::Bounds>,
    pub chorus_units: Vec<crate::chorus_panel::Bounds>,
}

#[cfg(test)]
impl Default for EffectsUiBounds {
    fn default() -> Self {
        Self {
            library: Rect::NOTHING,
            toolbar: Rect::NOTHING,
            library_toggle: Rect::NOTHING,
            clip_settings_toggle: Rect::NOTHING,
            clip_settings_toolbar: Rect::NOTHING,
            clip_settings: Rect::NOTHING,
            clip_name: Rect::NOTHING,
            clip_source: Rect::NOTHING,
            clip_gain: Rect::NOTHING,
            clip_transpose: Rect::NOTHING,
            lane_toggle: Rect::NOTHING,
            header: Rect::NOTHING,
            folders: [Rect::NOTHING; effects_catalog::CATEGORY_COUNT],
            strip: Rect::NOTHING,
            scrollbar: Rect::NOTHING,
            entry: Rect::NOTHING,
            drive_entry: Rect::NOTHING,
            drive_v2_entry: Rect::NOTHING,
            auto_filter_entry: Rect::NOTHING,
            filter_entry: Rect::NOTHING,
            compressor_entry: Rect::NOTHING,
            ott_entry: Rect::NOTHING,
            limiter_entry: Rect::NOTHING,
            direction_mixer_entry: Rect::NOTHING,
            stereo_spread_entry: Rect::NOTHING,
            saturator_entry: Rect::NOTHING,
            bitcrusher_entry: Rect::NOTHING,
            chorus_entry: Rect::NOTHING,
            lane_toolbar: Rect::NOTHING,
            lane_compact: Rect::NOTHING,
            chain: vec![],
            lane_viewport: Rect::NOTHING,
            lane_scroll: 0.0,
            units: vec![],
            multiband_eq_units: vec![],
            drive_units: vec![],
            drive_v2_units: vec![],
            auto_filter_units: vec![],
            filter_units: vec![],
            compressor_units: vec![],
            ott_units: vec![],
            limiter_units: vec![],
            direction_mixer_units: vec![],
            stereo_spread_units: vec![],
            saturator_units: vec![],
            bitcrusher_units: vec![],
            chorus_units: vec![],
        }
    }
}

#[derive(Default)]
pub(super) struct Visibility {
    pub catalog: bool,
    pub lane: bool,
    pub chords: bool,
}

impl DawUi {
    pub(super) fn effects_catalog_visible(&self) -> bool {
        match self.view_mode {
            ViewMode::Edit => self.editor_effects.catalog,
            ViewMode::Mixer => self.mixer_effects.catalog,
            _ => self.timeline_catalog_visible,
        }
    }

    pub(super) fn set_effects_catalog_visible(&mut self, visible: bool) {
        match self.view_mode {
            ViewMode::Edit => self.editor_effects.catalog = visible,
            ViewMode::Mixer => self.mixer_effects.catalog = visible,
            _ => self.timeline_catalog_visible = visible,
        }
    }

    pub(super) fn effects_lane_visible(&self) -> bool {
        match self.view_mode {
            ViewMode::Edit => self.editor_effects.lane,
            ViewMode::Mixer => self.mixer_effects.lane,
            _ => self.timeline_effects_visible,
        }
    }

    pub(super) fn set_effects_lane_visible(&mut self, visible: bool) {
        match self.view_mode {
            ViewMode::Edit => self.editor_effects.lane = visible,
            ViewMode::Mixer => self.mixer_effects.lane = visible,
            _ => self.timeline_effects_visible = visible,
        }
    }

    pub(super) fn chords_visible(&self) -> bool {
        match self.view_mode {
            ViewMode::Edit => self.editor_effects.chords,
            ViewMode::Timeline => self.chords.visible,
            _ => false,
        }
    }

    pub(super) fn set_chords_visible(&mut self, visible: bool) {
        if self.view_mode == ViewMode::Edit {
            self.editor_effects.chords = visible;
        } else {
            self.chords.visible = visible;
        }
    }

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
        if self.view_mode == ViewMode::Edit {
            return self
                .editor_track()
                .map(|track| EffectTarget::Track(track.id));
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
        self.session.project.effect_chain(target)
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
            && self.chords.drag.as_ref().is_none_or(|drag| !drag.active)
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
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        let mut timeline_drop = None;
        let mut transferred_selection = None;
        let mut original_compact = compact;
        let mut original_effects = effects.clone();
        #[cfg(test)]
        {
            self.effects_bounds.units.clear();
            self.effects_bounds.multiband_eq_units.clear();
            self.effects_bounds.drive_units.clear();
            self.effects_bounds.drive_v2_units.clear();
            self.effects_bounds.auto_filter_units.clear();
            self.effects_bounds.filter_units.clear();
            self.effects_bounds.compressor_units.clear();
            self.effects_bounds.ott_units.clear();
            self.effects_bounds.limiter_units.clear();
            self.effects_bounds.direction_mixer_units.clear();
            self.effects_bounds.stereo_spread_units.clear();
            self.effects_bounds.saturator_units.clear();
            self.effects_bounds.bitcrusher_units.clear();
            self.effects_bounds.chorus_units.clear();
        }
        // Keep child fills below the workspace's shared top separator.
        ui.add_space(1.0);
        let bounds = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
        let minimum_content = match self.view_mode {
            ViewMode::Instrument => toolbars::HEIGHT,
            ViewMode::Timeline => RULER_HEIGHT + self.track_row_height() + SCROLLBAR_HEIGHT,
            ViewMode::Edit => midi_editor::MIN_HEIGHT - ROW_HEIGHT + self.track_row_height(),
            ViewMode::Mixer => mixer::MIN_HEIGHT,
        };
        let below_track = (bounds.height() - minimum_content).max(0.0);
        let lane_visible = self.effects_lane_visible();
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
        let clip_settings_visible =
            self.view_mode == ViewMode::Timeline && self.clip_settings_visible;
        let components_visible = self.view_mode == ViewMode::Instrument
            && self.instrument_catalog_visible
            && !clip_settings_visible;
        let chords_visible = self.chords_visible();
        let catalog_visible =
            self.effects_catalog_visible() || components_visible || chords_visible;
        if !clip_settings_visible && self.view_mode != ViewMode::Edit {
            self.clear_clip_settings_edits();
        }
        let library = catalog_visible.then(|| panels::ControlPanel::begin(ui, upper));
        #[cfg(test)]
        {
            self.effects_bounds.folders = [Rect::NOTHING; effects_catalog::CATEGORY_COUNT];
            self.effects_bounds.library = Rect::NOTHING;
            self.effects_bounds.clip_settings = Rect::NOTHING;
            self.effects_bounds.clip_name = Rect::NOTHING;
            self.effects_bounds.clip_source = Rect::NOTHING;
            self.effects_bounds.clip_gain = Rect::NOTHING;
            self.effects_bounds.clip_transpose = Rect::NOTHING;
            self.effects_bounds.header = Rect::NOTHING;
            self.effects_bounds.entry = Rect::NOTHING;
            self.effects_bounds.drive_entry = Rect::NOTHING;
            self.effects_bounds.auto_filter_entry = Rect::NOTHING;
            self.effects_bounds.compressor_entry = Rect::NOTHING;
            self.effects_bounds.ott_entry = Rect::NOTHING;
            self.effects_bounds.limiter_entry = Rect::NOTHING;
            self.effects_bounds.direction_mixer_entry = Rect::NOTHING;
            self.effects_bounds.stereo_spread_entry = Rect::NOTHING;
            self.effects_bounds.saturator_entry = Rect::NOTHING;
            self.effects_bounds.bitcrusher_entry = Rect::NOTHING;
            self.effects_bounds.chorus_entry = Rect::NOTHING;
        }
        ui.horizontal_top(|ui| {
            if let Some(library) = library.as_ref() {
                ui.allocate_ui_with_layout(
                    Vec2::new(TRACK_WIDTH, upper.height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(TRACK_WIDTH);
                        ui.set_min_height(upper.height());
                        let catalog_rect =
                            Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, upper.height()));
                        if chords_visible {
                            self.chords_catalog_panel(ui, library, catalog_rect);
                        } else if components_visible {
                            self.instrument_catalog_panel(
                                ui,
                                library,
                                catalog_rect,
                                self.effects_editing_enabled(ui),
                            );
                        } else {
                            let output = self.effects_catalog.show(
                                ui,
                                library,
                                catalog_rect,
                                "Effects",
                                "effects",
                                catalog::Interaction {
                                    rows_enabled: allowed
                                        && self
                                            .effects_drag
                                            .as_ref()
                                            .is_none_or(|drag| drag.is_library() && !drag.active)
                                        && selected.is_some(),
                                    keyboard_enabled: allowed
                                        && self.effects_drag.is_none()
                                        && selected.is_some()
                                        && !ui.input(|input| input.pointer.primary_down()),
                                    clear_on_empty: allowed && self.effects_drag.is_none(),
                                    empty_enabled: allowed,
                                },
                            );
                            if output.add_selected {
                                self.add_catalog_effects(
                                    &mut effects,
                                    effects_catalog::instantiate(
                                        &self.effects_catalog.selected_types(),
                                    ),
                                );
                            }
                            if let Some(index) = output.double_clicked {
                                self.add_catalog_effects(
                                    &mut effects,
                                    vec![(effects_catalog::ENTRIES[index].make)()],
                                );
                            }
                            for &(index, ref response) in &output.rows {
                                if allowed
                                    && (lane_visible || self.view_mode == ViewMode::Instrument)
                                {
                                    let target = self.effect_catalog_target();
                                    let chain = target
                                        .and_then(|t| self.effect_chain(t))
                                        .map(|(e, _)| e.to_vec())
                                        .unwrap_or_default();
                                    self.start_library_effect_drag(
                                        ui, response, target, index, &chain,
                                    );
                                }
                                #[cfg(test)]
                                match effects_catalog::ENTRIES[index].type_id {
                                    daw_core::EQ_TYPE_ID => {
                                        self.effects_bounds.entry = response.rect
                                    }
                                    daw_core::DRIVE_V2_TYPE_ID => {
                                        self.effects_bounds.drive_v2_entry = response.rect;
                                    }
                                    daw_core::DRIVE_TYPE_ID => {
                                        self.effects_bounds.drive_entry = response.rect
                                    }
                                    daw_core::FILTER_TYPE_ID => {
                                        self.effects_bounds.filter_entry = response.rect
                                    }
                                    daw_core::AUTO_FILTER_TYPE_ID => {
                                        self.effects_bounds.auto_filter_entry = response.rect
                                    }
                                    daw_core::COMPRESSOR_TYPE_ID => {
                                        self.effects_bounds.compressor_entry = response.rect
                                    }
                                    daw_core::OTT_TYPE_ID => {
                                        self.effects_bounds.ott_entry = response.rect
                                    }
                                    daw_core::STEREO_SPREAD_TYPE_ID => {
                                        self.effects_bounds.stereo_spread_entry = response.rect;
                                    }
                                    daw_core::SATURATOR_TYPE_ID => {
                                        self.effects_bounds.saturator_entry = response.rect;
                                    }
                                    daw_core::BITCRUSHER_TYPE_ID => {
                                        self.effects_bounds.bitcrusher_entry = response.rect;
                                    }
                                    daw_core::CHORUS_TYPE_ID => {
                                        self.effects_bounds.chorus_entry = response.rect;
                                    }
                                    daw_core::DIRECTION_MIXER_TYPE_ID => {
                                        self.effects_bounds.direction_mixer_entry = response.rect;
                                    }
                                    daw_core::LIMITER_TYPE_ID => {
                                        self.effects_bounds.limiter_entry = response.rect
                                    }
                                    _ => {}
                                }
                            }
                            #[cfg(test)]
                            {
                                self.effects_bounds.library = catalog_rect;
                                self.effects_bounds.header = output.header;
                                for (index, response) in &output.folders {
                                    self.effects_bounds.folders[*index] = response.rect;
                                }
                            }
                            #[cfg(not(test))]
                            {
                                let _ = output.header;
                            }
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
                    let mut content_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("effects-workspace-content", self.view_mode.label()))
                            .max_rect(content_bounds)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    content_ui.set_clip_rect(content_bounds.intersect(ui.clip_rect()));
                    content_ui.add_enabled_ui(
                        (self.effects_drag.is_none() && self.chords.drag.is_none())
                            || self.view_mode == ViewMode::Instrument,
                        |ui| match self.view_mode {
                            ViewMode::Instrument => {
                                self.timeline_bounds = Rect::NOTHING;
                                self.lane_bounds.clear();
                                self.file_drop_target = None;
                                self.instrument_view(ui);
                            }
                            ViewMode::Timeline => timeline_drop = self.timeline_workspace(ui),
                            ViewMode::Edit => self.edit_view(ui),
                            ViewMode::Mixer => self.mixer(ui),
                        },
                    );
                    #[cfg(test)]
                    {
                        self.effects_bounds.scrollbar = if self.view_mode == ViewMode::Timeline {
                            self.scrollbar_bounds
                        } else {
                            Rect::NOTHING
                        };
                    }
                },
            );
        });
        self.finish_chord_drag(ui);
        if let Some(library) = library {
            library.finish(ui);
        }
        if let Some((destination, transfer)) =
            self.finish_timeline_effect_drag(ui, timeline_drop, allowed)
        {
            self.edit(transfer.edit);
            if self.error.is_none() {
                match destination {
                    EffectTarget::Master => self.select_master(),
                    EffectTarget::Track(track) | EffectTarget::Layer { track, .. } => {
                        self.select_track(track, false);
                    }
                }
                transferred_selection = Some(transfer.selected);
            }
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
        let focus_transferred = transferred_selection.is_some();
        if let Some(ids) = transferred_selection {
            self.selected_effects = ids.into_iter().collect();
        }
        // An inline transfer can remove track effects while the instrument is drawn.
        // Refresh that source snapshot before painting or committing the bottom chain.
        if let Some(target) = selected
            && let Some((saved, saved_compact)) = self.effect_chain(target)
            && (saved != original_effects || saved_compact != original_compact)
        {
            effects = saved.to_vec();
            compact = saved_compact;
            original_effects = effects.clone();
            original_compact = compact;
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
            if let Some(transfer) = self.effects_lane(
                ui,
                strip,
                selected,
                &mut effects,
                compact,
                LaneInteraction {
                    allowed,
                    request_focus: focus_transferred,
                },
            ) {
                self.edit(transfer.edit);
                if self.error.is_none() {
                    self.selected_effects = transfer.selected.into_iter().collect();
                    self.instrument_ui.effects_layer = None;
                    self.instrument_ui.track_effects_target = true;
                    (compact, effects) = self.effects_state(selected);
                    original_compact = compact;
                    original_effects = effects.clone();
                }
            }
        }
        if self
            .effects_drag
            .as_ref()
            .is_some_and(|d| matches!(d.target, crate::effects_panel::EffectTarget::Layer { .. }))
            && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        {
            self.effects_drag = None;
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
                EffectTarget::Layer { track, layer } => Edit::SetLayerEffects {
                    track_id: track,
                    layer_id: layer,
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
        interaction: LaneInteraction,
    ) -> Option<daw_core::EffectTransfer> {
        let LaneInteraction {
            allowed,
            request_focus,
        } = interaction;
        let modulation_sources = crate::modulation_panel::sources(target.and_then(|target| {
            match target {
                EffectTarget::Track(id) | EffectTarget::Layer { track: id, .. } => self
                    .session
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.instrument.as_ref()),
                EffectTarget::Master => None,
            }
        }))
        .into_iter()
        .filter(|s| daw_unit_core::ModulationScope::Shared.accepts(s.scope))
        .collect::<Vec<_>>();
        let mut remove = None;
        let mut slots = Vec::with_capacity(effects.len());
        let content = Rect::from_min_max(strip.min, effects_toolbar::bounds(strip).left_bottom());
        let mut lane = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("effects-lane", target))
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lane.set_clip_rect(content.intersect(ui.clip_rect()));
        let keyboard_id = lane.id().with("effect-lane-keyboard-focus");
        let keyboard_focus = crate::unit_selection::keyboard_focus(&mut lane, strip, keyboard_id);
        if request_focus {
            keyboard_focus.request_focus();
        }
        if crate::unit_selection::clicked(&lane, strip, allowed) {
            self.instrument_ui.effects_layer = None;
            self.instrument_ui.track_effects_target = true;
        }
        let focused = self.view_mode != ViewMode::Instrument
            || self.instrument_ui.effects_layer.is_none()
            || crate::unit_selection::clicked(&lane, strip, allowed);
        if focused {
            self.effects_lane_focus.set(&lane, strip, true);
        }
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
                        crate::strip_reorder::scroll(ui);
                        let (painted, removed) = self.effect_units(
                            ui,
                            effects,
                            UnitLane {
                                compact,
                                height: strip.height(),
                                target,
                                allowed,
                                sources: &modulation_sources,
                            },
                        );
                        slots = painted;
                        remove = removed;
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
            keyboard_focus.request_focus();
        }
        if focused {
            self.effect_selection_shortcuts(&lane, effects, allowed, &keyboard_focus);
        }
        if let Some(target) = target
            && lane.input(|i| i.pointer.latest_pos().is_some_and(|p| strip.contains(p)))
        {
            self.retarget_library_effect_drag(target, effects);
        }
        if let Some(target) = target
            && let Some(transfer) = self.finish_cross_effect_drag(
                &lane,
                target,
                &slots,
                scrolled.inner_rect.intersect(strip),
                allowed,
            )
        {
            self.effects_lane_focus.set(&lane, strip, true);
            keyboard_focus.request_focus();
            return Some(transfer);
        }
        if self
            .effects_drag
            .as_ref()
            .is_none_or(|d| !matches!(d.target, EffectTarget::Layer { .. }))
        {
            self.finish_effect_drag(
                &lane,
                effects,
                &slots,
                scrolled.inner_rect.intersect(strip),
                allowed,
            );
        }

        if let Some(id) = remove {
            effects.retain(|effect| effect.id != id);
        }
        None
    }

    pub(super) fn effect_units(
        &mut self,
        ui: &mut egui::Ui,
        effects: &mut [daw_core::EffectInstance],
        controls: UnitLane<'_>,
    ) -> (Vec<(Id, Rect)>, Option<Id>) {
        let UnitLane {
            compact,
            height,
            target,
            allowed,
            sources: modulation_sources,
        } = controls;
        let mut slots = Vec::with_capacity(effects.len());
        let mut remove = None;
        let order: Vec<_> = if let Some(target @ EffectTarget::Layer { .. }) = target {
            self.effect_chain(target).map_or_else(
                || effects.iter().map(|e| e.id).collect(),
                |(effects, _)| effects.iter().map(|e| e.id).collect(),
            )
        } else {
            effects.iter().map(|e| e.id).collect()
        };
        let mut power_actions = Vec::new();
        let mut power_states = if let Some(target @ EffectTarget::Layer { .. }) = target {
            self.effect_chain(target)
                .map_or_else(Vec::new, |(chain, _)| chain.to_vec())
        } else {
            effects.to_vec()
        };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = -1.0;
            for effect in effects.iter_mut() {
                let selected_here = match target {
                    Some(EffectTarget::Layer { layer, .. }) => {
                        self.instrument_ui.effects_layer == Some(layer)
                    }
                    _ => self.instrument_ui.effects_layer.is_none(),
                };
                let targets = if selected_here {
                    crate::unit_selection::drag_group(&order, &self.selected_effects, effect.id)
                } else {
                    vec![effect.id]
                };
                let supported = crate::unit_power::effect_enabled(&mut effect.unit).is_some();
                let menu = crate::unit_power::PowerMenu::new(
                    power_states
                        .iter_mut()
                        .filter(|e| targets.contains(&e.id))
                        .filter_map(|e| crate::unit_power::effect_enabled(&mut e.unit).copied()),
                );

                let mut unit_scope = egui::UiBuilder::new().id_salt(effect.id);
                if slots.is_empty() {
                    unit_scope = unit_scope.ui_stack_info(
                        egui::UiStackInfo::default().with_tag(panels::SHARED_LEFT_BORDER_TAG),
                    );
                }
                let unit = ui.scope_builder(unit_scope, |ui| {
                    let interactive = self.effects_drag.as_ref().is_none_or(|drag| !drag.active);
                    let (header, power) = ui
                        .add_enabled_ui(interactive, |ui| {
                            let output_level = meters::OutputLevel::new(
                                self.output
                                    .as_ref()
                                    .map_or(0.0, |o| o.effect_output_level(effect.id)),
                                self.output
                                    .as_ref()
                                    .and_then(|o| o.effect_output_peak(effect.id)),
                            );
                            match &mut effect.unit {
                                daw_core::EffectSettings::MultibandEq(settings) => {
                                    let (bounds, header) = crate::multiband_eq_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        |band| {
                                            self.output.as_ref().and_then(|o| {
                                                o.modulation_snapshot(match band {
                                                    Some(band) => {
                                                        daw_engine::ModulationTarget::EffectBand {
                                                            effect: effect.id,
                                                            band,
                                                        }
                                                    }
                                                    None => daw_engine::ModulationTarget::Effect(
                                                        effect.id,
                                                    ),
                                                })
                                            })
                                        },
                                        |band| {
                                            self.output.as_ref().map_or(0.0, |o| {
                                                o.eq_individual_band_level(effect.id, band)
                                            })
                                        },
                                        output_level,
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.multiband_eq_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Eq3(settings) => {
                                    let (_bounds, header) = crate::equalizer_panel::show(
                                        ui,
                                        settings,
                                        compact,
                                        height,
                                        self.output.as_ref().map_or([0.0; 3], |output| {
                                            output.eq_band_levels(effect.id)
                                        }),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::DriveV2(settings) => {
                                    let (bounds, header) = crate::drive_v2_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.drive_v2_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Drive(settings) => {
                                    let (_bounds, header) = crate::drive_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_values(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.drive_units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::AutoFilter(settings) => {
                                    let (_bounds, header) = crate::auto_filter_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        self.session.project.sample_rate_hz,
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.auto_filter_units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Filter(settings) => {
                                    let (_bounds, header) = crate::auto_filter_panel::show_static(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_values(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        self.session.project.sample_rate_hz,
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.filter_units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Ott(settings) => {
                                    let gains = self.output.as_ref().map_or([0.0; 3], |output| {
                                        output.ott_gain_change_db(effect.id)
                                    });
                                    let gains = meters::BandGainChange::new(
                                        gains,
                                        self.output.as_ref().and_then(|output| {
                                            output.ott_gain_change_meter(effect.id)
                                        }),
                                    );
                                    let (_bounds, header) = crate::ott_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        gains,
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.ott_units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Limiter(settings) => {
                                    let reduction = self
                                        .output
                                        .as_ref()
                                        .map_or(0.0, |output| output.gain_reduction_db(effect.id));
                                    let reduction = meters::GainReduction::new(
                                        reduction,
                                        self.output.as_ref().and_then(|output| {
                                            output.gain_reduction_meter(effect.id)
                                        }),
                                    );
                                    let (_bounds, header) = crate::limiter_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        reduction,
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.limiter_units.push(_bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::StereoSpread(settings) => {
                                    let (bounds, header) = crate::stereo_spread_panel::show(
                                        ui,
                                        settings,
                                        compact,
                                        height,
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.stereo_spread_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Saturator(settings) => {
                                    let (bounds, header) = crate::saturator_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.saturator_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Bitcrusher(settings) => {
                                    let (bounds, header) = crate::bitcrusher_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.bitcrusher_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Chorus(settings) => {
                                    let (bounds, header) = crate::chorus_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                        self.session.project.tempo_bpm,
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.chorus_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::DirectionMixer(settings) => {
                                    let (bounds, header) = crate::direction_mixer_panel::show(
                                        ui,
                                        settings,
                                        compact,
                                        height,
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                    );
                                    let power = bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.direction_mixer_units.push(bounds);
                                    (header, power)
                                }
                                daw_core::EffectSettings::Missing { type_id, .. } => {
                                    let response = ui.add(
                                        egui::Label::new(format!(
                                            "Missing effect: {type_id} (bypassed)"
                                        ))
                                        .sense(Sense::click_and_drag()),
                                    );
                                    (response, Rect::NOTHING)
                                }
                                daw_core::EffectSettings::Compressor(settings) => {
                                    let reduction = self
                                        .output
                                        .as_ref()
                                        .map_or(0.0, |output| output.gain_reduction_db(effect.id));
                                    let reduction = meters::GainReduction::new(
                                        reduction,
                                        self.output.as_ref().and_then(|output| {
                                            output.gain_reduction_meter(effect.id)
                                        }),
                                    );
                                    let (_bounds, header) = crate::compressor_panel::show(
                                        ui,
                                        settings,
                                        (compact, height),
                                        reduction,
                                        modulation_sources,
                                        self.output.as_ref().and_then(|o| {
                                            o.modulation_snapshot(
                                                daw_engine::ModulationTarget::Effect(effect.id),
                                            )
                                        }),
                                        output_level,
                                    );
                                    let power = _bounds.power;
                                    #[cfg(test)]
                                    self.effects_bounds.compressor_units.push(_bounds);
                                    (header, power)
                                }
                            }
                        })
                        .inner;
                    if allowed && interactive {
                        self.start_effect_drag(ui, &header, power, target, effect.id, &order);
                        crate::menus::ContextMenu::new(
                            &header.clone().on_hover_cursor(egui::CursorIcon::Grab),
                        )
                        .show(|ui| {
                            ui.add_enabled_ui(self.effects_drag.is_none() && supported, |ui| {
                                menu.show(ui)
                            });
                            if menu.action().is_some() {
                                crate::unit_power::schedule_focus(ui, &header);
                            }
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
                if crate::unit_selection::clicked(
                    ui,
                    rect,
                    allowed && self.effects_drag.as_ref().is_none_or(|drag| !drag.active),
                ) {
                    let layer = match target {
                        Some(EffectTarget::Layer { layer, .. }) => Some(layer),
                        _ => None,
                    };
                    if self.instrument_ui.effects_layer != layer {
                        self.selected_effects.clear();
                    }
                    self.instrument_ui.effects_layer = layer;
                    self.select_effect(effect.id, ui.input(|input| input.modifiers.shift));
                    crate::unit_selection::retain_focus(
                        ui,
                        &unit.response,
                        &self.effects_lane_focus,
                    );
                }
                if let Some(enable) = menu.action() {
                    power_actions.push((targets, enable));
                }
                slots.push((effect.id, rect));
            }
        });
        for (targets, enable) in power_actions {
            if let Some(EffectTarget::Layer { layer, .. }) = target {
                self.instrument_ui
                    .inline_power_actions
                    .push((layer, targets, enable));
            } else {
                for effect in effects.iter_mut().filter(|e| targets.contains(&e.id)) {
                    if let Some(state) = crate::unit_power::effect_enabled(&mut effect.unit) {
                        *state = enable;
                    }
                }
            }
        }
        // Paint last so adjacent units cannot cover a selected outline.
        for (id, rect) in &slots {
            crate::unit_selection::paint(ui, *rect, self.selected_effects.contains(id));
        }
        (slots, remove)
    }

    pub(super) fn effect_selection_shortcuts(
        &mut self,
        ui: &egui::Ui,
        effects: &mut Vec<daw_core::EffectInstance>,
        allowed: bool,
        keyboard_focus: &egui::Response,
    ) {
        let actions = crate::unit_selection::shortcuts(
            ui,
            &self.effects_lane_focus,
            allowed,
            self.effects_drag.is_some()
                || self.strip_dragging
                || crate::strip_reorder::dragging(ui.ctx()),
        );
        if actions.select_all {
            crate::unit_selection::select_all(
                &mut self.selected_effects,
                effects.iter().map(|e| e.id),
            );
            ui.ctx().request_repaint();
        }
        if actions.delete && !self.selected_effects.is_empty() {
            effects.retain(|effect| !self.selected_effects.contains(&effect.id));
            self.selected_effects.clear();
            // Keep focus on the pane after its focused unit disappears. A held Delete
            // must not fall through to the selected clip in the track overview.
            keyboard_focus.request_focus();
            ui.ctx().request_repaint();
        }
    }
}
