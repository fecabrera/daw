use super::*;
use crate::panels::{HEADER_FONT_SIZE, HEADER_HEIGHT};

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
    Effects,
    Edit,
    Mixer,
}

impl ViewMode {
    pub const ALL: [Self; 5] = [
        Self::Timeline,
        Self::Instrument,
        Self::Effects,
        Self::Edit,
        Self::Mixer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Timeline => "Timeline",
            Self::Mixer => "Mixer",
            Self::Instrument => "Instrument",
            Self::Effects => "Effects",
            Self::Edit => "Editor",
        }
    }
}

impl DawUi {
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn view_switch_enabled(&self) -> bool {
        self.settings_enabled()
            && !self.midi_editor.dragging()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, ctx: &egui::Context) {
        if !self.view_switch_enabled() || self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
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
                    egui::Checkbox::new(&mut selected, mode.label()),
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
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(
                            egui::RichText::new(label)
                                .font(egui::TextStyle::Button.resolve(&style)),
                        )
                        .selected(selected)
                        .frame(true)
                        .fill(if selected {
                            selected_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(Stroke::NONE)
                        .corner_radius(style.visuals.widgets.inactive.corner_radius)
                        .min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("Show {label} view"))
                    .clicked()
                {
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

        let tracks_rect = Rect::from_min_max(bounds.min, Pos2::new(master_left, bounds.bottom()));
        let mut tracks_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("mixer_tracks")
                .max_rect(tracks_rect),
        );
        tracks_ui.set_clip_rect(tracks_rect.intersect(ui.clip_rect()));
        let tracks = self.session.project.tracks.clone();
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
                                && self.file_action_enabled(FileAction::Import)
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
                                && self.file_action_enabled(FileAction::Import)
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
            content.add_sized(
                [inner.width(), toolbars::CONTROL_HEIGHT],
                egui::Label::new(egui::RichText::new("Master").size(HEADER_FONT_SIZE)),
            );
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
        let levels: [(f32, bool); 2] = std::array::from_fn(|ch| {
            if let Some(track) = track {
                let meter = self.output.as_ref().and_then(|o| o.track_meter(track.id));
                (
                    meter.map_or(0.0, |m| f32::from_bits(m.peaks[ch].load(Ordering::Relaxed))),
                    meter.is_some_and(|m| m.clipped[ch].load(Ordering::Relaxed)),
                )
            } else {
                (
                    self.output.as_ref().map_or(0.0, |o| {
                        f32::from_bits(o.status.peaks[ch].load(Ordering::Relaxed))
                    }),
                    self.output
                        .as_ref()
                        .is_some_and(|o| o.status.clipped.load(Ordering::Relaxed)),
                )
            }
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
                            let idle_peak = std::sync::atomic::AtomicU32::new(0);
                            let held_peak = self.output.as_ref().map_or(&idle_peak, |output| {
                                track.map_or(&output.status.held_peak, |track| {
                                    output
                                        .track_meter(track.id)
                                        .map_or(&idle_peak, |meter| &meter.held_peak)
                                })
                            });
                            meters::peak_level(ui, held_peak);
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
            if let Some(meter) = self.output.as_ref().and_then(|o| o.track_meter(track.id)) {
                for (ch, clicked) in cleared.into_iter().enumerate() {
                    if clicked {
                        meter.clipped[ch].store(false, Ordering::Relaxed);
                    }
                }
            }
        } else {
            self.master_gain_input = inputs.gain;
            if gain != previous.0 {
                self.session.project.master.gain_db = gain;
                self.changed();
            }
            if cleared.into_iter().any(|clicked| clicked)
                && let Some(output) = &self.output
            {
                output.status.clipped.store(false, Ordering::Relaxed);
            }
        }
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
