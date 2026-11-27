use super::*;
use daw_core::InstrumentComponent;

pub(super) const DUPLICATE_LAYER_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::D);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum InstrumentSubview {
    All,
    #[default]
    Layer,
    Sources,
    Effects,
    #[cfg(test)]
    Modulation,
    Lfos,
    Envelopes,
    Macros,
}

impl InstrumentSubview {
    const PROVIDERS: [Self; 3] = [Self::Lfos, Self::Envelopes, Self::Macros];

    pub(super) fn uses_track_column(self) -> bool {
        matches!(
            self,
            Self::All | Self::Layer | Self::Lfos | Self::Envelopes | Self::Macros
        )
    }

    pub(super) fn is_modulation(self) -> bool {
        matches!(self, Self::Lfos | Self::Envelopes | Self::Macros) || self.combined_modulation()
    }

    pub(super) fn combined_modulation(self) -> bool {
        #[cfg(test)]
        if self == Self::Modulation {
            return true;
        }
        false
    }

    pub(super) fn shows(self, component: InstrumentComponent) -> bool {
        use InstrumentComponent::*;
        match self {
            Self::All => false,
            Self::Layer | Self::Sources => component.is_source() || component == Amplitude,
            Self::Effects => matches!(component, Effect(_)),
            #[cfg(test)]
            Self::Modulation => matches!(component, Envelope(_) | Lfo(_) | Macros),
            Self::Lfos => matches!(component, Lfo(_)),
            Self::Envelopes => matches!(component, Envelope(_)),
            Self::Macros => component == Macros,
        }
    }
}

impl DawUi {
    pub(super) fn visible_instrument_layer(
        &self,
        settings: &daw_core::InstrumentSettings,
    ) -> Option<Id> {
        self.instrument_ui
            .layer_page
            .filter(|id| settings.layers.iter().any(|layer| layer.id == *id))
            .or_else(|| settings.layers.first().map(|layer| layer.id))
    }

    fn instrument_subview_tabs(
        &mut self,
        ui: &mut egui::Ui,
        modulation_width: f32,
        add_width: f32,
    ) {
        let settings = self
            .selected_instrument_owner()
            .and_then(|owner| self.session.project.instrument_settings(owner));
        let has_instrument = settings.is_some();
        let current = settings.and_then(|settings| self.visible_instrument_layer(settings));
        let layers: Vec<_> = settings
            .into_iter()
            .flat_map(|settings| {
                settings
                    .layers
                    .iter()
                    .map(|layer| (layer.id, layer.name.clone()))
            })
            .collect();
        let enabled = has_instrument
            && (self.view_panel_enabled(ViewPanel::TrackMixer, ui.ctx())
                || (self.instrument_ui.has_layer_gesture()
                    && self.effects_editing_available(ui.ctx())))
            && !self.instrument_ui.editing_name()
            && self.effects_drag.is_none()
            && self.component_drag.is_none();
        let show_add = has_instrument && self.instrument_subview.uses_track_column();
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.all_tab = Rect::NOTHING;
            self.instrument_catalog_bounds.layer_tabs.clear();
            self.instrument_catalog_bounds.layer_add_tab = Rect::NOTHING;
        }
        let bounds = ui.max_rect();
        ui.spacing_mut().item_spacing.y = 0.0;
        toolbars::Toolbar::row(ui, |ui| {
            if has_instrument {
                let layers_width =
                    ui.available_width() - modulation_width - ui.spacing().item_spacing.x;
                if layers_width > 0.0 {
                    self.instrument_layer_group(
                        ui,
                        &layers,
                        current,
                        enabled,
                        layers_width,
                        if show_add { add_width } else { 0.0 },
                    );
                }
            }
            let group = Self::modulation_tabs(
                ui,
                enabled,
                has_instrument.then_some(self.instrument_subview),
            );
            for (index, response) in group.inner.into_iter().enumerate() {
                #[cfg(test)]
                {
                    self.instrument_catalog_bounds.subviews[index] = response.rect;
                }
                if response.clicked() {
                    self.set_instrument_subview(InstrumentSubview::PROVIDERS[index], ui.ctx());
                }
            }
        });
        if !has_instrument && self.instrument_subview.uses_track_column() {
            panels::ControlPanel::row_dividers(ui, bounds, false, true);
        }
    }

    fn fixed_instrument_tab_widths(ui: &mut egui::Ui) -> (f32, f32) {
        // Measure the same buttons so font and display settings also apply to the reserved width.
        let mut measure = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("measure-fixed-instrument-tabs")
                .invisible()
                .sizing_pass(),
        );
        let modulation = Self::modulation_tabs(&mut measure, false, None);
        let add = rows::input_group(&mut measure, |ui| Self::layer_add_tab(ui, false));
        (modulation.response.rect.width(), add.response.rect.width())
    }

    fn modulation_tabs(
        ui: &mut egui::Ui,
        enabled: bool,
        active: Option<InstrumentSubview>,
    ) -> egui::InnerResponse<[egui::Response; 3]> {
        let labels = ["LFOs", "Envelopes", "Macros"];
        let group = rows::input_group(ui, |ui| {
            std::array::from_fn(|index| {
                Self::instrument_tab(
                    ui,
                    labels[index],
                    enabled,
                    active == Some(InstrumentSubview::PROVIDERS[index]),
                    index,
                    labels.len(),
                )
            })
        });
        for response in &group.inner[1..] {
            rows::group_separator(ui, group.response.rect, response.rect.left());
        }
        group
    }

    fn instrument_layer_group(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[(Id, String)],
        current: Option<Id>,
        enabled: bool,
        width: f32,
        add_width: f32,
    ) {
        let show_add = add_width > 0.0;
        let group = rows::input_group(ui, |ui| {
            let tabs_width = if show_add {
                width - add_width - ui.spacing().item_spacing.x
            } else {
                width
            };
            if tabs_width > 0.0 {
                egui::ScrollArea::horizontal()
                    .id_salt("instrument-tab-scroll")
                    .max_width(tabs_width)
                    .min_scrolled_width(0.0)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        self.instrument_layer_tabs(ui, layers, current, enabled, show_add);
                    });
            }
            show_add.then(|| {
                let plus =
                    Self::layer_add_tab(ui, enabled && !self.instrument_ui.has_layer_gesture());
                #[cfg(test)]
                {
                    self.instrument_catalog_bounds.layer_add_tab = plus.rect;
                }
                if plus.clicked()
                    && let Some(track_id) = self.selected_instrument_owner()
                    && let Some(mut settings) = self.session.project.instrument_snapshot(track_id)
                {
                    self.set_instrument_subview(InstrumentSubview::Layer, ui.ctx());
                    self.append_instrument_layer(&mut settings);
                    self.edit(Edit::SetInstrument {
                        track_id,
                        instrument: Some(settings),
                    });
                }
                plus
            })
        });
        if let Some(plus) = group.inner {
            rows::group_separator(ui, group.response.rect, plus.rect.left());
        }
    }

    fn layer_add_tab(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
        let plus = ui
            .scope(|ui| {
                rows::input_group_edge(ui, true);
                let icon = icons::Lucide::Plus.size(icons::TOOLBAR_ICON_SIZE).image();
                ui.add_enabled(
                    enabled,
                    egui::Button::image(icon)
                        .image_tint_follows_text_color(true)
                        .min_size(Vec2::splat(toolbars::CONTROL_HEIGHT)),
                )
                .on_hover_text("Add layer")
            })
            .inner;
        plus.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, plus.enabled(), "Add layer")
        });
        plus
    }

    fn instrument_layer_tabs(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[(Id, String)],
        current: Option<Id>,
        enabled: bool,
        show_add: bool,
    ) {
        let group = ui.horizontal(|ui| {
            let count = 1 + layers.len() + usize::from(show_add);
            let all = ui
                .push_id("all-layers", |ui| {
                    Self::instrument_tab(
                        ui,
                        "All",
                        enabled,
                        self.instrument_subview == InstrumentSubview::All,
                        0,
                        count,
                    )
                })
                .inner;
            #[cfg(test)]
            {
                self.instrument_catalog_bounds.all_tab = all.rect;
            }
            if all.clicked() {
                self.set_instrument_subview(InstrumentSubview::All, ui.ctx());
            }
            let tabs = layers
                .iter()
                .enumerate()
                .map(|(index, (layer, name))| {
                    let label = name
                        .strip_prefix("Layer ")
                        .filter(|number| {
                            !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit())
                        })
                        .unwrap_or(name);
                    let result = ui
                        .push_id(*layer, |ui| {
                            Self::instrument_tab(
                                ui,
                                label,
                                enabled,
                                matches!(
                                    self.instrument_subview,
                                    InstrumentSubview::Layer | InstrumentSubview::Sources
                                ) && current == Some(*layer),
                                index + 1,
                                count,
                            )
                        })
                        .inner;
                    #[cfg(test)]
                    self.instrument_catalog_bounds
                        .layer_tabs
                        .insert(*layer, result.rect);
                    if enabled && self.instrument_subview == InstrumentSubview::Layer {
                        self.start_layer_drag(ui, &result, *layer);
                    }
                    if result.clicked() {
                        let toggle = self.instrument_subview == InstrumentSubview::Layer
                            && ui.input(|input| input.modifiers.shift);
                        self.set_instrument_subview(InstrumentSubview::Layer, ui.ctx());
                        crate::unit_selection::select(
                            &mut self.instrument_ui.selected_layers,
                            *layer,
                            toggle,
                        );
                        let selected = self
                            .instrument_ui
                            .selected_layers
                            .contains(layer)
                            .then_some(*layer)
                            .or_else(|| {
                                layers
                                    .iter()
                                    .find(|(id, _)| self.instrument_ui.selected_layers.contains(id))
                                    .map(|(id, _)| *id)
                            });
                        self.instrument_ui.layer_page = selected.or(Some(*layer));
                        self.instrument_ui.selected_layer = selected;
                        self.instrument_ui.effects_layer = None;
                        self.instrument_ui.track_effects_target = false;
                        self.selected_effects.clear();
                        ui.ctx().request_repaint();
                    }
                    if let Some(action) = self.layer_tab_menu(&result, *layer, enabled) {
                        self.perform_layer_tab_action(*layer, action);
                    }
                    result
                })
                .collect::<Vec<_>>();
            (all, tabs)
        });
        let (all, tabs) = group.inner;
        for response in &tabs {
            rows::group_separator(ui, group.response.rect, response.rect.left());
        }
        let rows: Vec<_> = layers
            .iter()
            .zip(&tabs)
            .map(|((id, _), response)| (*id, response.rect))
            .collect();
        let mut viewport = group.response.rect.intersect(ui.clip_rect());
        viewport.min.x = viewport.min.x.max(all.rect.right());
        if self.instrument_subview == InstrumentSubview::Layer {
            self.reorder_layer_tabs(ui, &rows, viewport);
            self.finish_layer_tab_drops(ui, &rows, viewport);
        }
    }

    fn layer_tab_menu(
        &self,
        response: &egui::Response,
        layer: Id,
        enabled: bool,
    ) -> Option<TrackAction> {
        let settings = self
            .session
            .project
            .instrument_settings(self.selected_instrument_owner()?)?;
        let layer = settings.layers.iter().find(|entry| entry.id == layer)?;
        if !enabled {
            egui::Popup::close_id(&response.ctx, egui::Popup::default_response_id(response));
            return None;
        }
        menus::ContextMenu::new(response)
            .show(|ui| {
                for (action, label, shortcut, available, selected) in [
                    (
                        TrackAction::Duplicate,
                        "Duplicate",
                        DUPLICATE_LAYER_SHORTCUT,
                        layer.can_duplicate_hosted(),
                        false,
                    ),
                    (
                        TrackAction::Mute,
                        "Mute",
                        TrackAction::Mute.shortcut().unwrap(),
                        true,
                        layer.muted,
                    ),
                    (
                        TrackAction::Solo,
                        "Solo",
                        TrackAction::Solo.shortcut().unwrap(),
                        true,
                        layer.soloed,
                    ),
                    (
                        TrackAction::Delete,
                        "Delete",
                        TrackAction::Delete.shortcut().unwrap(),
                        settings.layers.len() > 1,
                        false,
                    ),
                ] {
                    if matches!(action, TrackAction::Mute | TrackAction::Delete) {
                        menus::separator(ui);
                    }
                    let button = menus::action_button(label)
                        .selected(selected)
                        .shortcut_text(toolbars::menu_shortcut_text(ui.ctx(), &shortcut));
                    if ui.add_enabled(available, button).clicked() {
                        ui.close();
                        return Some(action);
                    }
                }
                None
            })
            .and_then(|menu| menu.inner)
    }

    fn finish_layer_tab_drops(&mut self, ui: &egui::Ui, tabs: &[(Id, Rect)], viewport: Rect) {
        let Some(track) = self.selected_instrument_owner() else {
            return;
        };
        let Some(pointer) = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
        else {
            return;
        };
        let Some(&(layer, tab)) = tabs.iter().find(|(_, tab)| tab.contains(pointer)) else {
            return;
        };
        let target = daw_core::EffectChainTarget::Layer { track, layer };
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        if let Some(transfer) =
            self.finish_cross_effect_drag(ui, target, &[], tab.intersect(viewport), allowed)
        {
            self.edit(transfer.edit);
            if self.error.is_none() {
                self.instrument_ui.select_layer(layer);
                self.instrument_ui.layer_page = Some(layer);
                self.instrument_ui.effects_layer = Some(layer);
                self.instrument_ui.track_effects_target = false;
                self.selected_effects = transfer.selected.into_iter().collect();
            }
            return;
        }
        if self.component_drag.is_none() || self.instrument_ui.layer_page == Some(layer) {
            return;
        }
        let Some(mut settings) = self.session.project.instrument_snapshot(track) else {
            return;
        };
        if self.visible_instrument_layer(&settings) == Some(layer) {
            return;
        }
        self.activate_component_drag(ui);
        let transfer =
            self.finish_component_drag(ui, track, &mut settings, &[(layer, tab)], viewport);
        if let Some((edit, components)) = transfer {
            self.edit(edit);
            if self.error.is_none() {
                self.instrument_ui.selected_components = components
                    .into_iter()
                    .map(|component| (layer, component))
                    .collect();
                self.instrument_ui.component_refocus = true;
            }
        } else if self.session.project.instrument_settings(track) != Some(&settings) {
            self.edit(Edit::SetInstrument {
                track_id: track,
                instrument: Some(settings),
            });
        } else {
            return;
        }
        if self.error.is_none() {
            self.instrument_ui.layer_page = Some(layer);
            self.instrument_ui.track_effects_target = false;
        }
    }

    fn instrument_tab(
        ui: &mut egui::Ui,
        label: &str,
        enabled: bool,
        selected: bool,
        index: usize,
        count: usize,
    ) -> egui::Response {
        ui.scope(|ui| {
            rows::input_group_corners(ui, index == 0, index + 1 == count);
            ui.add_enabled(
                enabled,
                egui::Button::new(label)
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .sense(Sense::click_and_drag())
                    .selected(selected)
                    .min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)),
            )
        })
        .inner
    }

    pub(super) fn set_instrument_subview(
        &mut self,
        subview: InstrumentSubview,
        ctx: &egui::Context,
    ) {
        if self.instrument_subview == subview {
            return;
        }
        self.instrument_subview = subview;
        self.instrument_ui.cancel();
        if matches!(
            subview,
            InstrumentSubview::All
                | InstrumentSubview::Lfos
                | InstrumentSubview::Envelopes
                | InstrumentSubview::Macros
        ) {
            self.instrument_ui.clear_layer_selection();
        }
        self.instrument_ui.effects_layer = None;
        self.selected_effects.clear();
        self.instrument_catalog.clear();
        #[cfg(test)]
        let compact_tracks = self.tracks_panel_visible();
        #[cfg(not(test))]
        let compact_tracks = false;
        if !compact_tracks {
            if subview == InstrumentSubview::Effects && self.instrument_catalog_visible {
                self.toggle_view_panel(ViewPanel::EffectsCatalog, ctx);
            } else if subview != InstrumentSubview::Effects && self.effects_catalog_visible() {
                self.toggle_view_panel(ViewPanel::Components, ctx);
            }
        }
        ctx.request_repaint();
    }

    fn instrument_track_column(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.max_rect();
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.track_column = bounds;
        }
        let panel = panels::ControlPanel::begin(ui, bounds);
        let buttons = self.timeline_track_buttons(ui, &panel, false);
        panels::ControlPanel::row_dividers(ui, buttons, false, true);
        let master = Rect::from_min_max(
            Pos2::new(
                bounds.left(),
                (bounds.bottom() - MASTER_HEIGHT).max(ui.cursor().top()),
            ),
            bounds.max,
        );
        let tracks = self.track_scroll_area(ui, master.top()).show(ui, |ui| {
            self.lane_bounds.clear();
            let rows = self.tracks_kind(ui, false);
            self.empty_tracks_context_menu(ui);
            self.scroll_track_drag(ui);
            rows
        });
        self.track_scroll_offset = tracks.state.offset.y;
        self.track_reorder(ui, tracks.inner_rect);
        let group_panels = self.track_panel_containers(&tracks.inner);
        self.paint_track_group_edges(ui, &group_panels, tracks.inner_rect);
        let mut master_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-master")
                .max_rect(master),
        );
        master_ui.set_clip_rect(master.intersect(ui.clip_rect()));
        self.master_track(&mut master_ui);
        panels::ControlPanel::row_dividers(ui, master, true, false);
        panel.finish(ui);
    }

    pub(super) fn instrument_workspace(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.max_rect();
        let rail = timeline_toolbar::right_bounds(bounds);
        timeline_toolbar::paint_right_rail(ui, rail);
        let size = timeline_toolbar::BUTTON_SIZE;
        let inset = (timeline_toolbar::WIDTH - size) * 0.5;
        let mut toolbar = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-right-toolbar")
                .max_rect(rail),
        );
        toolbar.set_clip_rect(rail.intersect(ui.clip_rect()));
        let panel = ViewPanel::TrackMixer;
        let response = timeline_toolbar::toggle_at(
            &mut toolbar,
            rail.min + Vec2::splat(inset),
            panel.label(),
            icons::Lucide::SlidersVertical,
            "Show track mixer",
            self.view_panel_selected(panel),
            self.view_panel_enabled(panel, ui.ctx()),
        );
        if response.clicked() {
            self.toggle_view_panel(panel, ui.ctx());
        }
        #[cfg(test)]
        {
            self.effects_bounds.track_mixer_toggle = response.rect;
        }
        let mixer_width = if self.instrument_track_mixer_visible {
            (2.0 * mixer::STRIP_WIDTH)
                .min((rail.left() - bounds.left() - TRACK_WIDTH - lane_toolbar::WIDTH).max(0.0))
        } else {
            0.0
        };
        let mut mixer = Rect::from_min_max(
            rail.left_top() - Vec2::new(mixer_width, 0.0),
            rail.left_bottom(),
        );
        let mut canvas = Rect::from_min_max(bounds.min, mixer.left_bottom());
        let tracks = self.instrument_subview.uses_track_column();
        if tracks {
            let width = TRACK_WIDTH.min(canvas.width());
            let track_bounds = Rect::from_min_size(canvas.min, Vec2::new(width, canvas.height()));
            let mut track_column = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("instrument-tracks")
                    .max_rect(track_bounds)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            track_column.set_clip_rect(track_bounds.intersect(ui.clip_rect()));
            self.instrument_track_column(&mut track_column);
            canvas.min.x = track_bounds.right();
        }
        let compact_header = tracks || canvas.width() < TRACK_WIDTH + 225.0;
        let tabs_left = if compact_header {
            canvas.left()
        } else {
            canvas.left() + TRACK_WIDTH
        };
        let (modulation_width, add_width) = Self::fixed_instrument_tab_widths(ui);
        let tabs_bottom = canvas.top() + toolbars::HEIGHT;
        let minimum_tabs_width = modulation_width
            + 3.0 * f32::from(theme::TOOLBAR_PADDING)
            + add_width
            + rows::GROUP_DIVIDER_WIDTH
            + toolbars::CONTROL_HEIGHT;
        let tabs_right = if mixer_width > 0.0 && canvas.right() - tabs_left < minimum_tabs_width {
            // Give the tab bar the full row when the mixer would squeeze it out.
            mixer.min.y = tabs_bottom.min(mixer.bottom());
            rail.left()
        } else {
            canvas.right()
        };
        let tabs = Rect::from_min_max(
            Pos2::new(tabs_left, canvas.top()),
            Pos2::new(tabs_right, tabs_bottom),
        );
        let mut tabs_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-subviews")
                .max_rect(tabs)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        tabs_ui.set_clip_rect(tabs.intersect(ui.clip_rect()));
        self.instrument_subview_tabs(&mut tabs_ui, modulation_width, add_width);
        let mut lanes = canvas;
        if compact_header {
            lanes.min.y = tabs.bottom().min(canvas.bottom());
        }
        let mut lanes_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-lanes")
                .max_rect(lanes)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lanes_ui.set_clip_rect(lanes.intersect(ui.clip_rect()));
        self.instrument_view(&mut lanes_ui);
        if self.instrument_track_mixer_visible {
            self.track_mixer_panel(
                ui,
                mixer,
                self.view_panel_enabled(ViewPanel::TrackMixer, ui.ctx()),
            );
        }
    }
}
