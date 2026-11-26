//! Shared Mixer strip shell and output controls for tracks, beat lanes and Master.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum StripTarget<'a> {
    Master,
    Track(&'a daw_core::Track),
    Beat {
        parent: &'a daw_core::Track,
        row: &'a daw_core::BeatSubtrack,
    },
}
impl<'a> StripTarget<'a> {
    pub(super) fn id(self) -> Option<Id> {
        match self {
            Self::Master => None,
            Self::Track(t) => Some(t.id),
            Self::Beat { row, .. } => Some(row.id),
        }
    }
    fn track(self) -> Option<&'a daw_core::Track> {
        match self {
            Self::Track(t) => Some(t),
            _ => None,
        }
    }
    fn is_master(self) -> bool {
        matches!(self, Self::Master)
    }
    pub(super) fn selected(self, app: &DawUi) -> bool {
        match self {
            Self::Master => app.master_selected,
            Self::Track(t) => {
                app.track_selected(t.id)
                    && !app
                        .selected_mixer_beat_row()
                        .is_some_and(|(parent, _)| parent == t.id)
            }
            Self::Beat { parent, row } => {
                app.selected_mixer_beat_row()
                    .is_some_and(|(id, _)| id == parent.id)
                    && app.beat_subtrack_selected(row.id)
            }
        }
    }
    fn mix(self, app: &DawUi) -> (f32, f32, bool, bool) {
        match self {
            Self::Master => (app.session.project.master.gain_db, 0.0, false, false),
            Self::Track(t) => {
                let mix = mix_controls::MixValues::track(t);
                (mix.gain, mix.pan, mix.muted, mix.soloed)
            }
            Self::Beat { row, .. } => (
                row.instrument.main.gain_db,
                row.instrument.main.pan,
                row.muted,
                row.soloed,
            ),
        }
    }
    fn meter(self, output: &daw_output::AudioOutput) -> Option<&daw_output::StereoMeter> {
        match self {
            Self::Master => Some(&output.status.meter),
            Self::Track(t) => output.track_meter(t.id),
            Self::Beat { row, .. } => output.layer_meter(row.id, row.instrument.main.id),
        }
    }
}
impl DawUi {
    pub(crate) fn mixer_strip(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        track: Option<&daw_core::Track>,
        left_divider: bool,
    ) -> Option<Rect> {
        self.render_mixer_strip(
            ui,
            rect,
            track.map_or(StripTarget::Master, StripTarget::Track),
            left_divider,
        )
    }
    pub(super) fn beat_mixer_strip(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        parent: &daw_core::Track,
        row: &daw_core::BeatSubtrack,
        left_divider: bool,
    ) {
        self.render_mixer_strip(ui, rect, StripTarget::Beat { parent, row }, left_divider);
    }
    fn render_mixer_strip(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        target: StripTarget<'_>,
        left_divider: bool,
    ) -> Option<Rect> {
        let track = target.track();
        let mut rack_bounds = None;
        let palette = theme::palette(ui.ctx());
        let selected = target.selected(self);
        ui.painter().rect_filled(
            rect,
            0.0,
            if selected {
                theme::selected_track_header_background(ui.ctx())
            } else {
                palette.panel
            },
        );
        let header_fill = match target {
            StripTarget::Master => theme::unit_header_fill(ui.ctx(), false),
            StripTarget::Track(track) | StripTarget::Beat { parent: track, .. } => {
                theme::clip_colors(track.color, None).header
            }
        };
        // The first three ancestor bands mirror Timeline's nesting cap. Paint
        // each ancestor across every descendant so the gutter stays connected
        // when its parent scrolls outside the viewport.
        let mut ancestors = Vec::new();
        let mut parent = match target {
            StripTarget::Beat { parent, .. } => Some(parent.id),
            _ => track.and_then(|track| track.group_id),
        };
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
        if target.is_master()
            && !matches!(
                self.view_mode,
                ViewMode::Timeline | ViewMode::Mixer | ViewMode::Instrument
            )
        {
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
        match target {
            StripTarget::Track(track) => self.track_name_layout(&mut content, track, true),
            StripTarget::Beat { parent, row } => self.beat_mixer_name(&mut content, parent, row),
            StripTarget::Master => self.master_name_layout(&mut content, true),
        }
        if self.midi.busy() {
            content.disable();
        }
        content.spacing_mut().item_spacing.x = CONTROL_SPACING;
        content.spacing_mut().item_spacing.y = OUT_CONTROL_SPACING;
        if !target.is_master() {
            content.add_space(
                (header_top + HEADER_HEIGHT + OUT_CONTROL_SPACING - content.cursor().top())
                    .max(0.0),
            );
            // Match the send rows' inset within the strip's shared content padding.
            let width = (inner.width() - 2.0 * f32::from(menus::ITEM_PADDING)).max(0.0);
            match target {
                StripTarget::Track(track) => {
                    self.mixer_output_destination(&mut content, track, width)
                }
                StripTarget::Beat { parent, .. } => {
                    self.mixer_destination_label(&mut content, &parent.name, width)
                }
                StripTarget::Master => unreachable!(),
            }
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
            if matches!(target, StripTarget::Beat { .. }) {
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
        let (mut gain, mut pan, mut muted, mut soloed) = target.mix(self);
        let previous = mix_controls::MixValues::new(gain, pan, muted, soloed);
        let mut inputs = target.id().map_or_else(
            || Inputs {
                gain: self.master_gain_input.clone(),
                pan: String::new(),
            },
            |id| {
                self.inputs.remove(&id).unwrap_or_else(|| Inputs {
                    gain: faders::format_gain(gain),
                    pan: format!("{pan:.2}"),
                })
            },
        );
        if !target.is_master() {
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
        let levels = self
            .output
            .as_ref()
            .and_then(|output| target.meter(output))
            .map_or([(0.0, false); 2], |meter| meter.levels());
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
                                target.is_master(),
                            ) {
                                self.fail(error);
                            }
                        });
                        ui.spacing_mut().interact_size.y = toolbars::CONTROL_HEIGHT;
                        let fader = faders::gain(ui, &mut gain, fader_height);
                        #[cfg(test)]
                        self.mixer_faders.insert(target.id(), fader.rect);
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
                        let meter = self
                            .output
                            .as_ref()
                            .and_then(|output| target.meter(output))
                            .unwrap_or(&idle_meter);
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
        let mut arm = track.and_then(|track| self.midi_arm(track));
        if !target.is_master() {
            let width = (if arm.is_some() { 3.0 } else { 2.0 }) * toolbars::CONTROL_HEIGHT
                + rows::GROUP_DIVIDER_WIDTH
                + if arm.is_some() {
                    mix_controls::GROUP_GAP
                } else {
                    0.0
                };
            let (rect, _) = content
                .allocate_exact_size(Vec2::new(width, toolbars::CONTROL_HEIGHT), Sense::hover());
            // Retain the R group while the take owns transport.
            let mut controls = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("track-msr")
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            controls.add_enabled_ui(arm.is_some() || !self.midi.busy(), |ui| {
                mix_controls::track_mute_solo(ui, &mut muted, &mut soloed, arm.as_mut())
            });
        } else {
            content.allocate_space(Vec2::new(1.0, toolbars::CONTROL_HEIGHT));
        }
        if let Some(track) = track {
            self.apply_midi_arm(track.id, arm);
            if track.kind == daw_core::TrackKind::Audio
                && content.available_height() >= toolbars::CONTROL_HEIGHT
            {
                self.audio_recording_input(&mut content, track.id);
            }
            if self.midi.busy() {
                gain = previous.gain;
                pan = previous.pan;
                muted = previous.muted;
                soloed = previous.soloed;
            }
            self.inputs.insert(track.id, inputs);
            if previous != mix_controls::MixValues::new(gain, pan, muted, soloed) {
                if let Some(t) = self
                    .session
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track.id)
                {
                    mix_controls::MixValues::new(gain, pan, muted, soloed).apply(t);
                }
                self.changed();
            }
        } else if let StripTarget::Beat { parent, row } = target {
            self.inputs.insert(row.id, inputs);
            if !self.midi.busy()
                && previous != mix_controls::MixValues::new(gain, pan, muted, soloed)
            {
                self.edit_beat(
                    parent.id,
                    crate::mix_controls::MixValues::new(gain, pan, muted, soloed).beat_edit(row.id),
                );
            }
        } else {
            self.master_gain_input = inputs.gain;
            if gain != previous.gain {
                self.session.project.master.gain_db = gain;
                self.changed();
            }
        }
        let meter = self.output.as_ref().and_then(|output| target.meter(output));
        meters::reset_on_click(ui, meter, cleared);
        rack_bounds
    }
}
