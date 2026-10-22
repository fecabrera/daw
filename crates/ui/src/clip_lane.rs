use super::*;

// Shared clip-strip gestures and menu dispatch for Timeline and Editor.
impl DawUi {
    pub(super) fn clip_lane_interaction(
        &mut self,
        ui: &mut egui::Ui,
        track_id: Id,
        clip: &Clip,
        block: Rect,
        lane: Rect,
    ) {
        let edge = ui
            .input(|i| i.pointer.press_origin().or_else(|| i.pointer.latest_pos()))
            .is_some_and(|p| ClipDragMode::at(block, p).resize_edge().is_some());
        let response = ui.interact(
            block.intersect(lane),
            egui::Id::new(clip.id),
            if edge {
                Sense::drag()
            } else {
                Sense::click_and_drag()
            },
        );
        let selected = self.clip_selected(clip.id);
        response.context_menu(|ui| {
            if ui.button("Edit clip").clicked() {
                self.open_clip_editor(clip.id, track_id, ui.ctx());
                ui.close();
            }
            if self.view_mode == ViewMode::Timeline
                && self
                    .session
                    .project
                    .assets
                    .iter()
                    .any(|a| a.id == clip.asset_id && a.midi_pattern.is_none())
            {
                let ready = daw_project::SEPARATION_AVAILABLE
                    && self.file_action_enabled(FileAction::Import)
                    && self.session.folder.is_some()
                    && self.settings.separation_models.cpu.is_some();
                if ui
                    .add_enabled(ready, egui::Button::new("Stem-separation"))
                    .on_disabled_hover_text(if daw_project::SEPARATION_AVAILABLE {
                        "Save the project and select a CPU model in Settings first"
                    } else {
                        daw_project::SEPARATION_UNAVAILABLE_REASON
                    })
                    .clicked()
                {
                    self.separate_clip(clip.id);
                    ui.close();
                }
            }
            if ui
                .button(if clip.muted {
                    "Enable clip"
                } else {
                    "Disable clip"
                })
                .clicked()
            {
                self.edit(Edit::SetClipMuted {
                    clip_id: clip.id,
                    muted: !clip.muted,
                });
                ui.close();
            }
            let targets = if selected {
                self.selected_clip_ids()
            } else {
                vec![clip.id]
            };
            let split_enabled = self.clip_actions_available(ui.ctx())
                && !self.clips_crossed_by_playhead(&targets).is_empty();
            if ui
                .add_enabled(
                    split_enabled,
                    egui::Button::new("Split at playhead").shortcut_text(
                        toolbars::menu_shortcut_text(ui.ctx(), &ClipAction::Split.shortcut()),
                    ),
                )
                .clicked()
            {
                self.split_at(&targets);
                ui.close();
            }
            let group = self.selected_clip_snapshots();
            if ui
                .button(if selected && group.len() > 1 {
                    "Delete selected clips"
                } else {
                    "Delete clip"
                })
                .clicked()
            {
                if selected {
                    self.edit(Edit::Batch(
                        group
                            .into_iter()
                            .map(|entry| Edit::DeleteClip(entry.clip.id))
                            .collect(),
                    ));
                } else {
                    self.edit(Edit::DeleteClip(clip.id));
                }
                ui.close();
            }
            self.midi_clip_note_menu(ui, clip);
        });
        if response.double_clicked() {
            self.open_clip_editor(clip.id, track_id, ui.ctx());
        }
        if response.clicked() {
            self.select_clip(
                clip.id,
                track_id,
                ui.input(|input| input.modifiers.shift || input.modifiers.command),
            );
        }
        if response.hovered()
            && let Some(pos) = response.hover_pos()
        {
            match ClipDragMode::at(block, pos)
                .with_stretch_modifier(ui.input(|input| clip_copy_modifier(input.modifiers)))
            {
                ClipDragMode::StretchLeft | ClipDragMode::StretchRight => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    response.clone().on_hover_text("Drag to stretch selected clips without changing pitch. Hold Shift to bypass snapping.");
                }
                ClipDragMode::LoopLeft | ClipDragMode::LoopRight => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    response
                            .clone()
                            .on_hover_text(if cfg!(target_os = "macos") {
                                "Drag to loop selected clips. Hold Option to stretch. Hold Shift to bypass snapping."
                            } else {
                                "Drag to loop selected clips. Hold Ctrl to stretch. Hold Shift to bypass snapping."
                            });
                }
                ClipDragMode::TrimLeft | ClipDragMode::TrimRight => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    response
                            .clone()
                            .on_hover_text(if self.at_trim_boundary(clip, ClipDragMode::at(block, pos), block, pos) {
                                "Drag the dividing line to trim one clip and extend the other. Hold Shift to bypass snapping."
                            } else if cfg!(target_os = "macos") {
                                "Drag to trim selected clips. Hold Option to stretch. Hold Shift to bypass snapping."
                            } else {
                                "Drag to trim selected clips. Hold Ctrl to stretch. Hold Shift to bypass snapping."
                            });
                }
                ClipDragMode::Move => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    let frame = self.ruler_frame(lane, pos);
                    response.clone().on_hover_text(
                        if self
                            .portion
                            .as_ref()
                            .is_some_and(|portion| portion.contains(track_id, frame))
                        {
                            if cfg!(target_os = "macos") {
                                "Drag to move the selection. Hold Option to duplicate. Hold Shift to bypass snapping."
                            } else {
                                "Drag to move the selection. Hold Ctrl to duplicate. Hold Shift to bypass snapping."
                            }
                        } else if cfg!(target_os = "macos") {
                            "Drag to move clip. Hold Option to duplicate. Hold Shift to bypass snapping."
                        } else {
                            "Drag to move clip. Hold Ctrl to duplicate. Hold Shift to bypass snapping."
                        },
                    );
                }
                ClipDragMode::Body => {}
            }
        }
        let edge_press = midi_editor::primary_press(ui)
            .map(|(pos, _)| pos)
            .filter(|pos| {
                block.intersect(lane).contains(*pos)
                    && ClipDragMode::at(block, *pos).resize_edge().is_some()
            });
        if self.clip_actions_available(ui.ctx())
            && (edge_press.is_some() || response.drag_started())
            && let Some(pos) = edge_press.or_else(|| response.interact_pointer_pos())
        {
            // Capture edges on press, before egui's movement threshold. Both views
            // need the same source snapshot for their first resize preview.
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
            let modifiers = ui.input(|input| input.modifiers);
            let on_edge = ClipDragMode::at(block, origin).resize_edge().is_some();
            // Edge drags keep trim, loop, and stretch. Command-drag, and a drag
            // that starts inside a time range, belongs to that range instead.
            let range_drag = !on_edge || (modifiers.command && !clip_copy_modifier(modifiers));
            if range_drag
                && self.begin_portion_drag(
                    origin,
                    self.ruler_frame(lane, origin),
                    track_id,
                    modifiers,
                )
            {
                return;
            }
            // The Editor lane can claim the range before this clip widget.
            if self.portion_drag.is_some() {
                return;
            }
            let mode = ClipDragMode::at(block, origin)
                .with_stretch_modifier(ui.input(|input| clip_copy_modifier(input.modifiers)));
            if !self.clip_selected(clip.id) {
                let additive = ui.input(|input| {
                    input.modifiers.shift
                        || (input.modifiers.command && !clip_copy_modifier(input.modifiers))
                });
                if additive {
                    self.select_clip(clip.id, track_id, true);
                } else {
                    self.select_clip(clip.id, track_id, false);
                }
            }
            let clips = if matches!(
                mode,
                ClipDragMode::Move
                    | ClipDragMode::TrimLeft
                    | ClipDragMode::TrimRight
                    | ClipDragMode::LoopLeft
                    | ClipDragMode::LoopRight
                    | ClipDragMode::StretchLeft
                    | ClipDragMode::StretchRight
            ) {
                self.selected_clip_snapshots()
            } else {
                Vec::new()
            };
            self.drag = Some(Drag {
                clips,
                clip: clip.clone(),
                track: track_id,
                mode,
                origin,
                linked_boundary: self.at_trim_boundary(clip, mode, block, origin),
                duplicate: mode == ClipDragMode::Move
                    && ui.input(|input| clip_copy_modifier(input.modifiers)),
            });
            self.selected_clip = Some(clip.id);
        }
    }
}
