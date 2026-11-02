//! Browser geometry over shared track moves and beat group insertion.
use super::*;

pub(super) struct Gesture {
    row: Row,
    origin: Pos2,
    focus: egui::Id,
    rect: Rect,
    revision: u64,
    view: ViewMode,
    pub active: bool,
}
impl Gesture {
    pub fn owns(&self, id: Option<Id>) -> bool {
        self.active && id == Some(self.row.id)
    }
    pub fn new(
        row: Row,
        origin: Pos2,
        focus: egui::Id,
        rect: Rect,
        revision: u64,
        view: ViewMode,
    ) -> Self {
        Self {
            row,
            origin,
            focus,
            rect,
            revision,
            view,
            active: false,
        }
    }
}
fn project_target(
    project: &Project,
    selection: &[Id],
    rows: &[(Row, Rect)],
    pointer: Pos2,
) -> Option<(Option<Id>, usize, f32)> {
    let moved = project.moving_track_indices(selection);
    let index_of = |row: &Row| {
        project
            .tracks
            .iter()
            .position(|track| track.id == row.beat.unwrap_or(row.id))
    };
    let bottom = |index: usize| {
        let end = project.track_subtree(project.tracks[index].id).ok()?.end;
        rows.iter()
            .rev()
            .find(|(row, _)| index_of(row).is_some_and(|i| i >= index && i < end))
            .map(|(_, rect)| rect.bottom())
    };
    let adjust = |slot: usize| slot - moved.iter().filter(|&&i| i < slot).count();
    let finish = |parent, index, y| {
        let index = adjust(index);
        project
            .can_move_tracks(selection, parent, index)
            .then_some((parent, index, y))
    };
    for (row, rect) in rows {
        if pointer.y >= rect.bottom() {
            continue;
        }
        let mut anchor = index_of(row)?;
        let track = &project.tracks[anchor];
        if row.beat.is_none()
            && row.kind == TrackKind::Group
            && pointer.y >= rect.top() + rect.height() * 0.25
            && pointer.y <= rect.bottom() - rect.height() * 0.25
        {
            let end = project.track_subtree(track.id).ok()?.end;
            return finish(Some(track.id), end, bottom(anchor)?);
        }
        let after = row.beat.is_some() || pointer.y >= rect.center().y;
        let mut parent = track.group_id;
        let step = icons::TOOLBAR_ICON_SIZE + f32::from(menus::ITEM_PADDING);
        let depth = ((pointer.x
            - rect.left()
            - f32::from(menus::ITEM_PADDING)
            - icons::TOOLBAR_ICON_SIZE * 0.5)
            .max(0.0)
            / step)
            .round() as usize;
        while project.track_depth(project.tracks[anchor].id) > depth {
            let ancestor = parent?;
            anchor = project.tracks.iter().position(|t| t.id == ancestor)?;
            parent = project.tracks[anchor].group_id;
        }
        if after {
            return finish(
                parent,
                project.track_subtree(project.tracks[anchor].id).ok()?.end,
                bottom(anchor)?,
            );
        }
        let top = rows
            .iter()
            .find(|(row, _)| row.id == project.tracks[anchor].id)?
            .1
            .top();
        return finish(parent, anchor, top);
    }
    let bottom = rows.last()?.1.bottom();
    project
        .can_move_tracks(selection, None, project.tracks.len() - moved.len())
        .then_some((None, project.tracks.len() - moved.len(), bottom))
}
impl DawUi {
    pub(super) fn finish_browser_drag(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        rows: &[(Row, Rect)],
        enabled: bool,
    ) {
        let Some(mut gesture) = self.track_browser.gesture.take() else {
            return;
        };
        if crate::drag::cancelled(ui)
            || !self.edit_environment_available()
            || self.change_revision != gesture.revision
            || self.view_mode != gesture.view
            || if gesture.active {
                self.track_drag != Some(gesture.row.id)
            } else {
                self.track_drag.is_some()
            }
            || !enabled
        {
            self.track_drag = None;
            return;
        }
        if !gesture.active && crate::drag::crossed_threshold(ui, gesture.origin) {
            gesture.active = true;
            if let Some(parent) = gesture.row.beat {
                if self.view_mode == ViewMode::Edit {
                    if self.editor_track().is_none_or(|track| track.id != parent) {
                        self.select_editor_track(parent);
                    }
                } else {
                    self.select_track(parent, false);
                }
                self.track_drag = None;
                self.begin_beat_reorder(
                    ui,
                    parent,
                    gesture.row.id,
                    gesture.focus,
                    gesture.rect,
                    crate::beat_editor::BeatDragPane::Tracks,
                );
                return;
            }
            if !self.track_selected(gesture.row.id) {
                self.browser_select(&gesture.row, false);
            }
            self.track_drag = Some(gesture.row.id);
        }
        if !gesture.active {
            if ui.input(|i| i.pointer.primary_down()) {
                self.track_browser.gesture = Some(gesture);
            } else {
                self.track_drag = None;
            }
            return;
        }
        let target = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| viewport.contains(*p))
            .and_then(|pointer| {
                project_target(
                    &self.session.project,
                    &self.track_drag_selection(gesture.row.id),
                    rows,
                    pointer,
                )
            });
        ui.ctx().set_cursor_icon(if target.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NotAllowed
        });
        let track_ids = self.track_drag_selection(gesture.row.id);
        let changed = target
            .filter(|(parent, index, _)| self.track_move_changed(&track_ids, *parent, *index));
        if let Some((_, _, y)) = changed {
            ui.painter_at(viewport).hline(
                viewport.x_range(),
                y.clamp(viewport.top(), viewport.bottom()),
                Stroke::new(2.0, theme::accent(ui.ctx())),
            );
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            self.track_drag = None;
            if let Some((group_id, index, _)) = changed {
                self.edit(Edit::MoveTracks {
                    track_ids,
                    group_id,
                    index,
                });
            }
        } else if ui.input(|i| i.pointer.primary_down()) {
            self.track_browser.gesture = Some(gesture);
            ui.ctx().request_repaint();
        } else {
            self.track_drag = None;
        }
    }
}
