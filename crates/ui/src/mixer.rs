use super::*;

const STRIP_WIDTH: f32 = 108.0;
const STRIP_MARGIN: f32 = 7.0;
const CONTROL_SPACING: f32 = 6.0;
const HEADER_HEIGHT: f32 = toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const HEADER_FONT_SIZE: f32 = 11.0;

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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ViewMode {
    #[default]
    Timeline,
    Mixer,
}

impl DawUi {
    pub(super) fn view_controls(&mut self, ui: &mut egui::Ui) {
        let enabled = self.settings_enabled()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none);
        ui.scope(|ui| {
            // Menu bars change padding; use the same control style in every titlebar layout.
            let style = ui.ctx().global_style();
            ui.spacing_mut().button_padding = style.spacing.button_padding;
            let selected_fill = style.visuals.selection.bg_fill;
            let bright_text = theme::contrast_color(selected_fill);
            let secondary_text = theme::palette(ui.ctx()).secondary;
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
            for (mode, label) in [(ViewMode::Timeline, "Timeline"), (ViewMode::Mixer, "Mixer")] {
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
                    && self.view_mode != mode
                {
                    self.view_mode = mode;
                    self.track_name_edit = None;
                    self.tempo_edit = None;
                    self.inputs.clear();
                    self.master_gain_input = format!("{:.1}", self.session.project.master.gain_db);
                    if let Some(id) = ui.ctx().memory(|memory| memory.focused()) {
                        ui.ctx().memory_mut(|memory| memory.surrender_focus(id));
                    }
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
        let inner = rect.shrink(STRIP_MARGIN);
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
                    gain: format!("{gain:.1}"),
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
        if let Err(error) = mixer_numeric(
            &mut content,
            &mut inputs.gain,
            &mut gain,
            true,
            track.is_none(),
        ) {
            self.fail(error);
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
        content.allocate_ui_with_layout(
            Vec2::new(50.0, fader_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let fader = faders::gain(ui, &mut gain, fader_height);
                #[cfg(test)]
                self.mixer_faders.insert(track.map(|t| t.id), fader.rect);
                if fader.changed() {
                    inputs.gain = format!("{gain:.1}");
                }
                for ch in 0..2 {
                    cleared[ch] = meters::channel(
                        ui,
                        if ch == 0 { "L" } else { "R" },
                        fader_height,
                        levels[ch].0,
                        levels[ch].1,
                    )
                    .clicked();
                }
            },
        );
        if track.is_some() {
            content.allocate_ui_with_layout(
                Vec2::new(62.0, toolbars::CONTROL_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    for (label, tooltip, value) in
                        [("M", "Mute", &mut muted), ("S", "Solo", &mut soloed)]
                    {
                        let response = ui.add(
                            egui::Button::new(label)
                                .selected(*value)
                                .min_size(Vec2::new(28.0, toolbars::CONTROL_HEIGHT)),
                        );
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
            Vec2::new(36.0, toolbars::CONTROL_HEIGHT),
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
            format!("{v:.1}")
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
        let parsed = text.parse::<f32>().ok().filter(|v| {
            v.is_finite()
                && if gain {
                    daw_core::linear_gain(*v).is_finite()
                } else {
                    (-1.0..=1.0).contains(v)
                }
        });
        if let Some(parsed) = parsed {
            *value = if gain && clamp {
                parsed.clamp(-60.0, 12.0)
            } else {
                parsed
            };
            *text = format(*value);
        } else {
            *text = format(*value);
            return Err(if gain {
                "Gain must be a finite number"
            } else {
                "Pan must be between -1 and 1"
            });
        }
    }
    Ok(())
}
