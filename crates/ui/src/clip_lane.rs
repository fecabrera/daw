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
        self.clip_lane_response(ui, track_id, clip, block, lane, response);
    }

    pub(super) fn clip_lane_response(
        &mut self,
        ui: &mut egui::Ui,
        track_id: Id,
        clip: &Clip,
        block: Rect,
        lane: Rect,
        response: egui::Response,
    ) {
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                response.enabled(),
                self.clip_selected(clip.id),
                &clip.name,
            )
        });
        menus::ContextMenu::new(&response).show(|ui| {
            self.clip_context_menu(ui, clip.id);
            self.midi_clip_note_menu(ui, clip);
        });
        let fit_clip = response.double_clicked()
            && (!self.midi_editor.content_active
                || response
                    .interact_pointer_pos()
                    .is_some_and(|pos| pos.y < block.top() + 22.0));
        if fit_clip {
            self.open_clip_editor(clip.id, track_id, ui.ctx());
            if self.midi_editor.content_active {
                self.retain_content_view();
            }
            self.midi_editor.content.fitted = true;
            self.midi_editor.content.retain_on_selection = false;
            ui.ctx().request_repaint();
        } else if response.clicked() {
            self.select_clip(
                clip.id,
                track_id,
                ui.input(|input| input.modifiers.shift || input.modifiers.command),
            );
            if self.view_mode.has_track_overview() && !self.midi_editor.content_active {
                self.retain_content_view();
                self.midi_editor.content.scroll =
                    clip.start_frame as f64 / f64::from(self.session.project.sample_rate_hz);
            }
        }
        if self.midi_editor.content_active
            && !fit_clip
            && (response.clicked() || response.context_menu_opened())
        {
            self.retain_content_view();
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
            if !self.midi_editor.content_active
                && range_drag
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
            // Lower audio waveform bodies retain click selection. Only their
            // headers and edges start clip gestures; note bodies have their own handlers.
            if self.midi_editor.content_active && mode == ClipDragMode::Body {
                return;
            }
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
            self.content_clip_drag = self.midi_editor.content_active;
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
            if self.content_clip_drag {
                self.retain_content_view();
            }
        }
    }
}
