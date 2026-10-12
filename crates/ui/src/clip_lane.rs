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
            if ui.button("Split at playhead").clicked() {
                self.edit(Edit::Split {
                    clip_id: clip.id,
                    at: self.session.project.transport.playhead_frame,
                });
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
                    response
                            .clone()
                            .on_hover_text(if cfg!(target_os = "macos") {
                                "Drag to move clip. Hold Option to duplicate. Hold Shift to bypass snapping."
                            } else {
                                "Drag to move clip. Hold Ctrl to duplicate. Hold Shift to bypass snapping."
                            });
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
