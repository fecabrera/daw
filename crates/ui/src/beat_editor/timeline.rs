//! Saved beat hierarchy. Child lanes never enter clip placement hit geometry.
use super::*;

#[derive(Default)]
pub(crate) struct TimelineState {
    project: Option<Id>,
    pub(crate) rows: HashMap<Id, (Id, Rect, Rect)>,
    #[cfg(test)]
    pub(crate) disclosure: HashMap<Id, Rect>,
}

impl DawUi {
    pub(crate) fn timeline_beat_mix_action(&mut self, action: TrackAction) -> bool {
        let Some(parent) = self.selected_track else {
            return false;
        };
        if self.beat_ui.track != Some(parent)
            || self.beat_ui.output_selected
            || !self.timeline_beat_open(parent)
            || self
                .session
                .project
                .tracks
                .iter()
                .any(|track| track.id != parent && self.track_selected(track.id))
        {
            return false;
        }
        let Some(beat) = self.session.project.beat(parent) else {
            return false;
        };
        let edits: Vec<_> = beat
            .subtracks
            .iter()
            .filter(|row| self.beat_ui.selected.contains(&row.id))
            .map(|row| Edit::Beat {
                track_id: parent,
                edit: BeatEdit::SetMix {
                    id: row.id,
                    gain_db: row.instrument.main.gain_db,
                    pan: row.instrument.main.pan,
                    muted: if action == TrackAction::Mute {
                        !row.muted
                    } else {
                        row.muted
                    },
                    soloed: if action == TrackAction::Solo {
                        !row.soloed
                    } else {
                        row.soloed
                    },
                },
            })
            .collect();
        if edits.is_empty() {
            return false;
        }
        self.edit(Edit::Batch(edits));
        true
    }

    pub(crate) fn timeline_beat_row_at(&self, pointer: Pos2) -> bool {
        self.view_mode == ViewMode::Timeline
            && self
                .timeline_beats
                .rows
                .values()
                .any(|(_, panel, lane)| Rect::from_min_max(panel.min, lane.max).contains(pointer))
    }

    pub(crate) fn timeline_beat_bottom(&self) -> Option<f32> {
        (self.view_mode == ViewMode::Timeline)
            .then(|| {
                self.timeline_beats
                    .rows
                    .values()
                    .map(|(_, _, lane)| lane.bottom())
                    .reduce(f32::max)
            })
            .flatten()
    }

    pub(crate) fn reconcile_timeline_beats(&mut self) {
        if self.timeline_beats.project != Some(self.session.project.project_id) {
            self.timeline_beats = TimelineState {
                project: Some(self.session.project.project_id),
                ..Default::default()
            };
        }
        self.timeline_beats.rows.clear();
        #[cfg(test)]
        self.timeline_beats.disclosure.clear();
    }

    pub(crate) fn timeline_beat_open(&self, id: Id) -> bool {
        self.session.project.tracks.iter().any(|track| {
            track.id == id && track.kind == daw_core::TrackKind::Beat && !track.group_collapsed
        })
    }

    pub(crate) fn timeline_beat_disclosure(&mut self, ui: &mut egui::Ui, id: Id, width: f32) {
        let open = self.timeline_beat_open(id);
        let response = icons::disclosure_button(
            ui,
            open,
            if open {
                "Collapse beat subtracks"
            } else {
                "Expand beat subtracks"
            },
            self.editing_enabled(),
            None,
            width,
        );
        #[cfg(test)]
        self.timeline_beats.disclosure.insert(id, response.rect);
        if response.clicked() {
            self.edit(Edit::SetGroupCollapsed {
                track_id: id,
                collapsed: open,
            });
        }
    }

    pub(crate) fn timeline_beat_child_selected(&self, id: Id) -> bool {
        self.timeline_beats
            .rows
            .get(&id)
            .is_some_and(|(parent, _, _)| {
                self.selected_track == Some(*parent)
                    && self.beat_ui.track == Some(*parent)
                    && self.beat_subtrack_selected(id)
            })
    }

    pub(crate) fn timeline_beat_effect_target(&self, id: Id) -> effects_panel::EffectTarget {
        self.timeline_beats
            .rows
            .get(&id)
            .and_then(|(parent, _, _)| {
                self.session
                    .project
                    .beat(*parent)?
                    .subtracks
                    .iter()
                    .find(|row| row.id == id)
                    .map(|row| effects_panel::EffectTarget::Layer {
                        track: id,
                        layer: row.instrument.main.id,
                    })
            })
            .unwrap_or(effects_panel::EffectTarget::Track(id))
    }

    pub(crate) fn timeline_beat_children(
        &mut self,
        ui: &mut egui::Ui,
        parent: Id,
        following_depth: usize,
    ) -> Vec<(Id, Rect)> {
        if !self.timeline_beat_open(parent) {
            return Vec::new();
        }
        let rows: Vec<_> = self
            .session
            .project
            .beat(parent)
            .map(|beat| beat.subtracks.iter().map(BeatRow::from).collect())
            .unwrap_or_default();
        let height = self.track_row_height();
        let depth = self.session.project.track_depth(parent) + 1;
        let mut panels = Vec::with_capacity(rows.len());
        let count = rows.len();
        for (index, row) in rows.into_iter().enumerate() {
            let bounds = Rect::from_min_size(
                ui.next_widget_position(),
                Vec2::new(ui.available_width(), height),
            );
            let panel = panels::GroupPanel::bounds(bounds, TRACK_WIDTH, depth);
            let lane = Rect::from_min_max(
                Pos2::new(bounds.left() + TRACK_WIDTH, bounds.top()),
                bounds.max,
            );
            self.timeline_beats
                .rows
                .insert(row.id, (parent, panel, lane));
            panels.push((row.id, panel));
            // Preserve scroll and hierarchy geometry while omitting offscreen controls.
            if !bounds.intersects(ui.clip_rect().expand(height)) && !self.beat_renaming(row.id) {
                ui.allocate_space(bounds.size());
                continue;
            }
            let selected = self.timeline_beat_child_selected(row.id);
            let painter = ui.painter_at(lane);
            painter.rect_filled(
                lane,
                0.0,
                if selected {
                    theme::selected_track_background(ui.ctx())
                } else {
                    theme::palette(ui.ctx()).background
                },
            );
            self.paint_grid(&painter, lane);
            let mut header = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("timeline-beat", row.id))
                    .max_rect(Rect::from_min_max(
                        panel.min + Vec2::new(23.0, 7.0),
                        panel.max - Vec2::splat(7.0),
                    ))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            header.set_clip_rect(panel.intersect(ui.clip_rect()));
            // Match the track frame's spacing before reserving the meter column.
            header.spacing_mut().item_spacing = Vec2::new(mix_controls::GROUP_GAP, 4.0);
            ui.painter().rect_filled(
                panel,
                0.0,
                if selected {
                    theme::selected_track_header_background(ui.ctx())
                } else {
                    theme::palette(ui.ctx()).panel
                },
            );
            if let Some(track) = self
                .session
                .project
                .tracks
                .iter()
                .find(|track| track.id == parent)
            {
                panels::GroupPanel::track_color_strip(
                    ui,
                    panel,
                    theme::clip_colors(track.color, None).body,
                );
            }
            let meter = self
                .output
                .as_ref()
                .and_then(|output| output.layer_meter(row.id, row.mix.id));
            let levels = meter.map_or([(0.0, false); 2], |meter| meter.levels());
            let held = meter.map_or([0.0; 2], |meter| meter.held_peaks());
            let (mut gain, mut pan, mut muted, mut soloed) =
                (row.mix.gain_db, row.mix.pan, row.muted, row.soloed);
            let enabled =
                ui.is_enabled() && self.edit_environment_available() && !self.beat_dragging();
            let mut name_response = None;
            let mut name_text = Rect::NOTHING;
            let cleared =
                meters::stereo_panel_held(&mut header, height - 14.0, levels, held, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        let mut layout =
                            track_rows::HeaderRows::track_size(ui, self.timeline_track_size);
                        rows::centered(&mut layout.name, knobs::SIZE, |ui| {
                            rows::left_aligned(ui, toolbars::CONTROL_HEIGHT, |ui| {
                                icons::header_icon_tinted(
                                    ui,
                                    icons::Lucide::AudioLines,
                                    Sense::hover(),
                                    None,
                                    icons::HEADER_ICON_WIDTH,
                                );
                                name_response = Some(
                                    self.beat_rename_input(ui, parent, &row, rows::NAME_FONT_SIZE)
                                        .unwrap_or_else(|| {
                                            let (response, text) = rows::name_button(
                                                ui,
                                                ("beat_name_text", row.id),
                                                &row.name,
                                                rows::NAME_FONT_SIZE,
                                                selected,
                                                Sense::click(),
                                                None,
                                            );
                                            name_text = text.unwrap_or(Rect::NOTHING);
                                            response
                                                .on_hover_text(format!(
                                                    "{}\nDouble-click the name to edit.",
                                                    row.name
                                                ))
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        }),
                                );
                            });
                        });
                        let (knobs, toggles) = mix_controls::track_header_row(
                            &mut layout.mix,
                            (&mut gain, &mut pan),
                            &mut muted,
                            &mut soloed,
                            ["Mute subtrack", "Solo within this beat instrument"],
                            self.track_rows_collapsed(),
                        );
                        #[cfg(test)]
                        self.beat_ui.mix_bounds.insert(
                            row.id,
                            [
                                toggles[0].rect,
                                toggles[1].rect,
                                knobs.bounds[1],
                                knobs.bounds[0],
                            ],
                        );
                        #[cfg(not(test))]
                        let _ = (knobs, toggles);
                    });
                });
            meters::reset_on_click(
                ui,
                self.output
                    .as_ref()
                    .and_then(|output| output.layer_meter(row.id, row.mix.id)),
                cleared,
            );
            if enabled
                && ui.rect_contains_pointer(panel)
                && ui.input(|input| input.focused && input.pointer.primary_pressed())
            {
                self.select_track(parent, false);
                self.navigate_beat_owner(
                    parent,
                    Some(row.id),
                    ui.input(|input| input.modifiers.shift),
                );
            }
            #[cfg(test)]
            if let Some(response) = &name_response {
                self.beat_ui.headers.insert(row.id, response.rect);
            }
            if name_response.is_some_and(|response| {
                response.double_clicked()
                    && response
                        .interact_pointer_pos()
                        .is_some_and(|point| name_text.contains(point))
            }) && enabled
            {
                self.begin_beat_rename(row.id, row.name.clone());
            }
            if (gain, pan, muted, soloed) != (row.mix.gain_db, row.mix.pan, row.muted, row.soloed) {
                self.edit_beat(
                    parent,
                    BeatEdit::SetMix {
                        id: row.id,
                        gain_db: gain,
                        pan,
                        muted,
                        soloed,
                    },
                );
            }
            panels::GroupPanel::row_dividers(
                ui,
                bounds,
                if index + 1 == count {
                    following_depth
                } else {
                    depth
                },
                false,
                true,
            );
            ui.allocate_space(bounds.size());
        }
        panels
    }
}
