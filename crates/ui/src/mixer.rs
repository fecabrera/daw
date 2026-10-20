use super::*;
use crate::panels::HEADER_HEIGHT;

const STRIP_WIDTH: f32 = 120.0;
const STRIP_PADDING: Vec2 = Vec2::new(4.0, 7.0);
const CONTROL_SPACING: f32 = 6.0;
const METER_COLUMN_SPACING: f32 = 1.0;

// Leave the bottom edge to the single mixer/status separator.
fn strip_edges(ui: &egui::Ui, rect: Rect, stroke: Stroke) {
    let edge = rect.shrink(stroke.width / 2.0);
    ui.painter().line(
        vec![
            Pos2::new(edge.left(), rect.bottom()),
            edge.left_top(),
            edge.right_top(),
            Pos2::new(edge.right(), rect.bottom()),
        ],
        stroke,
    );
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
        matches!(self, Self::Timeline | Self::Instrument)
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
        self.view_mode = mode;
        if mode != ViewMode::Timeline {
            self.clip_settings_visible = false;
            self.clip_settings_draft = None;
            self.clip_transpose_drag = None;
            self.clip_settings_change = None;
        }
        self.effects_catalog.clear();
        self.instrument_catalog.clear();
        self.selected_effects.clear();
        self.midi_editor.cancel();
        self.instrument_ui.cancel();
        self.track_name_edit = None;
        self.tempo_edit = None;
        self.inputs.clear();
        self.master_gain_input = faders::format_gain(self.session.project.master.gain_db);
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        ctx.request_repaint();
    }

    pub(super) fn view_menu(&mut self, ui: &mut egui::Ui) {
        for mode in ViewMode::ALL {
            let mut selected = self.view_mode == mode;
            if ui
                .add_enabled(
                    self.view_switch_enabled(),
                    egui::Checkbox::new(
                        &mut selected,
                        (
                            mode.label(),
                            egui::Atom::grow(),
                            toolbars::menu_shortcut_text(ui.ctx(), &mode.shortcut()).weak(),
                        ),
                    ),
                )
                .clicked()
            {
                self.set_view_mode(mode, ui.ctx());
                ui.close();
            }
        }
    }

    pub(super) fn view_controls(&mut self, ui: &mut egui::Ui) {
        let enabled = self.view_switch_enabled();
        ui.scope(|ui| {
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
                        egui::Button::new(
                            egui::RichText::new(label)
                                .font(egui::TextStyle::Button.resolve(&style)),
                        )
                        .selected(selected)
                        .frame(true)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .corner_radius(style.visuals.widgets.inactive.corner_radius)
                        .min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("Show {label} view"));
                if mode == ViewMode::Instrument
                    && ui.input(|i| {
                        i.pointer
                            .latest_pos()
                            .is_some_and(|p| response.rect.contains(p))
                    })
                {
                    self.sampler_drag_navigation(ui.ctx());
                }
                let radius = style.visuals.widgets.inactive.corner_radius;
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
                if response.enabled()
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

    pub(super) fn mixer(&mut self, ui: &mut egui::Ui) {
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
        let master_left = (bounds.right() - STRIP_WIDTH).max(bounds.left());
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
        self.mixer_strip(&mut master_ui, master_rect, None);
        self.master_selection(&master_ui, master_rect);

        let tracks_rect = Rect::from_min_max(bounds.min, Pos2::new(master_left, bounds.bottom()));
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
                            self.mixer_strip(&mut strip_ui, rect, Some(&track));
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
                            let context_click = ui.is_enabled()
                                && self.editing_enabled()
                                && ui.rect_contains_pointer(rect)
                                && ui.input(|i| i.focused && i.pointer.secondary_clicked());
                            if let Some(selected) = egui::Popup::context_menu(&response)
                                .open_memory(
                                    context_click.then_some(egui::SetOpenCommand::Bool(true)),
                                )
                                .show(|ui| self.track_context_menu(ui))
                                .and_then(|response| response.inner)
                            {
                                action = Some(selected);
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
        // Shared horizontal borders replace stacked toolbar/strip/status borders.
        for y in [bounds.top() + 0.5, bounds.bottom() - 0.5] {
            ui.painter().hline(
                bounds.x_range(),
                y,
                Stroke::new(1.0, theme::palette(ui.ctx()).border),
            );
        }
        for (id, rect) in outlines {
            if self.track_selected(id) {
                let outline = theme::selection_outline(ui.ctx());
                let mut outline_rect = rect.shrink(1.0);
                // Reach the next strip's divider without leaving header fill outside the stroke.
                outline_rect.max.x = rect.right();
                strip_edges(&tracks_ui, outline_rect, outline);
                // Replace this portion of the shared line; do not add another row.
                tracks_ui.painter().hline(
                    rect.shrink(1.0).x_range(),
                    bounds.bottom() - outline.width / 2.0,
                    outline,
                );
            }
        }
        if let Some(action) = action {
            self.perform_track_action(action, ui.ctx());
        }
    }

    fn mixer_strip(&mut self, ui: &mut egui::Ui, rect: Rect, track: Option<&daw_core::Track>) {
        let palette = theme::palette(ui.ctx());
        ui.painter().rect_filled(rect, 0.0, palette.panel);
        if let Some(track) = track {
            let colors = theme::clip_colors(track.color, None);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER_HEIGHT)),
                0.0,
                colors.header,
            );
        }
        let border = Stroke::new(1.0, palette.border);
        ui.painter()
            .vline(rect.left() + 0.5, rect.y_range(), border);
        if track.is_none() {
            ui.painter()
                .vline(rect.right() - 0.5, rect.y_range(), border);
        }
        let inner = rect.shrink2(STRIP_PADDING);
        let content_rect = Rect::from_min_max(
            Pos2::new(
                inner.left(),
                rect.top() + (HEADER_HEIGHT - toolbars::CONTROL_HEIGHT) / 2.0,
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
        if let Some(track) = track {
            self.track_name(&mut content, track);
        } else {
            self.master_name(&mut content);
        }
        // Align all faders, meters, and bottom controls, including Master.
        let fader_height = (inner.height() - 154.0).clamp(24.0, 220.0);
        let controls_height =
            knobs::SIZE + 3.0 * toolbars::CONTROL_HEIGHT + fader_height + 4.0 * CONTROL_SPACING;
        content.add_space((inner.bottom() - content.cursor().top() - controls_height).max(0.0));
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
            if knobs::pan(&mut content, &mut pan).changed() {
                inputs.pan = format!("{pan:.2}");
            }
            if let Err(error) = mixer_numeric(&mut content, &mut inputs.pan, &mut pan, false, true)
            {
                self.fail(error);
            }
        } else {
            content.allocate_space(Vec2::new(
                1.0,
                knobs::SIZE + toolbars::CONTROL_HEIGHT + CONTROL_SPACING,
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
        let column_height = toolbars::CONTROL_HEIGHT + CONTROL_SPACING + fader_height;
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
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add_space((column_widths[1] - NUMERIC_WIDTH) / 2.0);
                            let idle_meter = daw_output::StereoMeter::default();
                            let meter = self.output.as_ref().map_or(&idle_meter, |output| {
                                track.map_or(&output.status.meter, |track| {
                                    output.track_meter(track.id).unwrap_or(&idle_meter)
                                })
                            });
                            meters::peak_level(ui, meter);
                        });
                        cleared = meters::stereo(ui, fader_height, levels)
                            .map(|response| response.clicked());
                    },
                );
            },
        );
        if track.is_some() {
            content.allocate_ui_with_layout(
                Vec2::new(
                    2.0 * toolbars::CONTROL_HEIGHT + CONTROL_SPACING,
                    toolbars::CONTROL_HEIGHT,
                ),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    for (label, tooltip, value) in
                        [("M", "Mute", &mut muted), ("S", "Solo", &mut soloed)]
                    {
                        let response = toolbars::square_toggle(ui, label, *value);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                *value,
                                tooltip,
                            )
                        });
                        if response.on_hover_text(tooltip).clicked() {
                            *value = !*value;
                        }
                    }
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
    }
}

fn mixer_numeric(
    ui: &mut egui::Ui,
    text: &mut String,
    value: &mut f32,
    gain: bool,
    clamp: bool,
) -> Result<(), &'static str> {
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(NUMERIC_WIDTH, toolbars::CONTROL_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(toolbars::CONTROL_HEIGHT);
                numeric_input(
                    ui,
                    text,
                    if gain {
                        "Gain (dB)"
                    } else {
                        "Pan (-1 left, 0 center, +1 right)"
                    },
                )
            },
        )
        .inner;
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
