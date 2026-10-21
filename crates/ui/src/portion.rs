//! Command-drag time ranges shared by Timeline and Editor.
//! Moving and duplicating reuse group-move snapping and placement.
use super::{ClipClipboard, ClipDragMode, ClipPreview, DawUi, Drag, SelectedClip};
use daw_core::{Clip, Id};
use egui::{Pos2, Rect, Stroke, StrokeKind};

#[derive(Clone, Debug)]
pub(super) struct ClipPortion {
    pub tracks: Vec<Id>,
    pub start_frame: u64,
    pub end_frame: u64,
}

impl ClipPortion {
    pub(super) fn contains(&self, track: Id, frame: u64) -> bool {
        self.tracks.contains(&track) && self.start_frame <= frame && frame < self.end_frame
    }

    fn shifted(&self, frame_delta: i128, track_delta: i128, project: &daw_core::Project) -> Self {
        let start = (i128::from(self.start_frame) + frame_delta).max(0) as u64;
        let end = (i128::from(self.end_frame) + frame_delta).max(i128::from(start) + 1) as u64;
        let tracks = self
            .tracks
            .iter()
            .filter_map(|id| {
                let index = project.tracks.iter().position(|track| track.id == *id)?;
                let next = index as i128 + track_delta;
                (next >= 0).then_some(next as usize)
            })
            .filter_map(|index| project.tracks.get(index).map(|track| track.id))
            .collect();
        Self {
            tracks,
            start_frame: start,
            end_frame: end,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PortionMode {
    Select,
    Move,
}

pub(super) struct PortionDrag {
    origin: Pos2,
    anchor_track: Id,
    mode: PortionMode,
    selection: ClipPortion,
    slices: Vec<SelectedClip>,
}

impl DawUi {
    pub(super) fn begin_portion_drag(
        &mut self,
        origin: Pos2,
        frame: u64,
        track: Id,
        modifiers: egui::Modifiers,
    ) -> bool {
        if !self.view_mode.has_clip_lanes()
            || !self.editing_enabled()
            || self.portion_drag.is_some()
        {
            return false;
        }
        let inside = self
            .portion
            .as_ref()
            .is_some_and(|portion| portion.contains(track, frame));
        let copy = super::clip_copy_modifier(modifiers);
        // On Windows and Linux, Ctrl is both the command key and the copy modifier.
        // A drag that starts inside the range duplicates or moves it. Ctrl elsewhere
        // starts a new range. On macOS, Command always starts a range.
        let mode = if cfg!(target_os = "macos") {
            if modifiers.command {
                Some(PortionMode::Select)
            } else if inside {
                Some(PortionMode::Move)
            } else {
                None
            }
        } else if inside && copy {
            Some(PortionMode::Move)
        } else if modifiers.command {
            Some(PortionMode::Select)
        } else if inside {
            Some(PortionMode::Move)
        } else {
            None
        };
        let Some(mode) = mode else {
            return false;
        };
        let selection = if mode == PortionMode::Select {
            ClipPortion {
                tracks: vec![track],
                start_frame: frame,
                end_frame: frame,
            }
        } else {
            self.portion.clone().unwrap()
        };
        let slices = if mode == PortionMode::Move {
            self.portion_slices(&selection)
        } else {
            Vec::new()
        };
        self.portion_drag = Some(PortionDrag {
            origin,
            anchor_track: track,
            mode,
            selection,
            slices,
        });
        true
    }

    pub(super) fn update_portion(&mut self, ui: &egui::Ui) {
        if !self.view_mode.has_clip_lanes() {
            return;
        }
        let cancel = !ui.is_enabled()
            || !self.edit_environment_available()
            || !ui.input(|input| input.focused)
            || ui.input(|input| input.key_pressed(egui::Key::Escape));
        if cancel {
            let was_dragging = self.portion_drag.is_some();
            self.portion_drag = None;
            // Timeline's shared Escape handler owns idle range deselection.
            // Keep the existing gesture cancellation and Editor behavior here.
            if (self.view_mode != super::ViewMode::Timeline || was_dragging)
                && ui.input(|input| input.key_pressed(egui::Key::Escape))
                && self.clip_actions_available(ui.ctx())
            {
                self.portion = None;
            }
            return;
        }
        let Some(drag) = self.portion_drag.as_ref() else {
            self.paint_portion(ui, self.portion.clone());
            self.show_range_cursor(ui);
            return;
        };
        let drag = drag.clone_drag();
        let pointer = ui
            .input(|input| input.pointer.latest_pos())
            .unwrap_or(drag.origin);
        let unsnapped = super::musical_time::snap_bypassed(ui);
        if drag.mode == PortionMode::Select {
            // Same crosshair as creating a loop range.
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            let end_frame = self.ruler_frame(self.timeline_bounds, pointer);
            let tracks = self.portion_tracks(drag.origin.y, pointer.y, drag.anchor_track);
            if let Some((start, end)) =
                self.snapped_portion(drag.selection.start_frame, end_frame, unsnapped)
            {
                let live = ClipPortion {
                    tracks,
                    start_frame: start,
                    end_frame: end,
                };
                self.paint_portion(ui, Some(live.clone()));
                if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
                    self.portion = Some(live);
                    self.selected_clip = None;
                    self.selected_clips.clear();
                    self.portion_drag = None;
                }
            } else if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary))
            {
                self.portion_drag = None;
            }
            return;
        }
        let duplicate = super::clip_copy_modifier(ui.input(|input| input.modifiers));
        let Some((frame_delta, track_delta, previews)) =
            self.portion_displacement(&drag, pointer, unsnapped, duplicate)
        else {
            self.portion_drag = None;
            return;
        };
        let live = drag
            .selection
            .shifted(frame_delta, track_delta, &self.session.project);
        self.paint_portion(ui, Some(live));
        if !previews.is_empty() {
            self.paint_preview_clips(ui, self.timeline_bounds, &previews);
            ui.ctx().set_cursor_icon(if duplicate {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            });
        }
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            self.portion_drag = None;
            self.commit_portion(&drag, frame_delta, track_delta, previews, duplicate);
        }
    }

    pub(super) fn delete_portion(&mut self) {
        let Some(portion) = self.portion.clone() else {
            return;
        };
        if self.portion_slices(&portion).is_empty() {
            self.portion = None;
            return;
        }
        let project = match self.session.project.without_clip_range(
            &portion.tracks,
            portion.start_frame,
            portion.end_frame,
        ) {
            Ok(project) => project,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        self.portion = None;
        self.publish_portion_project(project);
    }

    pub(super) fn portion_has_content(&self) -> bool {
        self.portion
            .as_ref()
            .is_some_and(|portion| !self.portion_slices(portion).is_empty())
    }

    pub(super) fn copy_portion(&mut self, ctx: &egui::Context) -> bool {
        let Some(portion) = self.portion.clone() else {
            return false;
        };
        let clips = self.portion_slices(&portion);
        if clips.is_empty() {
            return true;
        }
        let token = format!("DAW clips {}", Id::new_v4());
        ctx.copy_text(token.clone());
        self.clip_clipboard = Some(ClipClipboard {
            project: self.session.project.project_id,
            clips,
            token,
            anchor_frame: Some(portion.start_frame),
        });
        true
    }

    fn portion_slices(&self, portion: &ClipPortion) -> Vec<SelectedClip> {
        let Ok(slices) = self.session.project.clip_range_slices(
            &portion.tracks,
            portion.start_frame,
            portion.end_frame,
        ) else {
            return Vec::new();
        };
        slices
            .into_iter()
            .filter_map(|placement| {
                let track_index = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .position(|track| track.id == placement.track_id)?;
                Some(SelectedClip {
                    track: placement.track_id,
                    track_index,
                    clip: placement.clip,
                })
            })
            .collect()
    }

    fn snapped_portion(&self, start: u64, end: u64, unsnapped: bool) -> Option<(u64, u64)> {
        let first = self.selection_frame(i128::from(start.min(end)), 0..=u64::MAX, unsnapped);
        let second = self.selection_frame(i128::from(start.max(end)), 0..=u64::MAX, unsnapped);
        (first < second).then_some((first, second))
    }

    fn portion_tracks(&self, top: f32, bottom: f32, anchor: Id) -> Vec<Id> {
        let (top, bottom) = (top.min(bottom), top.max(bottom));
        let mut tracks: Vec<_> = self
            .lane_bounds
            .iter()
            .filter(|(_, lane)| lane.bottom() > top && lane.top() < bottom)
            .map(|(id, lane)| (lane.top(), *id))
            .collect();
        tracks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut ids: Vec<_> = tracks.into_iter().map(|(_, id)| id).collect();
        if ids.is_empty() {
            ids.push(anchor);
        }
        ids
    }

    fn portion_displacement(
        &self,
        drag: &PortionDrag,
        pointer: Pos2,
        unsnapped: bool,
        duplicate: bool,
    ) -> Option<(i128, i128, Vec<ClipPreview>)> {
        if drag.slices.is_empty() {
            let raw = self.drag_delta(
                &Drag {
                    clip: Clip {
                        id: Id::nil(),
                        asset_id: Id::nil(),
                        name: String::new(),
                        color: None,
                        start_frame: 0,
                        source_offset_frame: 0,
                        length_frames: 1,
                        repeat: None,
                        stretch: None,
                        gain_db: 0.0,
                        transpose: 0,
                    },
                    track: drag.anchor_track,
                    mode: ClipDragMode::Move,
                    origin: drag.origin,
                    duplicate,
                    linked_boundary: false,
                    clips: Vec::new(),
                },
                pointer,
            ) as i128;
            let length = drag.selection.end_frame - drag.selection.start_frame;
            let start = self.selection_frame(
                i128::from(drag.selection.start_frame) + raw,
                0..=u64::MAX.saturating_sub(length),
                unsnapped || raw == 0,
            );
            let frame_delta = i128::from(start) - i128::from(drag.selection.start_frame);
            return Some((frame_delta, 0, Vec::new()));
        }
        let probe = drag.move_probe(duplicate);
        let previews = self.move_group_preview(&probe, pointer, unsnapped)?;
        let frame_delta =
            i128::from(previews[0].clip.start_frame) - i128::from(drag.slices[0].clip.start_frame);
        let track_delta = previews[0].track_index as i128 - drag.slices[0].track_index as i128;
        Some((frame_delta, track_delta, previews))
    }

    fn commit_portion(
        &mut self,
        drag: &PortionDrag,
        frame_delta: i128,
        track_delta: i128,
        previews: Vec<ClipPreview>,
        duplicate: bool,
    ) {
        if frame_delta == 0 && track_delta == 0 {
            return;
        }
        let (start_frame, end_frame) = shifted_frames(
            drag.selection.start_frame,
            drag.selection.end_frame,
            frame_delta,
        );
        if previews.is_empty() {
            let moved = drag
                .selection
                .shifted(frame_delta, track_delta, &self.session.project);
            if !moved.tracks.is_empty() {
                self.portion = Some(moved);
            }
            return;
        }
        let mut next = if duplicate {
            self.session.project.clone()
        } else {
            match self.session.project.without_clip_range(
                &drag.selection.tracks,
                drag.selection.start_frame,
                drag.selection.end_frame,
            ) {
                Ok(project) => project,
                Err(error) => {
                    self.fail(error);
                    return;
                }
            }
        };
        if let Err(error) = self.prepare_placement_tracks(
            &mut next,
            &previews
                .iter()
                .map(|preview| (preview.track_index, &preview.clip))
                .collect::<Vec<_>>(),
            duplicate,
        ) {
            self.fail(error);
            return;
        }
        let placements = previews
            .iter()
            .map(|preview| daw_core::ClipPlacement {
                track_id: next.tracks[preview.track_index].id,
                clip: preview.clip.clone(),
            })
            .collect::<Vec<_>>();
        let project = match next.placed_clips(&placements) {
            Ok(project) => project,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let tracks = drag
            .selection
            .tracks
            .iter()
            .filter_map(|id| {
                let index = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .position(|track| track.id == *id)?;
                let next_index = index as i128 + track_delta;
                (next_index >= 0).then_some(next_index as usize)
            })
            .filter_map(|index| project.tracks.get(index).map(|track| track.id))
            .collect::<Vec<_>>();
        if tracks.is_empty() {
            self.fail("Track not found");
            return;
        }
        self.portion = Some(ClipPortion {
            tracks,
            start_frame,
            end_frame,
        });
        self.selected_clip = None;
        self.selected_clips.clear();
        self.publish_portion_project(project);
    }

    fn publish_portion_project(&mut self, project: daw_core::Project) {
        if self.prepare_audio_edit(&project, None) {
            return;
        }
        self.session.project = project;
        self.prune_clip_selection();
        self.changed();
    }

    fn show_range_cursor(&self, ui: &egui::Ui) {
        if !self.editing_enabled() {
            return;
        }
        let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) else {
            return;
        };
        let Some(track) = self.session.project.tracks.iter().find(|track| {
            self.lane_bounds
                .get(&track.id)
                .is_some_and(|lane| lane.contains(pointer))
        }) else {
            return;
        };
        let lane = self.lane_bounds[&track.id];
        let modifiers = ui.input(|input| input.modifiers);
        let copy = super::clip_copy_modifier(modifiers);
        let on_edge = track.clips.iter().any(|clip| {
            let block = self.clip_block(lane, clip, clip.start_frame);
            ClipDragMode::at(block, pointer).resize_edge().is_some()
        });
        // Match the gesture: edges keep trim, loop, and stretch unless Command
        // starts a range. The pointer then uses the loop-creation crosshair.
        if on_edge && !(modifiers.command && !copy) {
            return;
        }
        let frame = self.ruler_frame(lane, pointer);
        let inside = self
            .portion
            .as_ref()
            .is_some_and(|portion| portion.contains(track.id, frame));
        let select = if cfg!(target_os = "macos") {
            modifiers.command
        } else {
            modifiers.command && !(inside && copy)
        };
        if select {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    fn paint_portion(&self, ui: &egui::Ui, portion: Option<ClipPortion>) {
        let Some(portion) = portion else {
            return;
        };
        if portion.end_frame <= portion.start_frame {
            return;
        }
        for track in &portion.tracks {
            let Some(lane) = self.lane_bounds.get(track).copied() else {
                continue;
            };
            let left = self.portion_x(lane, portion.start_frame);
            let right = self.portion_x(lane, portion.end_frame);
            let rect =
                Rect::from_min_max(Pos2::new(left, lane.top()), Pos2::new(right, lane.bottom()))
                    .intersect(lane);
            if !rect.is_positive() {
                continue;
            }
            let painter = ui.painter_at(lane);
            painter.rect_filled(
                rect,
                0.0,
                super::theme::accent(ui.ctx()).gamma_multiply(0.18),
            );
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, super::theme::accent(ui.ctx())),
                StrokeKind::Inside,
            );
        }
    }

    fn portion_x(&self, lane: Rect, frame: u64) -> f32 {
        lane.left()
            + ((self.session.project.seconds(frame) - self.view_scroll()) as f32) * self.view_zoom()
    }
}

fn shifted_frames(start: u64, end: u64, delta: i128) -> (u64, u64) {
    let start = i128::from(start)
        .saturating_add(delta)
        .clamp(0, i128::from(u64::MAX)) as u64;
    let end = i128::from(end)
        .saturating_add(delta)
        .clamp(i128::from(start) + 1, i128::from(u64::MAX)) as u64;
    (start, end)
}

impl PortionDrag {
    fn clone_drag(&self) -> Self {
        Self {
            origin: self.origin,
            anchor_track: self.anchor_track,
            mode: self.mode,
            selection: self.selection.clone(),
            slices: self.slices.clone(),
        }
    }

    fn move_probe(&self, duplicate: bool) -> Drag {
        Drag {
            clip: self.slices[0].clip.clone(),
            track: self.anchor_track,
            mode: ClipDragMode::Move,
            origin: self.origin,
            duplicate,
            linked_boundary: false,
            clips: self.slices.clone(),
        }
    }
}
