use super::*;
use daw_core::{MIDI_TICKS_PER_BEAT, MidiNote, MidiPattern, TrackKind};

use crate::track_overview::LABEL_WIDTH as KEY_WIDTH;
const NOTE_HEIGHT: f32 = 18.0;
pub(super) const UNIT_STRIP_WIDTH: f32 = 90.0;
pub(super) const OSCILLATOR_STRIP_WIDTH: f32 = UNIT_STRIP_WIDTH;
#[derive(Clone, Copy)]
enum NoteDragMode {
    Move,
    Start,
    End,
}
struct DraggedNote {
    original: MidiNote,
    copy_id: Id,
}
struct NoteDragPattern {
    clip: Id,
    original: MidiPattern,
    preview: MidiPattern,
    notes: Vec<DraggedNote>,
    beat_width: f32,
}
struct NoteDrag {
    anchor: (Id, Id),
    patterns: Vec<NoteDragPattern>,
    mode: NoteDragMode,
    origin: Pos2,
    anchor_frame: i128,
    previous_selection: HashSet<(Id, Id)>,
    previous_note: Option<Id>,
}
struct NoteDraw {
    clip: Id,
    origin: Pos2,
    start_tick: u32,
    pitch: u8,
    note: Id,
    pattern: MidiPattern,
    original: MidiPattern,
    grid_left: f32,
    beat_width: f32,
    step: u32,
    min_tick: u32,
    max_tick: u32,
}
struct NoteSelection {
    origin: Pos2,
    previous: HashSet<(Id, Id)>,
    previous_note: Option<Id>,
    toggle: bool,
}
pub(super) struct MidiEditor {
    pub(super) content: crate::editor_viewport::EditorViewport,
    pub(super) content_active: bool,
    #[cfg(test)]
    pub(crate) panel_bounds: Rect,
    #[cfg(test)]
    pub(crate) selector_bounds: Rect,
    #[cfg(test)]
    pub(crate) scale_snap_bounds: Rect,
    #[cfg(test)]
    pub(crate) scale_root_bounds: Rect,
    #[cfg(test)]
    pub(crate) scale_mode_bounds: Rect,
    #[cfg(test)]
    pub(crate) mix_bounds: [Rect; 2],
    #[cfg(test)]
    pub(crate) layer_bounds: HashMap<Id, crate::instrument_panel::LayerUiBounds>,
    #[cfg(test)]
    pub(crate) add_layer_bounds: Rect,
    #[cfg(test)]
    pub(crate) instrument_header_bounds: Rect,
    selected_note: Option<Id>,
    selected_notes: HashSet<(Id, Id)>,
    selection: Option<NoteSelection>,
    note_regions: Vec<((Id, Id), Rect)>,
    drag: Option<NoteDrag>,
    draw: Option<NoteDraw>,
    pub(super) zoom: f32,
    pub(super) scroll: f64,
    pub(super) source_origin: f64,
    pub(super) active_clip: Option<Id>,
    pub(super) active_track: Option<Id>,
    #[cfg(test)]
    pub(crate) track_bounds: Rect,
    #[cfg(test)]
    pub(crate) clip_bounds: HashMap<Id, Rect>,
    #[cfg(test)]
    pub(crate) track_note_bounds: HashMap<(Id, Id), Vec<Rect>>,
    #[cfg(test)]
    pub(crate) grid_bounds: Rect,
    #[cfg(test)]
    pub(crate) ruler_bounds: Rect,
    #[cfg(test)]
    pub(crate) zoom_bounds: Rect,
    #[cfg(test)]
    pub(crate) pattern_bounds: Rect,
    #[cfg(test)]
    pub(crate) pattern_header_bounds: HashMap<Id, Rect>,
    #[cfg(test)]
    pub(crate) note_bounds: HashMap<Id, Rect>,
    #[cfg(test)]
    pub(crate) oscillator_bounds: Vec<Rect>,
    #[cfg(test)]
    pub(crate) oscillator_strip_bounds: Vec<Rect>,
    #[cfg(test)]
    pub(crate) oscillator_power_bounds: Vec<Rect>,
    #[cfg(test)]
    pub(crate) add_oscillator_bounds: Rect,
    #[cfg(test)]
    pub(crate) add_oscillator_strip_bounds: Rect,
    #[cfg(test)]
    pub(crate) oscillator_panel_bounds: Rect,
    #[cfg(test)]
    pub(crate) envelope_panel_bounds: Rect,
    #[cfg(test)]
    pub(crate) envelope_stage_bounds: [Rect; 4],
}
impl Default for MidiEditor {
    fn default() -> Self {
        Self {
            content: Default::default(),
            content_active: false,
            #[cfg(test)]
            panel_bounds: Rect::NOTHING,
            #[cfg(test)]
            selector_bounds: Rect::NOTHING,
            #[cfg(test)]
            scale_snap_bounds: Rect::NOTHING,
            #[cfg(test)]
            scale_root_bounds: Rect::NOTHING,
            #[cfg(test)]
            scale_mode_bounds: Rect::NOTHING,
            #[cfg(test)]
            mix_bounds: [Rect::NOTHING; 2],
            #[cfg(test)]
            layer_bounds: HashMap::new(),
            #[cfg(test)]
            add_layer_bounds: Rect::NOTHING,
            #[cfg(test)]
            instrument_header_bounds: Rect::NOTHING,
            selected_note: None,
            selected_notes: HashSet::new(),
            selection: None,
            note_regions: Vec::new(),
            drag: None,
            draw: None,
            zoom: 240.0,
            scroll: 0.0,
            source_origin: 0.0,
            active_clip: None,
            active_track: None,
            #[cfg(test)]
            track_bounds: Rect::NOTHING,
            #[cfg(test)]
            clip_bounds: HashMap::new(),
            #[cfg(test)]
            track_note_bounds: HashMap::new(),
            #[cfg(test)]
            grid_bounds: Rect::NOTHING,
            #[cfg(test)]
            ruler_bounds: Rect::NOTHING,
            #[cfg(test)]
            zoom_bounds: Rect::NOTHING,
            #[cfg(test)]
            pattern_bounds: Rect::NOTHING,
            #[cfg(test)]
            pattern_header_bounds: HashMap::new(),
            #[cfg(test)]
            note_bounds: HashMap::new(),
            #[cfg(test)]
            oscillator_bounds: Vec::new(),
            #[cfg(test)]
            oscillator_strip_bounds: Vec::new(),
            #[cfg(test)]
            oscillator_power_bounds: Vec::new(),
            #[cfg(test)]
            add_oscillator_bounds: Rect::NOTHING,
            #[cfg(test)]
            add_oscillator_strip_bounds: Rect::NOTHING,
            #[cfg(test)]
            oscillator_panel_bounds: Rect::NOTHING,
            #[cfg(test)]
            envelope_panel_bounds: Rect::NOTHING,
            #[cfg(test)]
            envelope_stage_bounds: [Rect::NOTHING; 4],
        }
    }
}
impl MidiEditor {
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.draw.is_some() || self.selection.is_some()
    }
    pub fn cancel(&mut self) {
        self.drag = None;
        self.draw = None;
        self.selection = None;
        self.clear_note_selection();
    }
    #[cfg(test)]
    pub(crate) fn selected_notes(&self) -> &HashSet<(Id, Id)> {
        &self.selected_notes
    }
    #[cfg(test)]
    pub(crate) fn selecting(&self) -> bool {
        self.selection.is_some()
    }
    pub(super) fn clear_note_selection(&mut self) {
        self.selected_note = None;
        self.selected_notes.clear();
    }
    fn select_note(&mut self, clip: Id, note: Id) {
        self.selected_note = Some(note);
        self.selected_notes = HashSet::from([(clip, note)]);
    }
}

pub(super) fn primary_press(ui: &egui::Ui) -> Option<(Pos2, egui::Modifiers)> {
    ui.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } => Some((*pos, *modifiers)),
            _ => None,
        })
    })
}

fn note_rect(grid: Rect, note: &MidiNote, beat_width: f32) -> Rect {
    let scale = beat_width / MIDI_TICKS_PER_BEAT as f32;
    Rect::from_min_size(
        Pos2::new(
            grid.left() + note.start_tick as f32 * scale,
            grid.top() + f32::from(127 - note.pitch) * NOTE_HEIGHT,
        ),
        Vec2::new(
            (note.length_ticks as f32 * scale).max(2.0),
            NOTE_HEIGHT - 1.0,
        ),
    )
}
use daw_unit_core::note_name;
fn black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

impl DawUi {
    pub(super) fn add_midi_pattern(&mut self, track: Id, start: u64, ctx: &egui::Context) {
        if !self.view_mode.has_clip_lanes() || !self.clip_actions_available(ctx) {
            return;
        }
        match self.session.project.add_midi_pattern(track, start) {
            Ok(id) => {
                self.select_clip(id, track, false);
                self.changed();
            }
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn create_midi_pattern_in_empty_lane(
        &mut self,
        ui: &egui::Ui,
        track: &daw_core::Track,
        lane: Rect,
    ) {
        if track.kind != TrackKind::Midi
            || !ui.is_enabled()
            || !self.clip_actions_available(ui.ctx())
            || !ui.rect_contains_pointer(lane)
            || !ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
            })
        {
            return;
        }
        let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        if track.clips.iter().any(|clip| {
            let block = if self.view_mode.has_track_overview() {
                self.track_overview_clip_rect(lane, clip)
            } else {
                self.clip_block(lane, clip, clip.start_frame)
            };
            block.contains(pointer)
        }) {
            return;
        }
        let start = self.selection_frame(
            i128::from(self.ruler_frame(lane, pointer)),
            0..=u64::MAX,
            musical_time::snap_bypassed(ui),
        );
        self.add_midi_pattern(track.id, start, ui.ctx());
    }
    pub(super) fn open_clip_editor(&mut self, clip: Id, track: Id, ctx: &egui::Context) {
        self.select_clip(clip, track, false);
        self.set_view_mode(ViewMode::Edit, ctx);
    }
    pub(super) fn edit_view(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.available_rect_before_wrap();
        self.midi_editor_panel(ui, bounds);
        let canvas = Rect::from_min_max(bounds.min + Vec2::new(TRACK_WIDTH, 0.0), bounds.max);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("editor-canvas")
                .max_rect(canvas)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(canvas.intersect(ui.clip_rect()));
        self.edit_canvas(&mut child);
        ui.advance_cursor_after_rect(bounds);
    }

    fn edit_canvas(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        if !ui.is_enabled()
            || !ui.input(|i| i.focused)
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            if let Some(selection) = self.midi_editor.selection.take() {
                self.midi_editor.selected_notes = selection.previous;
                self.midi_editor.selected_note = selection.previous_note;
            }
            if let Some(drag) = self.midi_editor.drag.take() {
                self.midi_editor.selected_notes = drag.previous_selection;
                self.midi_editor.selected_note = drag.previous_note;
            }
            if self.midi_editor.draw.take().is_some() {
                self.midi_editor.clear_note_selection();
            }
        }
        self.midi_editor.content_active = true;
        self.update_midi_notes(ui);
        self.midi_editor.content_active = false;
        let previous_track = self.midi_editor.active_track;
        let (track, assets) = self.track_overview_content(ui);
        self.prepare_track_overview(track.as_ref());
        let viewport = Rect::from_min_max(
            Pos2::new(ui.max_rect().left() + KEY_WIDTH, ui.max_rect().top()),
            Pos2::new(
                ui.max_rect().right(),
                ui.max_rect().top() + RULER_HEIGHT + ROW_HEIGHT,
            ),
        );
        self.apply_fit_all(viewport.width());
        self.timeline_pinch(ui, viewport);
        self.timeline_swipe(ui, viewport);
        self.track_overview_ruler(ui);
        let Some(track) = track else {
            return;
        };
        self.track_overview_strip(ui, &track, &assets);
        self.finish_clip_drag(ui);
        self.update_portion(ui);
        self.editor_scrollbar(ui, ui.next_widget_position().y, "editor_overview_scrollbar");
        // Re-resolve after strip gestures and keep the content transform scoped to
        // the lower pane. Menus and overview gestures retain the overview clock.
        let (track, assets) = self.track_overview_content(ui);
        let Some(track) = track else {
            return;
        };
        self.midi_editor.content_active = true;
        self.editor_content_header(ui, &track);
        if track.kind == TrackKind::Midi {
            self.piano_roll(ui, &track, &assets, previous_track);
        } else {
            self.audio_editor(ui);
        }
        self.midi_editor.content_active = false;
    }

    fn piano_roll(
        &mut self,
        ui: &mut egui::Ui,
        track: &daw_core::Track,
        assets: &[daw_core::Asset],
        previous_track: Option<Id>,
    ) {
        if self.portion.is_none()
            && !self.midi_editor.dragging()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.delete_selected_midi_notes();
        }
        let preview = self.editor_clip_preview(ui);
        let track = preview
            .as_ref()
            .and_then(|p| p.tracks.iter().find(|t| t.id == track.id))
            .unwrap_or(track);
        let assets = preview.as_ref().map_or(assets, |p| p.assets.as_slice());
        self.midi_pattern_headers(ui, track);
        let mut scroll = egui::ScrollArea::vertical().id_salt(("piano-roll", track.id));
        if previous_track != Some(track.id) {
            scroll = scroll.vertical_scroll_offset(55.0 * NOTE_HEIGHT);
        }
        scroll
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .max_height(ui.available_height().max(0.0))
            .show(ui, |ui| {
                let (canvas, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 128.0 * NOTE_HEIGHT),
                    Sense::hover(),
                );
                let grid_left = canvas.left() + KEY_WIDTH;
                self.midi_editor.content.left = grid_left;
                #[cfg(test)]
                {
                    self.midi_editor.content.bounds =
                        Rect::from_min_max(Pos2::new(grid_left, canvas.top()), canvas.max)
                            .intersect(ui.clip_rect());
                }
                let note_viewport =
                    Rect::from_min_max(Pos2::new(grid_left, canvas.top()), canvas.max)
                        .intersect(ui.clip_rect());
                self.midi_editor.note_regions.clear();
                #[cfg(test)]
                {
                    self.midi_editor.note_bounds.clear();
                    self.midi_editor.track_note_bounds.clear();
                }
                let p = ui.painter_at(canvas);
                let palette = theme::palette(ui.ctx());
                for pitch in 0..=127u8 {
                    let y = canvas.top() + f32::from(127 - pitch) * NOTE_HEIGHT;
                    let row = Rect::from_min_size(
                        Pos2::new(canvas.left(), y),
                        Vec2::new(canvas.width(), NOTE_HEIGHT),
                    );
                    if row.intersects(ui.clip_rect()) {
                        p.rect_filled(
                            row,
                            0.0,
                            if black_key(pitch) || !track.midi_options.contains_pitch(pitch) {
                                palette.background
                            } else {
                                palette.panel
                            },
                        );
                        p.line_segment(
                            [Pos2::new(grid_left, y), Pos2::new(canvas.right(), y)],
                            Stroke::new(1.0, palette.divider),
                        );
                        p.text(
                            Pos2::new(canvas.left() + 4.0, y + NOTE_HEIGHT / 2.0),
                            egui::Align2::LEFT_CENTER,
                            note_name(pitch),
                            egui::FontId::proportional(10.0),
                            if track.midi_options.contains_pitch(pitch) {
                                palette.text
                            } else {
                                palette.secondary
                            },
                        );
                    }
                }
                let grid = Rect::from_min_max(Pos2::new(grid_left, canvas.top()), canvas.max);
                self.paint_grid(&ui.painter_at(note_viewport), grid);
                for clip in &track.clips {
                    let Some(pattern) = assets
                        .iter()
                        .find(|a| a.id == clip.asset_id)
                        .and_then(|a| a.midi_pattern.as_ref())
                    else {
                        continue;
                    };
                    self.editor_clip_notes(ui, canvas, note_viewport, track, clip, pattern);
                }
                self.midi_pattern_edges(ui, track, note_viewport);
                self.update_midi_notes(ui);
                self.editor_note_selection(ui, note_viewport);
                self.midi_pattern_outlines(&ui.painter_at(note_viewport), track, note_viewport);
                let x = grid_left
                    + (self
                        .session
                        .project
                        .seconds(self.session.project.transport.playhead_frame)
                        - self.view_scroll()) as f32
                        * self.view_zoom();
                if x >= grid_left && x <= canvas.right() {
                    p.line_segment(
                        [Pos2::new(x, canvas.top()), Pos2::new(x, canvas.bottom())],
                        Stroke::new(1.0, theme::accent(ui.ctx())),
                    );
                }
            });
        self.editor_scrollbar(ui, ui.max_rect().bottom(), "editor_content_scrollbar");
        if self.drag.is_some() && self.content_clip_drag {
            self.finish_clip_drag(ui);
            self.retain_content_view();
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(draw) = self.midi_editor.draw.take()
        {
            self.edit(Edit::ReplaceMidiPattern {
                clip_id: draw.clip,
                pattern: draw.pattern,
            });
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(drag) = self.midi_editor.drag.take()
        {
            let commands: Vec<_> = drag
                .patterns
                .into_iter()
                .filter(|p| p.preview != p.original)
                .map(|p| Edit::ReplaceMidiPattern {
                    clip_id: p.clip,
                    pattern: p.preview,
                })
                .collect();
            if !commands.is_empty() {
                self.edit(Edit::Batch(commands));
            }
        }
    }
    fn editor_note_selection(&mut self, ui: &egui::Ui, viewport: Rect) {
        if !self.midi_editor.dragging()
            && self.drag.is_none()
            && ui.is_enabled()
            && ui.input(|i| i.focused)
            && !egui::Popup::is_any_open(ui.ctx())
            && let Some((origin, modifiers)) = primary_press(ui)
            && !modifiers.command
            && viewport.contains(origin)
            && !self
                .midi_editor
                .note_regions
                .iter()
                .any(|(_, rect)| rect.contains(origin))
        {
            self.midi_editor.selection = Some(NoteSelection {
                origin,
                previous: self.midi_editor.selected_notes.clone(),
                previous_note: self.midi_editor.selected_note,
                toggle: modifiers.shift,
            });
        }
        if let Some(selection) = &self.midi_editor.selection
            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        {
            let rect = Rect::from_two_pos(selection.origin, viewport.clamp(pointer));
            // Deduplicate repeats before toggling against the selection at the initial press.
            let hits: HashSet<_> = self
                .midi_editor
                .note_regions
                .iter()
                .filter(|(_, bounds)| rect.is_positive() && bounds.intersects(rect))
                .map(|(note, _)| *note)
                .collect();
            self.midi_editor.selected_notes = if selection.toggle {
                selection
                    .previous
                    .symmetric_difference(&hits)
                    .copied()
                    .collect()
            } else {
                hits
            };
            self.midi_editor.selected_note = if selection.toggle {
                selection.previous_note.filter(|note| {
                    self.selected_clip.is_some_and(|clip| {
                        self.midi_editor.selected_notes.contains(&(clip, *note))
                    })
                })
            } else {
                None
            };
            let accent = theme::accent(ui.ctx());
            ui.painter_at(viewport)
                .rect_filled(rect, 0.0, accent.linear_multiply(0.15));
            ui.painter_at(viewport).rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, accent),
                StrokeKind::Inside,
            );
            ui.ctx().request_repaint();
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            self.midi_editor.selection = None;
        }
    }
    fn delete_selected_midi_notes(&mut self) {
        let selected = &self.midi_editor.selected_notes;
        let commands: Vec<_> = self
            .session
            .project
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .filter_map(|clip| {
                let pattern = self
                    .session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)?
                    .midi_pattern
                    .as_ref()?;
                let mut next = pattern.clone();
                next.notes
                    .retain(|note| !selected.contains(&(clip.id, note.id)));
                (next != *pattern).then_some(Edit::ReplaceMidiPattern {
                    clip_id: clip.id,
                    pattern: next,
                })
            })
            .collect();
        if !commands.is_empty() {
            self.edit(Edit::Batch(commands));
            self.midi_editor.clear_note_selection();
        }
    }
    pub(super) fn editor_select_clip(&mut self, clip: Id, track: Id) {
        if self.selected_clip != Some(clip) {
            self.midi_editor.clear_note_selection();
        }
        self.select_clip(clip, track, false);
        self.midi_editor.active_clip = Some(clip);
        self.midi_editor.content.retain_on_selection = true;
    }
    pub(super) fn editor_pattern<'a>(
        &'a self,
        clip: Id,
        original: &'a MidiPattern,
    ) -> &'a MidiPattern {
        self.midi_editor
            .draw
            .as_ref()
            .filter(|d| d.clip == clip)
            .map(|d| &d.pattern)
            .or_else(|| {
                self.midi_editor
                    .drag
                    .as_ref()
                    .and_then(|d| d.patterns.iter().find(|p| p.clip == clip))
                    .map(|p| &p.preview)
            })
            .unwrap_or(original)
    }
    fn editor_beat_width(&self, clip: &Clip) -> f32 {
        self.midi_editor.content.zoom * 60.0 / self.session.project.tempo_bpm
            * clip.stretch.map_or(1.0, |s| s.ratio() as f32)
    }
    fn editor_step(&self, clip: &Clip) -> u32 {
        (self
            .midi_editor
            .content
            .timeline(&self.session.project)
            .visible_step()
            / clip.stretch.map_or(1.0, |s| s.ratio())
            * f64::from(MIDI_TICKS_PER_BEAT))
        .round()
        .max(1.0) as u32
    }

    pub(super) fn midi_clip_note_menu(&mut self, ui: &mut egui::Ui, clip: &Clip) {
        if Some(clip.id) != self.selected_clip {
            return;
        }
        let Some(pattern) = self
            .session
            .project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .and_then(|a| a.midi_pattern.as_ref())
            .cloned()
        else {
            return;
        };
        let Some(note) = pattern
            .notes
            .iter()
            .find(|n| Some(n.id) == self.midi_editor.selected_note)
        else {
            return;
        };
        ui.separator();
        ui.label("Velocity");
        let mut velocity = note.velocity;
        if ui
            .add_enabled(
                !self.midi_editor.dragging(),
                egui::DragValue::new(&mut velocity).range(1..=127),
            )
            .changed()
        {
            let mut next = pattern.clone();
            next.notes
                .iter_mut()
                .find(|n| n.id == note.id)
                .unwrap()
                .velocity = velocity;
            self.edit(Edit::ReplaceMidiPattern {
                clip_id: clip.id,
                pattern: next,
            });
        }
        if ui
            .add_enabled(
                !self.midi_editor.dragging(),
                egui::Button::new("Delete note"),
            )
            .clicked()
        {
            self.delete_midi_note(clip.id, &pattern);
            ui.close();
        }
    }
    fn editor_clip_notes(
        &mut self,
        ui: &mut egui::Ui,
        canvas: Rect,
        viewport: Rect,
        track: &daw_core::Track,
        clip: &Clip,
        pattern: &MidiPattern,
    ) {
        let lane = Rect::from_min_max(
            Pos2::new(canvas.left() + KEY_WIDTH, canvas.top()),
            canvas.max,
        );
        let block = self.clip_block(lane, clip, clip.start_frame);
        let edge_input = ui
            .input(|i| i.pointer.press_origin().or_else(|| i.pointer.latest_pos()))
            .is_some_and(|point| {
                (point.x - block.left()).abs() < midi_pattern_headers::edge_width(block)
                    || (point.x - block.right()).abs() < midi_pattern_headers::edge_width(block)
            });
        let visible = block.intersect(viewport);
        if !visible.is_positive() {
            return;
        }
        let painter = ui.painter_at(viewport);
        let palette = theme::palette(ui.ctx());
        let beat_width = self.editor_beat_width(clip);
        let step = self.editor_step(clip);
        let base = clip.repeat.map_or(clip.length_frames, |r| r.length_frames);
        let phase = clip.repeat.map_or(0, |r| r.phase_frame);
        let rate = f64::from(self.session.project.sample_rate_hz);
        let frame_at = |x: f32| {
            ((f64::from(x - block.left()) / f64::from(self.view_zoom())) * rate).max(0.0) as u64
        };
        let first = frame_at(visible.left()).saturating_add(phase) / base;
        let last = frame_at(visible.right()).saturating_add(phase) / base;
        let display = self.editor_pattern(clip.id, pattern).clone();
        // Below half a pixel per repetition, each visible pixel covers several
        // repeats. Paint their pitch coverage once instead of visiting every repeat.
        if clip.repeat.is_some()
            && self.session.project.seconds(base) as f32 * self.view_zoom() < 0.5
        {
            for note in &display.notes {
                let start = self.session.project.midi_frame(note.start_tick);
                let end = self
                    .session
                    .project
                    .midi_frame(note.start_tick + note.length_ticks);
                let start = clip.stretch.map_or(start, |s| s.scale(start).unwrap_or(0));
                let end = clip.stretch.map_or(end, |s| s.scale(end).unwrap_or(0));
                if start < clip.source_offset_frame + base && end > clip.source_offset_frame {
                    let y = canvas.top() + f32::from(127 - note.pitch) * NOTE_HEIGHT;
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(visible.left(), y + 1.0),
                            Pos2::new(visible.right(), y + NOTE_HEIGHT - 2.0),
                        ),
                        0.0,
                        if track.midi_options.contains_pitch(note.pitch) {
                            theme::clip_colors(track.color, clip.color).header
                        } else {
                            palette.secondary
                        },
                    );
                }
            }
            ui.interact(
                visible,
                egui::Id::new(("dense-midi-repeats", clip.id)),
                Sense::hover(),
            )
            .on_hover_text("Zoom in to edit individual notes.");
            return;
        }
        for repetition in first..=last {
            let local = i128::from(repetition) * i128::from(base) - i128::from(phase);
            let start = local.max(0).min(i128::from(clip.length_frames));
            let end = (local + i128::from(base)).min(i128::from(clip.length_frames));
            if end <= start {
                continue;
            }
            let span = Rect::from_min_max(
                Pos2::new(
                    block.left() + start as f32 / rate as f32 * self.view_zoom(),
                    canvas.top(),
                ),
                Pos2::new(
                    block.left() + end as f32 / rate as f32 * self.view_zoom(),
                    canvas.bottom(),
                ),
            );
            let source_left = block.left()
                + (local - i128::from(clip.source_offset_frame)) as f32 / rate as f32
                    * self.view_zoom();
            let grid = Rect::from_min_size(
                Pos2::new(source_left, canvas.top()),
                Vec2::new(
                    beat_width * display.length_ticks as f32 / MIDI_TICKS_PER_BEAT as f32,
                    canvas.height(),
                ),
            );
            let note_viewport = viewport.intersect(span);
            if !note_viewport.is_positive() {
                continue;
            }
            #[cfg(test)]
            {
                if Some(clip.id) == self.selected_clip && repetition == first {
                    self.midi_editor.grid_bounds = grid;
                }
            }
            if repetition > 0 {
                painter.line_segment(
                    [span.left_top(), span.left_bottom()],
                    Stroke::new(1.0, palette.divider),
                );
            }
            let note_painter = ui.painter_at(note_viewport);
            let mut note_ui = ui.new_child(egui::UiBuilder::new());
            if edge_input
                || self.drag.is_some()
                || self.midi_editor.drag.is_some()
                || self.midi_editor.draw.is_some()
                || self.midi_editor.selection.is_some()
            {
                note_ui.disable();
            }
            let _background = ui.interact(
                span.intersect(note_viewport),
                egui::Id::new(("midi-background", clip.id, repetition)),
                Sense::click_and_drag(),
            );
            for note in &display.notes {
                let rect = note_rect(grid, note, beat_width);
                if !rect.intersects(note_viewport) {
                    continue;
                }
                self.midi_editor
                    .note_regions
                    .push(((clip.id, note.id), rect.intersect(note_viewport)));
                #[cfg(test)]
                {
                    self.midi_editor
                        .track_note_bounds
                        .entry((clip.id, note.id))
                        .or_default()
                        .push(rect.intersect(note_viewport));
                    if Some(clip.id) == self.selected_clip {
                        self.midi_editor
                            .note_bounds
                            .insert(note.id, rect.intersect(note_viewport));
                    }
                }
                let response = note_ui.interact(
                    rect.intersect(note_viewport),
                    egui::Id::new(("midi-note", clip.id, repetition, note.id)),
                    Sense::click_and_drag(),
                );
                let selected = self
                    .midi_editor
                    .selected_notes
                    .contains(&(clip.id, note.id));
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        note_ui.is_enabled(),
                        selected,
                        format!(
                            "{} note · velocity {}",
                            note_name(note.pitch),
                            note.velocity
                        ),
                    )
                });
                note_painter.rect_filled(
                    rect.shrink(1.0),
                    2.0,
                    if !track.midi_options.contains_pitch(note.pitch) {
                        palette.secondary
                    } else if Some(clip.id) == self.selected_clip {
                        theme::accent(ui.ctx())
                    } else {
                        theme::clip_colors(track.color, clip.color).header
                    },
                );
                note_painter.rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(
                        if selected { 2.0 } else { 1.0 },
                        if selected {
                            palette.text
                        } else {
                            palette.border
                        },
                    ),
                    StrokeKind::Inside,
                );
                response.clone().on_hover_text(format!(
                    "{} · velocity {}",
                    note_name(note.pitch),
                    note.velocity
                ));
                if response.hovered()
                    && let Some(pointer) = response.hover_pos()
                {
                    ui.ctx().set_cursor_icon(
                        if pointer.x - rect.left() < 6.0 || rect.right() - pointer.x < 6.0 {
                            egui::CursorIcon::ResizeHorizontal
                        } else {
                            egui::CursorIcon::Grab
                        },
                    );
                }
                if response.clicked() && !self.midi_editor.dragging() {
                    if ui.input(|i| i.modifiers.shift) {
                        // Selecting a different pattern must retain the other selected notes.
                        self.select_clip(clip.id, track.id, false);
                        self.midi_editor.active_clip = Some(clip.id);
                        self.midi_editor.content.retain_on_selection = true;
                        if self.midi_editor.selected_notes.remove(&(clip.id, note.id)) {
                            self.midi_editor.selected_note = pattern
                                .notes
                                .iter()
                                .find(|n| {
                                    self.midi_editor.selected_notes.contains(&(clip.id, n.id))
                                })
                                .map(|n| n.id);
                        } else {
                            self.midi_editor.selected_notes.insert((clip.id, note.id));
                            self.midi_editor.selected_note = Some(note.id);
                        }
                    } else {
                        self.editor_select_clip(clip.id, track.id);
                        self.midi_editor.select_note(clip.id, note.id);
                    }
                }
                if !self.midi_editor.dragging()
                    && ((response.double_clicked() && !ui.input(|i| i.modifiers.shift))
                        || response.secondary_clicked())
                {
                    self.editor_select_clip(clip.id, track.id);
                    let mut next = pattern.clone();
                    next.notes.retain(|n| n.id != note.id);
                    self.edit(Edit::ReplaceMidiPattern {
                        clip_id: clip.id,
                        pattern: next,
                    });
                    self.midi_editor.clear_note_selection();
                    self.midi_editor.drag = None;
                }
                // egui may finish a fast drag before reporting drag_started this frame.
                let fast_origin = primary_press(ui).map(|(pos, _)| pos).filter(|origin| {
                    note_ui.is_enabled()
                        && !egui::Popup::is_any_open(ui.ctx())
                        && rect.intersect(note_viewport).contains(*origin)
                        && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
                        && ui.input(|i| i.pointer.latest_pos()).is_some_and(|end| {
                            origin.distance(end)
                                > ui.ctx().options(|o| o.input_options.max_click_dist)
                        })
                        && display
                            .notes
                            .iter()
                            .rev()
                            .find(|n| note_rect(grid, n, beat_width).contains(*origin))
                            .is_some_and(|n| n.id == note.id)
                });
                if (response.drag_started() || fast_origin.is_some())
                    && ui.input(|i| i.focused)
                    && self.midi_editor.drag.is_none()
                    && self.drag.is_none()
                    && self.midi_editor.draw.is_none()
                    && self.midi_editor.selection.is_none()
                    && let Some(origin) = ui.input(|i| i.pointer.press_origin()).or(fast_origin)
                {
                    let mode = if origin.x - rect.left() < 6.0 {
                        NoteDragMode::Start
                    } else if rect.right() - origin.x < 6.0 {
                        NoteDragMode::End
                    } else {
                        NoteDragMode::Move
                    };
                    self.start_midi_note_drag(
                        track,
                        clip,
                        note.id,
                        mode,
                        origin,
                        if matches!(mode, NoteDragMode::End) {
                            rect.right()
                        } else {
                            rect.left()
                        },
                    );
                }
            }
            if !edge_input
                && self.drag.is_none()
                && !self.midi_editor.dragging()
                && ui.is_enabled()
                && ui.input(|i| i.focused)
                && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                && !egui::Popup::is_any_open(ui.ctx())
                && let Some((pointer, modifiers)) = primary_press(ui)
                && modifiers.command
                && grid.intersect(note_viewport).contains(pointer)
                && !pattern
                    .notes
                    .iter()
                    .any(|n| note_rect(grid, n, beat_width).contains(pointer))
            {
                let pitch = (127 - ((pointer.y - grid.top()) / NOTE_HEIGHT).floor() as i16)
                    .clamp(0, 127) as u8;
                let pitch =
                    track
                        .midi_options
                        .snap_pitch(pitch, 0..=127, musical_time::snap_bypassed(ui));
                self.editor_select_clip(clip.id, track.id);
                let min_tick = (((span.left() - grid.left()) / beat_width
                    * MIDI_TICKS_PER_BEAT as f32)
                    .round()
                    .max(0.0) as u32)
                    .min(pattern.length_ticks - 1);
                let max_tick = (((span.right() - grid.left()) / beat_width
                    * MIDI_TICKS_PER_BEAT as f32)
                    .round()
                    .max(1.0) as u32)
                    .min(pattern.length_ticks)
                    .max(min_tick + 1);
                let start_tick = snapped_note_tick(
                    &self.musical_timeline(),
                    f64::from(pointer.x - lane.left()),
                    f64::from(grid.left() - lane.left()),
                    beat_width,
                    min_tick..=max_tick - 1,
                    musical_time::snap_bypassed(ui),
                )
                .min(pattern.length_ticks - 1);
                let note = Id::new_v4();
                self.midi_editor.draw = Some(NoteDraw {
                    clip: clip.id,
                    origin: pointer,
                    start_tick: start_tick.clamp(min_tick, max_tick - 1),
                    pitch,
                    note,
                    pattern: pattern.clone(),
                    original: pattern.clone(),
                    grid_left: grid.left(),
                    beat_width,
                    step,
                    min_tick,
                    max_tick,
                });
                self.midi_editor.select_note(clip.id, note);
            }
        }
    }
    fn start_midi_note_drag(
        &mut self,
        track: &daw_core::Track,
        clip: &Clip,
        note: Id,
        mode: NoteDragMode,
        origin: Pos2,
        anchor_x: f32,
    ) {
        let anchor_frame = self
            .musical_timeline()
            .frame_at_pixel(f64::from(anchor_x - self.midi_editor.content.left));
        let clip = clip.id;
        let previous_selection = self.midi_editor.selected_notes.clone();
        let previous_note = self.midi_editor.selected_note;
        let selected = if previous_selection.contains(&(clip, note)) {
            previous_selection.clone()
        } else {
            HashSet::from([(clip, note)])
        };
        let patterns = track
            .clips
            .iter()
            .filter_map(|c| {
                let original = self
                    .session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)?
                    .midi_pattern
                    .as_ref()?;
                let notes: Vec<_> = original
                    .notes
                    .iter()
                    .filter(|n| selected.contains(&(c.id, n.id)))
                    .map(|n| DraggedNote {
                        original: n.clone(),
                        copy_id: Id::new_v4(),
                    })
                    .collect();
                (!notes.is_empty()).then(|| NoteDragPattern {
                    clip: c.id,
                    original: original.clone(),
                    preview: original.clone(),
                    notes,
                    beat_width: self.editor_beat_width(c),
                })
            })
            .collect();
        // Changing the primary clip within this track must retain the note group and viewport.
        self.select_clip(clip, track.id, false);
        self.midi_editor.active_clip = Some(clip);
        self.midi_editor.content.retain_on_selection = true;
        self.midi_editor.selected_note = Some(note);
        self.midi_editor.selected_notes = selected;
        self.midi_editor.drag = Some(NoteDrag {
            anchor: (clip, note),
            patterns,
            mode,
            origin,
            anchor_frame,
            previous_selection,
            previous_note,
        });
    }
    fn update_midi_notes(&mut self, ui: &egui::Ui) {
        let timeline = self.musical_timeline();
        let timeline_left = self.midi_editor.content.left;
        let bypass = musical_time::snap_bypassed(ui);
        let scale = self.editor_midi_options();
        if let Some(drag) = &mut self.midi_editor.drag
            && ui.input(|i| {
                i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
            })
            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        {
            let movement = f64::from(pointer.x - drag.origin.x);
            let mut pixels = movement;
            let mut pitch_delta = ((pointer.y - drag.origin.y) / NOTE_HEIGHT).round() as i16;
            {
                let mut min_pixels = f64::NEG_INFINITY;
                let mut max_pixels = f64::INFINITY;
                let mut min_pitch_delta = -127;
                let mut max_pitch_delta = 127;
                for pattern in &drag.patterns {
                    let pixels_per_tick =
                        f64::from(pattern.beat_width) / f64::from(MIDI_TICKS_PER_BEAT);
                    for note in &pattern.notes {
                        let n = &note.original;
                        let available_end =
                            pattern.original.length_ticks - n.start_tick - n.length_ticks;
                        let (min_ticks, max_ticks) = match drag.mode {
                            NoteDragMode::Move => {
                                (-i64::from(n.start_tick), i64::from(available_end))
                            }
                            NoteDragMode::Start => {
                                (-i64::from(n.start_tick), i64::from(n.length_ticks - 1))
                            }
                            NoteDragMode::End => {
                                (-i64::from(n.length_ticks - 1), i64::from(available_end))
                            }
                        };
                        min_pixels = min_pixels.max(min_ticks as f64 * pixels_per_tick);
                        max_pixels = max_pixels.min(max_ticks as f64 * pixels_per_tick);
                        min_pitch_delta = min_pitch_delta.max(i16::from(n.pitch) - 127);
                        max_pitch_delta = max_pitch_delta.min(i16::from(n.pitch));
                    }
                }
                if movement != 0.0 {
                    let at = |pixels| {
                        (drag.anchor_frame + timeline.frame_delta(pixels))
                            .clamp(0, i128::from(u64::MAX)) as u64
                    };
                    let target = timeline.snap_bounded(
                        drag.anchor_frame + timeline.frame_delta(movement),
                        at(min_pixels)..=at(max_pixels),
                        std::iter::empty(),
                        bypass,
                    );
                    pixels = timeline.pixels_for_frames(i128::from(target) - drag.anchor_frame);
                }
                pixels = pixels.clamp(min_pixels, max_pixels);
                pitch_delta = pitch_delta.clamp(min_pitch_delta, max_pitch_delta);
                // Snap the anchor only on a pitch move. Keep a common displacement
                // for the group so timing-only drags and existing intervals stay intact.
                if matches!(drag.mode, NoteDragMode::Move)
                    && pitch_delta != 0
                    && let Some(anchor) = drag.patterns.iter().find_map(|p| {
                        p.notes
                            .iter()
                            .find(|n| (p.clip, n.original.id) == drag.anchor)
                    })
                {
                    let original = i16::from(anchor.original.pitch);
                    let pitch = scale.snap_pitch(
                        (original - pitch_delta) as u8,
                        (original - max_pitch_delta) as u8..=(original - min_pitch_delta) as u8,
                        bypass,
                    );
                    pitch_delta = original - i16::from(pitch);
                }
            }
            let duplicate = matches!(drag.mode, NoteDragMode::Move)
                && ui.input(|i| clip_copy_modifier(i.modifiers))
                && (pixels != 0.0 || pitch_delta != 0);
            let mut selected = HashSet::new();
            let mut anchor = None;
            for pattern in &mut drag.patterns {
                // Rebuild from immutable originals when modifiers or the pointer change.
                pattern.preview = pattern.original.clone();
                let local_delta = (pixels * f64::from(MIDI_TICKS_PER_BEAT)
                    / f64::from(pattern.beat_width))
                .round() as i64;
                for note in &pattern.notes {
                    let mut next = note.original.clone();
                    match drag.mode {
                        NoteDragMode::Move => {
                            next.start_tick = (i64::from(next.start_tick) + local_delta).clamp(
                                0,
                                i64::from(pattern.original.length_ticks - next.length_ticks),
                            ) as u32;
                            next.pitch = (i16::from(next.pitch) - pitch_delta) as u8;
                        }
                        NoteDragMode::Start => {
                            let end = next.start_tick + next.length_ticks;
                            next.start_tick = (i64::from(next.start_tick) + local_delta)
                                .clamp(0, i64::from(end - 1))
                                as u32;
                            next.length_ticks = end - next.start_tick;
                        }
                        NoteDragMode::End => {
                            next.length_ticks = (i64::from(next.length_ticks) + local_delta).clamp(
                                1,
                                i64::from(pattern.original.length_ticks - next.start_tick),
                            ) as u32;
                        }
                    }
                    if duplicate {
                        next.id = note.copy_id;
                        pattern.preview.notes.push(next.clone());
                    } else {
                        *pattern
                            .preview
                            .notes
                            .iter_mut()
                            .find(|n| n.id == next.id)
                            .unwrap() = next.clone();
                    }
                    selected.insert((pattern.clip, next.id));
                    if drag.anchor == (pattern.clip, note.original.id) {
                        anchor = Some(next.id);
                    }
                }
            }
            self.midi_editor.selected_notes = selected;
            self.midi_editor.selected_note = anchor;
            ui.ctx().set_cursor_icon(match drag.mode {
                NoteDragMode::Move if duplicate => egui::CursorIcon::Copy,
                NoteDragMode::Move => egui::CursorIcon::Grabbing,
                NoteDragMode::Start | NoteDragMode::End => egui::CursorIcon::ResizeHorizontal,
            });
            ui.ctx().request_repaint();
        }
        if let Some(draw) = &mut self.midi_editor.draw
            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        {
            let end = if (pointer.x - draw.origin.x).abs() < 0.001 {
                draw.start_tick
                    .saturating_add(draw.step)
                    .clamp(draw.min_tick, draw.max_tick)
            } else {
                snapped_note_tick(
                    &timeline,
                    f64::from(pointer.x - timeline_left),
                    f64::from(draw.grid_left - timeline_left),
                    draw.beat_width,
                    draw.min_tick..=draw.max_tick,
                    bypass,
                )
                .clamp(draw.min_tick, draw.max_tick)
            };
            let start = draw
                .start_tick
                .min(end)
                .clamp(draw.min_tick, draw.max_tick - 1);
            let end = draw
                .start_tick
                .max(end)
                .max(start + 1)
                .clamp(draw.min_tick, draw.max_tick);
            if let Ok(next) = draw.original.with_overwritten_note(MidiNote {
                id: draw.note,
                start_tick: start,
                length_ticks: end - start,
                pitch: draw.pitch,
                velocity: 100,
            }) {
                draw.pattern = next;
            }
        }
    }
    fn delete_midi_note(&mut self, clip: Id, pattern: &MidiPattern) {
        let Some(id) = self.midi_editor.selected_note else {
            return;
        };
        let mut next = pattern.clone();
        next.notes.retain(|n| n.id != id);
        self.midi_editor.selected_notes.remove(&(clip, id));
        self.midi_editor.selected_note = None;
        if next != *pattern {
            self.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: next,
            });
        }
    }
}

/// Adapt the shared project grid to a source's tick coordinates, including repeats/stretch.
fn snapped_note_tick(
    timeline: &musical_time::Timeline,
    pixel: f64,
    source_pixel: f64,
    beat_width: f32,
    limits: std::ops::RangeInclusive<u32>,
    bypass: bool,
) -> u32 {
    let at = |tick: u32| {
        timeline
            .frame_at_pixel(
                source_pixel
                    + f64::from(tick) * f64::from(beat_width) / f64::from(MIDI_TICKS_PER_BEAT),
            )
            .clamp(0, i128::from(u64::MAX)) as u64
    };
    let frame = timeline.snap_bounded(
        timeline.frame_at_pixel(pixel),
        at(*limits.start())..=at(*limits.end()),
        std::iter::empty(),
        bypass,
    );
    ((timeline.pixel_at_frame(frame) - source_pixel) / f64::from(beat_width)
        * f64::from(MIDI_TICKS_PER_BEAT))
    .round()
    .max(0.0) as u32
}
