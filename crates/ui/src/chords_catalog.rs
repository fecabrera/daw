//! Chord browsing and a release-only timeline placement transaction.
use super::*;
use daw_core::{CHORD_ROOTS, Chord, ChordKind, TrackKind};

macro_rules! labels {
    ($root:literal) => {
        [
            concat!($root, " major"),
            concat!($root, " minor"),
            concat!($root, " diminished"),
            concat!($root, " augmented"),
            concat!($root, " sus2"),
            concat!($root, " sus4"),
            concat!($root, " 6"),
            concat!($root, " minor 6"),
            concat!($root, " 7"),
            concat!($root, " major 7"),
            concat!($root, " minor 7"),
            concat!($root, " half-diminished 7"),
        ]
    };
}
const LABELS: [[&str; 12]; 12] = [
    labels!("C"),
    labels!("C#"),
    labels!("D"),
    labels!("D#"),
    labels!("E"),
    labels!("F"),
    labels!("F#"),
    labels!("G"),
    labels!("G#"),
    labels!("A"),
    labels!("A#"),
    labels!("B"),
];
fn chord(index: usize) -> Chord {
    Chord {
        root: (index / 12) as u8,
        kind: ChordKind::ALL[index % 12],
    }
}
fn selected(types: &[&str]) -> Vec<Chord> {
    LABELS
        .iter()
        .flatten()
        .enumerate()
        .filter(|(_, label)| types.contains(label))
        .map(|(index, _)| chord(index))
        .collect()
}

pub(super) struct Chords {
    pub visible: bool,
    pub catalog: catalog::Catalog,
    root: Option<usize>,
    pub drag: Option<ChordDrag>,
    pub audition: chord_audition::State,
    #[cfg(test)]
    pub bounds: Bounds,
}
impl Default for Chords {
    fn default() -> Self {
        Self {
            visible: false,
            root: None,
            drag: None,
            audition: Default::default(),
            catalog: catalog::Catalog::new(LABELS.iter().flatten().map(|label| (*label, *label)))
                .with_folders(
                    CHORD_ROOTS
                        .iter()
                        .enumerate()
                        .map(|(root, name)| (*name, LABELS[root].to_vec())),
                ),
            #[cfg(test)]
            bounds: Default::default(),
        }
    }
}
pub(super) struct ChordDrag {
    project: Id,
    revision: u64,
    start: catalog::DragStart,
    chords: Vec<Chord>,
    pub active: bool,
    candidate: Option<Candidate>,
}
struct Candidate {
    target: Option<Id>,
    start: u64,
    project: daw_core::Project,
    track: Id,
    clips: Vec<Id>,
}
#[cfg(test)]
pub(super) struct Bounds {
    pub toggle: Rect,
    pub filter: Rect,
    pub entries: Vec<(usize, Rect)>,
    pub folders: Vec<(usize, Rect)>,
    pub preview: Vec<Rect>,
}

#[cfg(test)]
impl Default for Bounds {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            filter: Rect::NOTHING,
            entries: Vec::new(),
            folders: Vec::new(),
            preview: Vec::new(),
        }
    }
}

impl DawUi {
    pub(super) fn chords_catalog_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
    ) {
        let allowed = self.effects_editing_enabled(ui) && self.component_drag.is_none();
        let old_root = self.chords.root;
        self.chords.catalog.filter_folder(old_root);
        let root = &mut self.chords.root;
        let output = self.chords.catalog.show_with_header(
            ui,
            panel,
            bounds,
            ("Chords", "chords"),
            catalog::Interaction {
                rows_enabled: allowed,
                keyboard_enabled: allowed && !ui.input(|i| i.pointer.primary_down()),
                clear_on_empty: allowed,
                empty_enabled: allowed,
            },
            |ui| {
                let response = ui
                    .add_enabled_ui(allowed, |ui| {
                        egui::ComboBox::from_id_salt("chord-root-filter")
                            .selected_text(root.map_or("All notes", |index| CHORD_ROOTS[index]))
                            .width(92.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(root, None, "All notes");
                                for (index, name) in CHORD_ROOTS.iter().enumerate() {
                                    ui.selectable_value(root, Some(index), *name);
                                }
                            })
                            .response
                            .on_hover_text("Filter chords by root note")
                    })
                    .inner;
                #[cfg(test)]
                {
                    self.chords.bounds.filter = response.rect;
                }
                response.rect
            },
        );
        #[cfg(test)]
        {
            self.chords.bounds.entries = output
                .rows
                .iter()
                .map(|(index, r)| (*index, r.rect))
                .collect();
            self.chords.bounds.folders = output
                .folders
                .iter()
                .map(|(index, r)| (*index, r.rect))
                .collect();
        }
        if self.chords.root != old_root {
            self.chords.catalog.filter_folder(self.chords.root);
            ui.ctx().request_repaint();
            return;
        }
        if allowed && let Some(index) = output.selected_entry {
            self.audition_chord(chord(index));
        }
        if allowed && (output.add_selected || output.double_clicked.is_some()) {
            let chords = output.double_clicked.map_or_else(
                || selected(&self.chords.catalog.selected_types()),
                |index| vec![chord(index)],
            );
            let track = self.selected_track.filter(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .any(|t| t.id == *id && t.kind == TrackKind::Midi)
            });
            if let Some(track) = track {
                self.commit_chords(
                    Some(track),
                    self.session.project.transport.playhead_frame,
                    &chords,
                );
            }
        }
        for (index, response) in output.rows {
            if allowed
                && self.chords.drag.is_none()
                && let Some(start) = self.chords.catalog.start_drag(ui, &response, index)
            {
                self.chords.drag = Some(ChordDrag {
                    project: self.session.project.project_id,
                    revision: self.change_revision,
                    chords: selected(&start.types),
                    start,
                    active: false,
                    candidate: None,
                });
            }
        }
    }

    pub(super) fn update_chord_drag(&mut self, ui: &egui::Ui) {
        let invalid = self.chords.drag.as_ref().is_some_and(|d| {
            self.view_mode != ViewMode::Timeline
                || !self.chords.visible
                || drag::cancelled(ui)
                || !self.edit_environment_available()
                || self.session.project.project_id != d.project
                || self.change_revision != d.revision
                || self.drag.is_some()
                || self.track_drag.is_some()
                || self.effects_drag.is_some()
                || self.component_drag.is_some()
        });
        if invalid && let Some(drag) = self.chords.drag.take() {
            drag::restore_focus(ui.ctx(), drag.start.focus);
        }
        if let Some(drag) = &mut self.chords.drag
            && !drag.active
            && drag::crossed_threshold(ui, drag.start.origin)
        {
            drag.active = true;
            self.chords.catalog.select_dragged(drag.start.index);
        }
    }

    fn chord_target(&self, ui: &egui::Ui, count: usize) -> Option<(Option<Id>, u64)> {
        let pointer = ui.input(|i| i.pointer.latest_pos())?;
        if !placement::drop_bounds(self.timeline_bounds).contains(pointer) {
            return None;
        }
        let track = if let Some(track) = self.clip_drop_track(pointer, self.timeline_bounds) {
            if track.kind != TrackKind::Midi {
                return None;
            }
            Some(track.id)
        } else {
            let bottom = self
                .lane_bounds
                .values()
                .map(Rect::bottom)
                .fold(self.timeline_bounds.top(), f32::max);
            if pointer.y < bottom {
                return None;
            }
            None
        };
        let length = self
            .session
            .project
            .midi_frame(4 * daw_core::MIDI_TICKS_PER_BEAT)
            .checked_mul(count as u64)?;
        let over_header = pointer.x < self.timeline_bounds.left();
        let raw = if over_header {
            0
        } else {
            self.session.project.frames(
                (self.scroll + f64::from((pointer.x - self.timeline_bounds.left()) / self.zoom))
                    .max(0.0),
            )
        };
        let start = placement_start(
            &self.session.project,
            i128::from(raw),
            length,
            track,
            None,
            (!over_header && !musical_time::snap_bypassed(ui)).then_some(self.zoom),
        );
        start.checked_add(length)?;
        Some((track, start))
    }

    pub(super) fn finish_chord_drag(&mut self, ui: &egui::Ui) {
        #[cfg(test)]
        {
            self.chords.bounds.preview.clear();
        }
        let Some(mut drag) = self.chords.drag.take() else {
            return;
        };
        let released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        if !drag.active {
            if !released {
                self.chords.drag = Some(drag);
            }
            return;
        }
        let target = self.chord_target(ui, drag.chords.len());
        if let Some((track, start)) = target {
            if drag
                .candidate
                .as_ref()
                .is_none_or(|p| p.target != track || p.start != start)
            {
                let mut project = self.session.project.clone();
                drag.candidate = project
                    .add_midi_chords(track, start, &drag.chords)
                    .ok()
                    .map(|(id, clips)| Candidate {
                        target: track,
                        start,
                        project,
                        track: id,
                        clips,
                    });
            }
            if let Some(candidate) = &drag.candidate {
                let incoming: HashSet<_> = candidate.clips.iter().copied().collect();
                let mut previews = self.placement_neighbors(&candidate.project, &incoming);
                self.mask_neighbor_previews(ui, self.timeline_bounds, &previews);
                for (index, track) in candidate.project.tracks.iter().enumerate() {
                    previews.extend(track.clips.iter().filter(|c| incoming.contains(&c.id)).map(
                        |c| ClipPreview {
                            track: track.id,
                            track_index: index,
                            clip: c.clone(),
                            valid: true,
                            removed: false,
                        },
                    ));
                }
                self.paint_preview_clips_from_project(
                    ui,
                    self.timeline_bounds,
                    &previews,
                    Some(&candidate.project),
                );
                #[cfg(test)]
                {
                    self.chords.bounds.preview = previews
                        .iter()
                        .filter(|p| !p.removed)
                        .filter_map(|p| {
                            self.preview_lane(p)
                                .map(|lane| self.clip_block(lane, &p.clip, p.clip.start_frame))
                        })
                        .collect();
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
            } else {
                ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
            }
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
        }
        if released {
            if target.is_some()
                && let Some(candidate) = drag.candidate
            {
                self.publish_chords(candidate);
            }
        } else if ui.input(|i| i.pointer.primary_down()) {
            self.chords.drag = Some(drag);
            ui.ctx().request_repaint();
        }
    }
    fn commit_chords(&mut self, target: Option<Id>, start: u64, chords: &[Chord]) {
        if chords.is_empty() {
            return;
        }
        let mut project = self.session.project.clone();
        match project.add_midi_chords(target, start, chords) {
            Ok((track, clips)) => self.publish_chords(Candidate {
                target,
                start,
                project,
                track,
                clips,
            }),
            Err(error) => self.fail(error),
        }
    }
    fn publish_chords(&mut self, candidate: Candidate) {
        self.ensure_history();
        if self.prepare_audio_edit(
            &candidate.project,
            Some((candidate.track, candidate.clips.clone())),
        ) {
            return;
        }
        self.session.project = candidate.project;
        self.select_clip(candidate.clips[0], candidate.track, false);
        self.selected_clips.extend(candidate.clips);
        self.changed();
    }
}
