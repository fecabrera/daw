//! Instrument track navigation. Expansion and focus are UI state, never project edits.
use super::*;
use daw_core::{Project, TrackKind};

#[derive(Default)]
pub(super) struct State {
    pub visible: bool,
    open: HashSet<Id>,
    cursor: Option<Id>,
    focus: Option<egui::Id>,
    project: Option<Id>,
    #[cfg(test)]
    pub bounds: Bounds,
}
#[cfg(test)]
pub(super) struct Bounds {
    pub toggle: Rect,
    panel: Rect,
    rows: Vec<(Id, Rect)>,
}
#[cfg(test)]
impl Default for Bounds {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            panel: Rect::NOTHING,
            rows: Vec::new(),
        }
    }
}
struct Row {
    id: Id,
    parent: Option<Id>,
    name: String,
    kind: TrackKind,
    depth: usize,
}
impl State {
    fn reconcile(&mut self, project: &Project) {
        if self.project != Some(project.project_id) {
            self.open.clear();
            self.cursor = None;
            self.focus = None;
            self.project = Some(project.project_id);
        }
        let groups: HashSet<_> = project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Group)
            .map(|t| t.id)
            .collect();
        self.open.retain(|id| groups.contains(id));
    }
    fn rows(&self, project: &Project) -> Vec<Row> {
        // Validated projects store each parent before its contiguous descendants.
        let mut parents = HashMap::new();
        let mut rows = Vec::new();
        for track in &project.tracks {
            let (visible, depth) = track.group_id.map_or((true, 0), |id| {
                parents.get(&id).copied().unwrap_or((false, 0))
            });
            if visible {
                rows.push(Row {
                    id: track.id,
                    parent: track.group_id,
                    name: track.name.clone(),
                    kind: track.kind,
                    depth,
                });
            }
            parents.insert(
                track.id,
                (visible && self.open.contains(&track.id), depth + 1),
            );
        }
        rows
    }
}
impl DawUi {
    pub(super) fn project_tracks_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        enabled: bool,
    ) {
        self.track_browser.reconcile(&self.session.project);
        let rows = self.track_browser.rows(&self.session.project);
        let selected = self.selected_track;
        let header = panel.header(ui, "Tracks", |_| {});
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        let enabled = enabled && !ui.ctx().text_edit_focused();
        let mut select = None;
        if let Some(focus) = self.track_browser.focus {
            crate::selection_focus::retain_list_navigation(ui, focus, true);
        }
        let owns_focus = self
            .track_browser
            .focus
            .is_some_and(|id| ui.ctx().memory(|m| m.focused()) == Some(id));
        let mut moved = false;
        if enabled && owns_focus && !ui.input(|i| i.pointer.any_down()) && !rows.is_empty() {
            let index = self
                .track_browser
                .cursor
                .and_then(|id| rows.iter().position(|r| r.id == id))
                .unwrap_or(0);
            let key = ui.input_mut(|i| {
                [
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::Enter,
                ]
                .into_iter()
                .find(|key| i.consume_key(egui::Modifiers::NONE, *key))
            });
            if let Some(key) = key {
                let row = &rows[index];
                let next = match key {
                    egui::Key::ArrowUp => index.saturating_sub(1),
                    egui::Key::ArrowDown => (index + 1).min(rows.len() - 1),
                    egui::Key::ArrowLeft if !self.track_browser.open.remove(&row.id) => row
                        .parent
                        .and_then(|id| rows.iter().position(|r| r.id == id))
                        .unwrap_or(index),
                    egui::Key::ArrowRight if row.kind == TrackKind::Group => {
                        self.track_browser.open.insert(row.id);
                        index
                    }
                    egui::Key::Enter if row.kind == TrackKind::Group => {
                        if !self.track_browser.open.remove(&row.id) {
                            self.track_browser.open.insert(row.id);
                        }
                        index
                    }
                    egui::Key::Enter => {
                        select = Some(row.id);
                        index
                    }
                    _ => index,
                };
                let row = &rows[next];
                self.track_browser.cursor = Some(row.id);
                if matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown)
                    && row.kind != TrackKind::Group
                {
                    select = Some(row.id);
                }
                moved = true;
            }
        }
        // Rebuild after a disclosure key, so newly hidden children cannot receive input.
        let rows = if moved {
            self.track_browser.rows(&self.session.project)
        } else {
            rows
        };
        #[cfg(test)]
        {
            self.track_browser.bounds.rows.clear();
            self.track_browser.bounds.panel = bounds;
        }
        egui::ScrollArea::vertical()
            .id_salt(("project-tracks", self.session.project.project_id))
            .max_height((bounds.bottom() - header.response.rect.bottom()).max(0.))
            .show(ui, |ui| {
                panel.catalog_body(ui, |ui| {
                    if rows.is_empty() {
                        ui.label("No tracks");
                    }
                    for row in rows {
                        let icon = if row.kind == TrackKind::Midi {
                            icons::Lucide::KeyboardMusic
                        } else {
                            icons::Lucide::AudioLines
                        };
                        let open = (row.kind == TrackKind::Group)
                            .then(|| self.track_browser.open.contains(&row.id));
                        let response = ui
                            .push_id(
                                ("project-track", self.session.project.project_id, row.id),
                                |ui| {
                                    ui.add_enabled_ui(enabled, |ui| {
                                        panel.project_track(
                                            ui,
                                            &row.name,
                                            selected == Some(row.id),
                                            row.depth,
                                            (icon, open),
                                        )
                                    })
                                    .inner
                                },
                            )
                            .inner;
                        #[cfg(test)]
                        self.track_browser.bounds.rows.push((row.id, response.rect));
                        if moved && self.track_browser.cursor == Some(row.id) {
                            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
                            response.request_focus();
                            response.scroll_to_me(None);
                            self.track_browser.focus = Some(response.id);
                            crate::selection_focus::retain_list_navigation(ui, response.id, true);
                        }
                        if response.clicked() && enabled {
                            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
                            response.request_focus();
                            self.track_browser.focus = Some(response.id);
                            crate::selection_focus::retain_list_navigation(ui, response.id, true);
                            self.track_browser.cursor = Some(row.id);
                            if row.kind == TrackKind::Group {
                                if !self.track_browser.open.remove(&row.id) {
                                    self.track_browser.open.insert(row.id);
                                }
                            } else {
                                select = Some(row.id);
                            }
                            ui.ctx().request_repaint();
                        }
                    }
                })
            });
        if let Some(id) = select {
            if self.selected_track != Some(id) {
                self.instrument_ui.cancel();
                self.instrument_ui.selected_layer = None;
                self.instrument_ui.selected_layers.clear();
                self.instrument_ui.effects_layer = None;
                self.instrument_ui.track_effects_target = true;
            }
            self.select_track(id, false);
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests;
