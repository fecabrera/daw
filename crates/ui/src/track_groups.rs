//! Group actions and pure drop geometry over the shared track hierarchy transactions.
use super::*;
use daw_core::TrackKind;
impl DawUi {
    /// Extend only group panels to the last visible descendant. Row and lane hit
    /// bounds remain separate, including placeholders above/below the viewport.
    pub(super) fn track_panel_containers(&self, rows: &[(Id, Rect)]) -> Vec<(Id, Rect)> {
        let mut panels = rows.to_vec();
        if self.timeline_beats.rows.is_empty()
            && self
                .session
                .project
                .tracks
                .iter()
                .all(|track| track.group_id.is_none())
        {
            return panels;
        }
        let indices: HashMap<_, _> = rows
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();
        let mut parents: HashMap<_, _> = self
            .session
            .project
            .tracks
            .iter()
            .map(|t| (t.id, t.group_id))
            .collect();
        for track in &self.session.project.tracks {
            if let Some(beat) = &track.beat {
                parents.extend(beat.subtracks.iter().map(|row| (row.id, Some(track.id))));
            }
        }
        for index in (0..panels.len()).rev() {
            if let Some(parent) = parents.get(&panels[index].0).copied().flatten()
                && let Some(&parent_index) = indices.get(&parent)
            {
                panels[parent_index].1.max.y = panels[parent_index]
                    .1
                    .bottom()
                    .max(panels[index].1.bottom());
            }
        }
        panels
    }
    pub(super) fn paint_track_group_edges(&self, ui: &egui::Ui, panels: &[(Id, Rect)], clip: Rect) {
        // The workspace owns the root left edge; adding an inner root stroke
        // would produce two adjacent borders. Only inset rows own a new edge.
        let mut nested: HashSet<_> = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| t.group_id.is_some())
            .map(|t| t.id)
            .collect();
        nested.extend(self.timeline_beats.rows.keys().copied());
        for (id, rect) in panels {
            if nested.contains(id) {
                panels::ControlPanel::row_left_divider(ui, *rect, clip);
            }
        }
    }

    pub(super) fn selected_track_roots(&self) -> Vec<Id> {
        self.session.project.track_selection_roots(
            &self
                .session
                .project
                .tracks
                .iter()
                .filter(|t| self.track_selected(t.id))
                .map(|t| t.id)
                .collect::<Vec<_>>(),
        )
    }
    pub(super) fn selected_group(&self) -> Option<Id> {
        let mut selected = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| self.track_selected(t.id));
        let track = selected.next()?;
        (track.kind == TrackKind::Group && selected.next().is_none()).then_some(track.id)
    }
    pub(super) fn perform_group_action(&mut self, action: TrackAction) {
        if action == TrackAction::Group {
            let group_id = Id::new_v4();
            let number = self
                .session
                .project
                .tracks
                .iter()
                .filter(|t| t.kind == TrackKind::Group)
                .count()
                + 1;
            self.edit(Edit::GroupTracks {
                track_ids: self.selected_track_roots(),
                group_id,
                name: format!("Group {number}"),
            });
            if self.session.project.tracks.iter().any(|t| t.id == group_id) {
                self.select_track(group_id, false);
            }
        } else if let Some(track_id) = self.selected_group() {
            let children: Vec<_> = self
                .session
                .project
                .tracks
                .iter()
                .filter(|t| t.group_id == Some(track_id))
                .map(|t| t.id)
                .collect();
            self.edit(Edit::UngroupTrack { track_id });
            self.selected_tracks = children.iter().copied().collect();
            self.selected_track = children.first().copied();
        }
    }
    /// The middle half of a group row accepts children. At a row edge, the
    /// name gutter chooses the parent depth; the far left moves to Master.
    pub(super) fn track_drag_selection(&self, id: Id) -> Vec<Id> {
        self.session
            .project
            .tracks
            .iter()
            .filter(|t| t.id == id || (self.track_selected(id) && self.track_selected(t.id)))
            .map(|t| t.id)
            .collect()
    }
    pub(super) fn track_move_changed(&self, ids: &[Id], parent: Option<Id>, index: usize) -> bool {
        self.session
            .project
            .track_move_layout(ids, parent, index)
            .is_ok_and(|layout| {
                layout
                    .iter()
                    .enumerate()
                    .any(|(position, &(original, parent))| {
                        position != original
                            || self.session.project.tracks[original].group_id != parent
                    })
            })
    }

    pub(super) fn track_drop_target(
        &self,
        id: Id,
        pointer: Pos2,
    ) -> Option<(Option<Id>, usize, f32)> {
        let project = &self.session.project;
        let selection = self.track_drag_selection(id);
        let moved = project.moving_track_indices(&selection);
        let visible = project.visible_tracks();
        let remaining_index = |slot: usize| slot - moved.iter().filter(|&&i| i < slot).count();
        let subtree_bottom = |end: usize| {
            visible
                .iter()
                .rev()
                .filter(|i| **i < end)
                .find_map(|i| self.lane_bounds.get(&project.tracks[*i].id))
                .map(|rect| {
                    self.timeline_beats
                        .rows
                        .values()
                        .filter(|(parent, _, _)| {
                            visible
                                .iter()
                                .any(|i| *i < end && project.tracks[*i].id == *parent)
                        })
                        .map(|(_, _, lane)| lane.bottom())
                        .fold(rect.bottom(), f32::max)
                })
        };
        for &index in &visible {
            let track = &project.tracks[index];
            let Some(rect) = self.lane_bounds.get(&track.id) else {
                continue;
            };
            if pointer.y >= rect.top() + rect.height() * 0.25
                && pointer.y <= rect.bottom() - rect.height() * 0.25
                && track.kind == TrackKind::Group
            {
                let end = project.track_subtree(track.id).ok()?.end;
                let slot = remaining_index(end);
                return project
                    .can_move_tracks(&selection, Some(track.id), slot)
                    .then_some((Some(track.id), slot, subtree_bottom(end)?));
            }
            if pointer.y < rect.bottom() {
                let after = pointer.y >= rect.center().y;
                let mut anchor = index;
                let mut parent = track.group_id;
                // Pointer position inside the fixed header chooses an ancestor boundary.
                let header_left = rect.left() - TRACK_WIDTH;
                let header_anchor = 7.0 + toolbars::CONTROL_HEIGHT * 0.5;
                let desired = ((pointer.x - header_left - header_anchor).max(0.0)
                    / (2.0 * f32::from(theme::TOOLBAR_PADDING)))
                .round() as usize;
                while project.track_depth(project.tracks[anchor].id) > desired {
                    let ancestor = parent?;
                    anchor = project.tracks.iter().position(|t| t.id == ancestor)?;
                    parent = project.tracks[anchor].group_id;
                }
                let (slot, y) = if after {
                    let end = project.track_subtree(project.tracks[anchor].id).ok()?.end;
                    (end, subtree_bottom(end)?)
                } else {
                    (
                        anchor,
                        self.lane_bounds.get(&project.tracks[anchor].id)?.top(),
                    )
                };
                let slot = remaining_index(slot);
                return project
                    .can_move_tracks(&selection, parent, slot)
                    .then_some((parent, slot, y));
            }
        }
        let y = subtree_bottom(project.tracks.len())?;
        let slot = project.tracks.len() - moved.len();
        project
            .can_move_tracks(&selection, None, slot)
            .then_some((None, slot, y))
    }
}
