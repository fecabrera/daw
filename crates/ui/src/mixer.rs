use super::*;
use crate::panels::HEADER_HEIGHT;

pub(super) const STRIP_WIDTH: f32 = 120.0;
pub(super) const HEADER_TEXT: egui::Color32 = egui::Color32::WHITE;
pub(super) const GROUP_GUTTER_HEIGHT: f32 = HEADER_HEIGHT / 2.0;
const STRIP_BORDER: f32 = 1.0;
const OUT_PADDING: f32 = 8.0;
const STRIP_PADDING: Vec2 = Vec2::new(4.0, OUT_PADDING + STRIP_BORDER);
const CONTROL_SPACING: f32 = 6.0;
pub(super) const OUT_CONTROL_SPACING: f32 = 8.0;
const METER_COLUMN_SPACING: f32 = 1.0;
const FIXED_CONTROL_HEIGHT: f32 = HEADER_HEIGHT
    + knobs::PAN_VISIBLE_HEIGHT
    + 4.0 * toolbars::CONTROL_HEIGHT
    + 6.0 * OUT_CONTROL_SPACING;
const MIN_FADER_HEIGHT: f32 = 24.0;
pub(super) const MIN_HEIGHT: f32 = 2.0 * STRIP_PADDING.y
    + FIXED_CONTROL_HEIGHT
    + MIN_FADER_HEIGHT
    + toolbars::CONTROL_HEIGHT
    + OUT_CONTROL_SPACING;

fn send_controls_height(count: usize) -> f32 {
    count as f32 * (knobs::HEADER_SIZE + CONTROL_SPACING)
        + if count > 0 {
            OUT_CONTROL_SPACING - CONTROL_SPACING
        } else {
            0.0
        }
        + toolbars::CONTROL_HEIGHT
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Timeline,
    Instrument,
    Edit,
    Mixer,
}

impl ViewMode {
    pub const ALL: [Self; 4] = [Self::Timeline, Self::Instrument, Self::Edit, Self::Mixer];

    pub fn shortcut(self) -> egui::KeyboardShortcut {
        egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            match self {
                Self::Timeline => egui::Key::Num1,
                Self::Instrument => egui::Key::Num2,
                Self::Edit => egui::Key::Num3,
                Self::Mixer => egui::Key::Num4,
            },
        )
    }

    pub(super) fn has_effects(self) -> bool {
        matches!(
            self,
            Self::Timeline | Self::Instrument | Self::Edit | Self::Mixer
        )
    }

    pub(super) fn has_track_overview(self) -> bool {
        self == Self::Edit
    }

    pub(super) fn has_clip_lanes(self) -> bool {
        self == Self::Timeline || self.has_track_overview()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Timeline => "Timeline",
            Self::Mixer => "Mixer",
            Self::Instrument => "Instrument",
            Self::Edit => "Editor",
        }
    }
}

impl DawUi {
    pub(super) fn view_shortcuts(&mut self, ctx: &egui::Context) {
        if self.native_menu
            || !self.view_switch_enabled()
            || ctx.input(|input| !input.focused || input.pointer.any_down())
        {
            return;
        }
        let mode = ctx.input_mut(|input| {
            let mut selected = None;
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                    && let Some(mode) = ViewMode::ALL.into_iter().find(|mode| {
                        let shortcut = mode.shortcut();
                        *key == shortcut.logical_key && modifiers.matches_exact(shortcut.modifiers)
                    })
                {
                    selected = Some(mode);
                    return false;
                }
                true
            });
            selected
        });
        if let Some(mode) = mode {
            self.set_view_mode(mode, ctx);
        }
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn view_switch_enabled(&self) -> bool {
        self.settings_enabled()
            && !self.midi_editor.dragging()
            && self.portion_drag.is_none()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, ctx: &egui::Context) {
        if !self.view_switch_enabled() || self.view_mode == mode {
            return;
        }
        self.cancel_browser_interactions();
        self.cancel_beat_reorder(None);
        self.view_mode = mode;
        if mode == ViewMode::Edit {
            self.midi_editor.delete_clips = true;
        }
        if mode != ViewMode::Timeline {
            self.clip_settings_visible = false;
        }
        self.clear_clip_settings_edits();
        self.effects_catalog.clear();
        self.instrument_catalog.clear();
        self.selected_effects.clear();
        self.midi_editor.cancel();
        self.instrument_ui.cancel();
        self.track_name_edit = None;
        self.tempo_edit = None;
        self.time_signature_control = Default::default();
        self.inputs.clear();
        self.master_gain_input = faders::format_gain(self.session.project.master.gain_db);
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        ctx.request_repaint();
    }

    pub(super) fn view_menu(&mut self, ui: &mut egui::Ui) {
        for mode in ViewMode::ALL {
            if ui
                .add_enabled_ui(self.view_switch_enabled(), |ui| {
                    crate::menus::checked_button(
                        ui,
                        self.view_mode == mode,
                        mode.label(),
                        toolbars::menu_shortcut_text(ui.ctx(), &mode.shortcut()),
                    )
                })
                .inner
                .clicked()
            {
                self.set_view_mode(mode, ui.ctx());
                ui.close();
            }
        }
        menus::separator(ui);
        for panel in ViewPanel::ALL {
            let selected = self.view_panel_selected(panel);
            let enabled = self.view_panel_enabled(panel, ui.ctx());
            if ui
                .add_enabled_ui(enabled, |ui| {
                    menus::checked_button(ui, selected, panel.label(), "")
                })
                .inner
                .clicked()
            {
                self.toggle_view_panel(panel, ui.ctx());
                ui.close();
            }
        }
        menus::separator(ui);
        let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
        if ui
            .add_enabled(
                self.view_switch_enabled(),
                egui::Button::new(if fullscreen {
                    "Exit Full Screen"
                } else {
                    "Enter Full Screen"
                }),
            )
            .clicked()
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            ui.close();
        }
    }

    pub(super) fn view_controls(&mut self, ui: &mut egui::Ui) {
        let enabled = self.view_switch_enabled();
        ui.scope(|ui| {
            self.unit_header_press_style(ui);
            // Menu bars change padding; use the same control style in every titlebar layout.
            let style = ui.ctx().global_style();
            ui.spacing_mut().button_padding = style.spacing.button_padding;
            let (selected_fill, bright_text) = theme::toggle_colors(ui.ctx(), true);
            let (_, secondary_text) = theme::toggle_colors(ui.ctx(), false);
            let widgets = &mut ui.visuals_mut().widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
                &mut widgets.noninteractive,
            ] {
                widget.bg_stroke = Stroke::NONE;
                widget.expansion = 0.0;
                widget.fg_stroke.color = bright_text;
            }
            widgets.inactive.fg_stroke.color = secondary_text;
            widgets.noninteractive.fg_stroke.color = secondary_text;
            ui.visuals_mut().selection.stroke.color = bright_text;
            for mode in ViewMode::ALL {
                let label = mode.label();
                let selected = self.view_mode == mode;
                // Reserve the backgrounds before the Button paints its text.
                let background = ui.painter().add(egui::Shape::Noop);
                let hover = ui.painter().add(egui::Shape::Noop);
                let response = ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(egui::RichText::new(label).font(crate::fonts::semibold(
                            egui::TextStyle::Button.resolve(&style).size,
                        )))
                        .sense(egui::Sense::click_and_drag())
                        .selected(selected)
                        .frame(true)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .corner_radius(icons::TOGGLE_CORNER_RADIUS)
                        .min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(crate::toolbars::action_tooltip(
                        ui.ctx(),
                        &format!("Show {label}"),
                        Some(mode.shortcut()),
                    ));
                if mode == ViewMode::Instrument
                    && ui.input(|i| {
                        i.pointer
                            .latest_pos()
                            .is_some_and(|p| response.rect.contains(p))
                    })
                {
                    self.sampler_drag_navigation(ui.ctx());
                }
                let radius = icons::TOGGLE_CORNER_RADIUS;
                ui.painter().set(
                    background,
                    egui::Shape::rect_filled(
                        response.rect,
                        radius,
                        if selected {
                            selected_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        },
                    ),
                );
                if !selected
                    && response.enabled()
                    && (response.hovered() || response.is_pointer_button_down_on())
                {
                    ui.painter().set(
                        hover,
                        egui::Shape::rect_filled(response.rect, radius, theme::TOOLBAR_HOVER_FILL),
                    );
                }
                if response.clicked() {
                    self.set_view_mode(mode, ui.ctx());
                }
            }
        });
    }

    pub(super) fn mixer_send_height(&self) -> f32 {
        let count = self
            .session
            .project
            .visible_tracks()
            .into_iter()
            .map(|i| self.session.project.tracks[i].sends.len())
            .max()
            .unwrap_or(0);
        send_controls_height(count) + OUT_CONTROL_SPACING + STRIP_BORDER * 0.5
    }

    pub(super) fn mixer(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.max_rect();
        let rail = Rect::from_min_max(
            Pos2::new(
                (bounds.right() - timeline_toolbar::WIDTH).max(bounds.left()),
                bounds.top(),
            ),
            bounds.max,
        );
        let palette = theme::palette(ui.ctx());
        ui.painter().rect_filled(rail, 0.0, palette.panel);
        ui.painter().vline(
            rail.left() + 0.5,
            rail.y_range(),
            Stroke::new(1.0, palette.border),
        );
        self.return_master_buttons(ui, rail);
        let content = Rect::from_min_max(bounds.min, rail.left_bottom());
        let mut body = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("mixer-content-workspace")
                .max_rect(content),
        );
        body.set_clip_rect(content.intersect(ui.clip_rect()));
        self.mixer_scroll_content(&mut body);
    }

    pub(super) fn mixer_gutter_height(&self) -> f32 {
        self.session
            .project
            .visible_tracks()
            .into_iter()
            .map(|i| {
                self.session
                    .project
                    .track_depth(self.session.project.tracks[i].id)
                    .min(3)
            })
            .max()
            .unwrap_or(0) as f32
            * GROUP_GUTTER_HEIGHT
    }

    fn mixer_scroll_content(&mut self, ui: &mut egui::Ui) {
        let base_height = MIN_HEIGHT - toolbars::CONTROL_HEIGHT - OUT_CONTROL_SPACING;
        let height = (base_height + self.mixer_send_height() + self.mixer_gutter_height())
            .max(ui.available_height());
        egui::ScrollArea::vertical()
            .id_salt("mixer_vertical")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
                let mut body =
                    ui.new_child(egui::UiBuilder::new().id_salt("mixer_body").max_rect(rect));
                body.set_clip_rect(rect.intersect(ui.clip_rect()));
                self.mixer_content(&mut body);
            });
    }

    fn mixer_content(&mut self, ui: &mut egui::Ui) {
        // Hidden timeline geometry must never remain a destination for a file drop.
        self.timeline_bounds = Rect::NOTHING;
        self.lane_bounds.clear();
        self.file_drop_target = None;
        #[cfg(test)]
        {
            self.mixer_bounds.clear();
            self.mixer_faders.clear();
        }
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let bounds = ui.max_rect();
        let master_left = if self.mixer_master_visible {
            (bounds.right() - STRIP_WIDTH).max(bounds.left())
        } else {
            bounds.right()
        };
        #[cfg(test)]
        {
            self.master_bounds = Rect::NOTHING;
        }
        if self.mixer_master_visible {
            let master_rect = Rect::from_min_max(Pos2::new(master_left, bounds.top()), bounds.max);
            #[cfg(test)]
            {
                self.master_bounds = master_rect;
            }
            let mut master_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("mixer_master")
                    .max_rect(master_rect),
            );
            master_ui.set_clip_rect(master_rect.intersect(ui.clip_rect()));
            let _ = self.mixer_strip(
                &mut master_ui,
                master_rect,
                None,
                master_left > bounds.left(),
            );
            self.master_selection(&master_ui, master_rect);
        }

        let return_count = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| self.mixer_returns_visible && t.kind == daw_core::TrackKind::Return)
            .count();
        let available_width = (master_left - bounds.left()).max(0.0);
        let source_width = if self
            .session
            .project
            .tracks
            .iter()
            .any(|track| track.kind != daw_core::TrackKind::Return)
        {
            STRIP_WIDTH.min(available_width)
        } else {
            0.0
        };
        // Fit every Return at its standard width before enabling bank overflow.
        // Keep one source strip usable and leave Master outside both scroll areas.
        let bank_width =
            (return_count as f32 * STRIP_WIDTH).min((available_width - source_width).max(0.0));
        let return_rect = Rect::from_min_max(
            Pos2::new(master_left - bank_width, bounds.top()),
            Pos2::new(master_left, bounds.bottom()),
        );
        self.return_mixer_bank(ui, return_rect);
        let tracks_rect =
            Rect::from_min_max(bounds.min, Pos2::new(return_rect.left(), bounds.bottom()));
        let mut tracks_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("mixer_tracks")
                .max_rect(tracks_rect),
        );
        tracks_ui.set_clip_rect(tracks_rect.intersect(ui.clip_rect()));
        let tracks: Vec<_> = self
            .session
            .project
            .visible_tracks()
            .into_iter()
            .filter(|&i| self.session.project.tracks[i].kind != daw_core::TrackKind::Return)
            .map(|i| self.session.project.tracks[i].clone())
            .collect();
        let mut action = None;
        let mut outlines = Vec::new();
        egui::ScrollArea::horizontal()
            .id_salt("mixer_scroll")
            .auto_shrink([false, false])
            .show(&mut tracks_ui, |ui| {
                ui.horizontal_top(|ui| {
                    let height = ui.available_height();
                    for track in tracks {
                        ui.push_id(track.id, |ui| {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(STRIP_WIDTH, height),
                                Sense::click(),
                            );
                            let mut strip_ui = ui
                                .new_child(egui::UiBuilder::new().max_rect(rect).id_salt("strip"));
                            let rack_bounds = self.mixer_strip(
                                &mut strip_ui,
                                rect,
                                Some(&track),
                                rect.left() > tracks_rect.left(),
                            );
                            #[cfg(test)]
                            self.mixer_bounds.insert(track.id, rect);
                            // Select the whole panel without taking input from its controls.
                            if ui.is_enabled()
                                && self.editing_enabled()
                                && ui.rect_contains_pointer(rect)
                                && ui.input(|input| input.focused && input.pointer.any_pressed())
                            {
                                if ui.input(|i| i.pointer.secondary_pressed()) {
                                    if !self.track_selected(track.id) {
                                        self.select_track(track.id, false);
                                    }
                                } else if ui.input(|i| i.pointer.primary_pressed()) {
                                    let toggle =
                                        ui.input(|i| i.modifiers.shift || i.modifiers.command);
                                    self.select_track(track.id, toggle);
                                }
                            }
                            let send_hover = !track.sends.is_empty()
                                && rack_bounds.is_some_and(|r| ui.rect_contains_pointer(r));
                            let context_click = !send_hover
                                && ui.is_enabled()
                                && self.editing_enabled()
                                && ui.rect_contains_pointer(rect)
                                && ui.input(|i| i.focused && i.pointer.secondary_clicked());
                            if let Some(selected) = crate::menus::ContextMenu::new(&response)
                                .open_memory(if send_hover {
                                    Some(egui::SetOpenCommand::Bool(false))
                                } else {
                                    context_click.then_some(egui::SetOpenCommand::Bool(true))
                                })
                                .show(|ui| self.track_context_menu(ui, Some(track.id)))
                                .and_then(|response| response.inner)
                            {
                                action = Some((track.id, selected));
                            }
                            outlines.push((track.id, rect));
                        });
                    }
                });
            });
        // Each strip owns its left divider. Only the last exposed edge needs a right one.
        if let Some((_, rect)) = outlines.last()
            && rect.right() < tracks_rect.right()
        {
            tracks_ui.painter().vline(
                rect.right() - 0.5,
                rect.y_range(),
                Stroke::new(1.0, theme::palette(ui.ctx()).border),
            );
        }
        if let Some((track, action)) = action {
            if matches!(
                action,
                TrackAction::Add
                    | TrackAction::AddMidi
                    | TrackAction::AddBeat
                    | TrackAction::AddGroup
            ) {
                self.add_track_to_group_from_menu(track, action, ui.ctx());
            } else if action == TrackAction::AddSubtrack {
                self.add_subtrack_from_menu(track, ui.ctx());
            } else {
                self.perform_track_action(action, ui.ctx());
            }
        }
    }

    pub(super) fn center_mixer_header(&self, ui: &mut egui::Ui, name: &str) {
        let leading_width = icons::TOOLBAR_ICON_SIZE + ui.spacing().item_spacing.x;
        let text = egui::WidgetText::from(
            egui::RichText::new(name).font(crate::fonts::semibold(panels::HEADER_FONT_SIZE)),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (ui.available_width() - leading_width).max(0.0),
            egui::TextStyle::Button,
        );
        // Measure the truncated label so the icon and visible text form one centered group.
        ui.add_space(((ui.available_width() - leading_width - text.size().x) / 2.0).max(0.0));
    }

    fn mixer_output_destination(&self, ui: &mut egui::Ui, track: &daw_core::Track, width: f32) {
        let palette = theme::palette(ui.ctx());
        let border = Stroke::new(STRIP_BORDER, palette.border);
        let destination = track
            .group_id
            .and_then(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|parent| parent.id == id)
            })
            .map_or("Master", |parent| parent.name.as_str());
        let (destination_rect, response) =
            ui.allocate_exact_size(Vec2::new(width, toolbars::CONTROL_HEIGHT), Sense::hover());
        ui.painter()
            .rect_stroke(destination_rect, 5.0, border, egui::StrokeKind::Inside);
        let text = egui::WidgetText::from(egui::RichText::new(destination).small()).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (destination_rect.width() - 8.0 - icons::TOOLBAR_ICON_SIZE - 4.0).max(0.0),
            egui::TextStyle::Small,
        );
        let icon_width = icons::TOOLBAR_ICON_SIZE;
        let left = destination_rect.center().x - (icon_width + 4.0 + text.size().x) / 2.0;
        icons::Lucide::ArrowRight
            .size(icon_width)
            .stroke_width(2.0)
            .color(palette.secondary)
            .image()
            .alt_text("Output destination")
            .paint_at(
                ui,
                Rect::from_center_size(
                    Pos2::new(left + icon_width / 2.0, destination_rect.center().y),
                    Vec2::splat(icon_width),
                ),
            );
        ui.painter().galley(
            Pos2::new(
                left + icon_width + 4.0,
                destination_rect.center().y - text.size().y / 2.0,
            ),
            text,
            palette.text,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Label,
                ui.is_enabled(),
                format!("Output destination: {destination}"),
            )
        });
        response.on_hover_text(format!("Output destination: {destination}"));
    }

    pub(super) fn mixer_strip(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        track: Option<&daw_core::Track>,
        left_divider: bool,
    ) -> Option<Rect> {
        let mut rack_bounds = None;
        let palette = theme::palette(ui.ctx());
        let selected = track.map_or(self.master_selected, |track| self.track_selected(track.id));
        ui.painter().rect_filled(
            rect,
            0.0,
            if selected {
                theme::selected_track_header_background(ui.ctx())
            } else {
                palette.panel
            },
        );
        let header_fill = track.map_or_else(
            || theme::unit_header_fill(ui.ctx(), false),
            |track| theme::clip_colors(track.color, None).header,
        );
        // The first three ancestor bands mirror Timeline's nesting cap. Paint
        // each ancestor across every descendant so the gutter stays connected
        // when its parent scrolls outside the viewport.
        let mut ancestors = Vec::new();
        let mut parent = track.and_then(|track| track.group_id);
        while let Some(id) = parent {
            let Some(group) = self
                .session
                .project
                .tracks
                .iter()
                .find(|track| track.id == id)
            else {
                break;
            };
            ancestors.push(group);
            parent = group.group_id;
        }
        let header_top = rect.top() + ancestors.len().min(3) as f32 * GROUP_GUTTER_HEIGHT;
        for (level, group) in ancestors.into_iter().rev().take(3).enumerate() {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.left(), rect.top() + level as f32 * GROUP_GUTTER_HEIGHT),
                    Vec2::new(rect.width(), GROUP_GUTTER_HEIGHT),
                ),
                0.0,
                theme::clip_colors(group.color, None).header,
            );
        }
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), header_top),
                Vec2::new(rect.width(), HEADER_HEIGHT),
            ),
            0.0,
            header_fill,
        );
        let border = Stroke::new(STRIP_BORDER, palette.border);
        if header_top > rect.top() {
            ui.painter().hline(rect.x_range(), header_top + 0.5, border);
        }
        // The toolbar or open catalog owns the workspace's left separator.
        if left_divider {
            ui.painter()
                .vline(rect.left() + 0.5, header_top..=rect.bottom(), border);
        }
        // The right toolbar owns the single outer divider in both workspaces.
        if track.is_none() && !matches!(self.view_mode, ViewMode::Timeline | ViewMode::Mixer) {
            ui.painter()
                .vline(rect.right() - 0.5, rect.y_range(), border);
        }
        let inner = rect.shrink2(STRIP_PADDING);
        let content_rect = Rect::from_min_max(
            Pos2::new(
                inner.left(),
                header_top + (HEADER_HEIGHT - toolbars::CONTROL_HEIGHT) / 2.0,
            ),
            inner.max,
        );
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("controls")
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        content.spacing_mut().item_spacing = Vec2::splat(CONTROL_SPACING);
        // Match the icon-to-label gap to the vertical padding around the header artwork.
        content.spacing_mut().item_spacing.x = (HEADER_HEIGHT - icons::TOOLBAR_ICON_SIZE) / 2.0;
        if let Some(track) = track {
            self.track_name_layout(&mut content, track, true);
        } else {
            self.master_name_layout(&mut content, true);
        }
        content.spacing_mut().item_spacing.x = CONTROL_SPACING;
        content.spacing_mut().item_spacing.y = OUT_CONTROL_SPACING;
        if let Some(track) = track {
            content.add_space(
                (header_top + HEADER_HEIGHT + OUT_CONTROL_SPACING - content.cursor().top())
                    .max(0.0),
            );
            // Match the send rows' inset within the strip's shared content padding.
            self.mixer_output_destination(
                &mut content,
                track,
                (inner.width() - 2.0 * f32::from(menus::ITEM_PADDING)).max(0.0),
            );
        }
        // Align all faders, meters, and bottom controls, including Master.
        let rack_height = self.mixer_send_height();
        let fader_height =
            (inner.height() - FIXED_CONTROL_HEIGHT - rack_height - self.mixer_gutter_height())
                .clamp(MIN_FADER_HEIGHT, 220.0);
        let numeric_height = numeric_field_height(&content);
        let controls_height = knobs::PAN_VISIBLE_HEIGHT
            + 2.0 * numeric_height
            + toolbars::CONTROL_HEIGHT
            + fader_height
            + 4.0 * OUT_CONTROL_SPACING;
        content.add_space(
            (inner.bottom()
                - content.cursor().top()
                - controls_height
                - rack_height
                - OUT_CONTROL_SPACING
                - STRIP_BORDER * 0.5)
                .max(0.0),
        );
        if rack_height > 0.0 {
            let (rack_rect, _) =
                content.allocate_exact_size(Vec2::new(inner.width(), rack_height), Sense::hover());
            // The remaining standard item gap after this slot mirrors this inset.
            let send_rect = Rect::from_min_max(
                rack_rect.min,
                Pos2::new(
                    rack_rect.right(),
                    (rack_rect.bottom() - OUT_CONTROL_SPACING - STRIP_BORDER * 0.5)
                        .max(rack_rect.top()),
                ),
            );
            let mut rack = content.new_child(
                egui::UiBuilder::new()
                    .id_salt("send_rack")
                    .max_rect(send_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            rack.set_clip_rect(send_rect.intersect(content.clip_rect()));
            rack.spacing_mut().item_spacing.y = CONTROL_SPACING;
            if let Some(track) = track.filter(|t| t.kind != daw_core::TrackKind::Return) {
                rack_bounds = Some(send_rect);
                rack.add_space(
                    (send_rect.height() - send_controls_height(track.sends.len())).max(0.0),
                );
                rack.add_enabled_ui(self.editing_enabled(), |ui| self.send_rack(ui, track));
                ui.painter()
                    .hline(rect.x_range(), rack_rect.bottom(), border);
            }
        }
        // Visible padding is measured from the divider stroke and strip border.
        let output_rect = Rect::from_min_max(
            Pos2::new(
                rect.left() + OUT_PADDING + STRIP_BORDER,
                content.cursor().top() + STRIP_BORDER * 0.5,
            ),
            Pos2::new(rect.right() - OUT_PADDING - STRIP_BORDER, inner.bottom()),
        );
        let mut content = content.new_child(
            egui::UiBuilder::new()
                .id_salt("out_controls")
                .max_rect(output_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        content.spacing_mut().item_spacing = Vec2::new(CONTROL_SPACING, OUT_CONTROL_SPACING);
        let mut gain = track.map_or(self.session.project.master.gain_db, |t| t.gain_db);
        let mut pan = track.map_or(0.0, |t| t.pan);
        let mut muted = track.is_some_and(|t| t.muted);
        let mut soloed = track.is_some_and(|t| t.soloed);
        let previous = (gain, pan, muted, soloed);
        let mut inputs = track.map_or_else(
            || Inputs {
                gain: self.master_gain_input.clone(),
                pan: String::new(),
            },
            |track| {
                self.inputs.remove(&track.id).unwrap_or_else(|| Inputs {
                    gain: faders::format_gain(gain),
                    pan: format!("{pan:.2}"),
                })
            },
        );
        if track.is_some() {
            if knobs::pan_visible(&mut content, &mut pan).changed() {
                inputs.pan = format!("{pan:.2}");
            }
            if let Err(error) = mixer_numeric(&mut content, &mut inputs.pan, &mut pan, false, true)
            {
                self.fail(error);
            }
        } else {
            content.allocate_space(Vec2::new(
                1.0,
                knobs::PAN_VISIBLE_HEIGHT + numeric_height + OUT_CONTROL_SPACING,
            ));
        }
        let levels = self.output.as_ref().map_or([(0.0, false); 2], |output| {
            track.map_or_else(
                || output.status.meter.levels(),
                |track| {
                    output
                        .track_meter(track.id)
                        .map_or([(0.0, false); 2], |meter| meter.levels())
                },
            )
        });
        let mut cleared = [false; 2];
        let column_widths = [
            faders::SCALE_WIDTH + faders::rail_width(&content),
            meters::STEREO_WIDTH,
        ];
        let column_height = numeric_height + OUT_CONTROL_SPACING + fader_height;
        content.allocate_ui_with_layout(
            Vec2::new(
                column_widths.iter().sum::<f32>() + METER_COLUMN_SPACING,
                column_height,
            ),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.x = METER_COLUMN_SPACING;
                ui.allocate_ui_with_layout(
                    Vec2::new(column_widths[0], column_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().interact_size.y = numeric_height;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add_space((column_widths[0] - NUMERIC_WIDTH) / 2.0);
                            if let Err(error) = mixer_numeric(
                                ui,
                                &mut inputs.gain,
                                &mut gain,
                                true,
                                track.is_none(),
                            ) {
                                self.fail(error);
                            }
                        });
                        ui.spacing_mut().interact_size.y = toolbars::CONTROL_HEIGHT;
                        let fader = faders::gain(ui, &mut gain, fader_height);
                        #[cfg(test)]
                        self.mixer_faders.insert(track.map(|t| t.id), fader.rect);
                        if fader.changed() {
                            inputs.gain = faders::format_gain(gain);
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    Vec2::new(column_widths[1], column_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let idle_meter = daw_output::StereoMeter::default();
                        let meter = self.output.as_ref().map_or(&idle_meter, |output| {
                            track.map_or(&output.status.meter, |track| {
                                output.track_meter(track.id).unwrap_or(&idle_meter)
                            })
                        });
                        ui.spacing_mut().interact_size.y = numeric_height;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add_space((column_widths[1] - NUMERIC_WIDTH) / 2.0);
                            meters::peak_level(ui, meter);
                        });
                        cleared = meters::stereo_held(ui, fader_height, levels, meter.held_peaks());
                    },
                );
            },
        );
        if track.is_some() {
            content.allocate_ui_with_layout(
                Vec2::new(
                    2.0 * toolbars::CONTROL_HEIGHT + rows::GROUP_DIVIDER_WIDTH,
                    toolbars::CONTROL_HEIGHT,
                ),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    mix_controls::mute_solo(ui, &mut muted, &mut soloed, ["Mute", "Solo"]);
                },
            );
        } else {
            content.allocate_space(Vec2::new(1.0, toolbars::CONTROL_HEIGHT));
        }
        if let Some(track) = track {
            self.inputs.insert(track.id, inputs);
            if previous != (gain, pan, muted, soloed) {
                if let Some(t) = self
                    .session
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track.id)
                {
                    t.gain_db = gain;
                    t.pan = pan;
                    t.muted = muted;
                    t.soloed = soloed;
                }
                self.changed();
            }
        } else {
            self.master_gain_input = inputs.gain;
            if gain != previous.0 {
                self.session.project.master.gain_db = gain;
                self.changed();
            }
        }
        let meter = self.output.as_ref().and_then(|output| {
            track.map_or(Some(&output.status.meter), |track| {
                output.track_meter(track.id)
            })
        });
        meters::reset_on_click(ui, meter, cleared);
        rack_bounds
    }
}

fn mixer_numeric(
    ui: &mut egui::Ui,
    text: &mut String,
    value: &mut f32,
    gain: bool,
    clamp: bool,
) -> Result<(), &'static str> {
    let response = numeric_input(
        ui,
        text,
        if gain {
            "Gain (dB)"
        } else {
            "Pan (-1 left, 0 center, +1 right)"
        },
    );
    let format = |v: f32| {
        if gain {
            faders::format_gain(v)
        } else {
            format!("{v:.2}")
        }
    };
    if (response.has_focus() || response.lost_focus())
        && ui.input(|i| i.key_pressed(egui::Key::Escape))
    {
        *text = format(*value);
        response.surrender_focus();
    } else if response.lost_focus()
        || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
    {
        let parsed = if gain {
            faders::parse_gain(text)
        } else {
            text.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && (-1.0..=1.0).contains(v))
        };
        if let Some(parsed) = parsed {
            *value = if gain && clamp {
                faders::clamp_gain(parsed)
            } else {
                parsed
            };
            *text = format(*value);
        } else {
            *text = format(*value);
            return Err(if gain {
                "Gain must be a finite number or −∞"
            } else {
                "Pan must be between -1 and 1"
            });
        }
    }
    Ok(())
}
