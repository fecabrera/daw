use super::*;
use daw_core::{MIDI_TICKS_PER_BEAT, MidiNote, MidiPattern, TrackKind};

const KEY_WIDTH: f32 = 52.0;
const NOTE_HEIGHT: f32 = 18.0;
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
    beat_width: f32,
    step: u32,
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
struct PatternDrag {
    clip: Id,
    origin: Pos2,
    edge: daw_core::ClipEdge,
    original_length: u32,
    min_start: i64,
    max_start: i64,
    min_end: i64,
    beat_width: f32,
    step: u32,
    start: i64,
    end: i64,
    preview: Option<daw_core::Project>,
    error: Option<String>,
}
pub(super) struct MidiEditor {
    selected_note: Option<Id>,
    selected_notes: HashSet<(Id, Id)>,
    selection: Option<NoteSelection>,
    note_regions: Vec<((Id, Id), Rect)>,
    drag: Option<NoteDrag>,
    pattern_drag: Option<PatternDrag>,
    draw: Option<NoteDraw>,
    pub(super) zoom: f32,
    pub(super) scroll: f64,
    pub(super) source_origin: f64,
    active_clip: Option<Id>,
    active_track: Option<Id>,
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
    pub(crate) note_bounds: HashMap<Id, Rect>,
}
impl Default for MidiEditor {
    fn default() -> Self {
        Self {
            selected_note: None,
            selected_notes: HashSet::new(),
            selection: None,
            note_regions: Vec::new(),
            drag: None,
            pattern_drag: None,
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
            note_bounds: HashMap::new(),
        }
    }
}
impl MidiEditor {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
            || self.pattern_drag.is_some()
            || self.draw.is_some()
            || self.selection.is_some()
    }
    pub fn cancel(&mut self) {
        self.drag = None;
        self.pattern_drag = None;
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
    fn clear_note_selection(&mut self) {
        self.selected_note = None;
        self.selected_notes.clear();
    }
    fn select_note(&mut self, clip: Id, note: Id) {
        self.selected_note = Some(note);
        self.selected_notes = HashSet::from([(clip, note)]);
    }
}

fn primary_press(ui: &egui::Ui) -> Option<(Pos2, egui::Modifiers)> {
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
fn note_name(pitch: u8) -> String {
    format!(
        "{}{}",
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ][usize::from(pitch % 12)],
        i16::from(pitch) / 12 - 1
    )
}
fn black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

impl DawUi {
    pub(super) fn add_midi_pattern(&mut self, track: Id, start: u64, ctx: &egui::Context) {
        if self.view_mode != ViewMode::Timeline || !self.clip_actions_available(ctx) {
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
    pub(super) fn open_clip_editor(&mut self, clip: Id, track: Id, ctx: &egui::Context) {
        self.select_clip(clip, track, false);
        self.set_view_mode(ViewMode::Edit, ctx);
    }
    pub(super) fn source_track_control(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(!self.midi_editor.dragging(), |ui| {
            let tracks: Vec<_> = self
                .session
                .project
                .tracks
                .iter()
                .map(|t| (t.id, t.name.clone()))
                .collect();
            // Edit follows the active clip when Units has selected another track.
            let current = if self.view_mode == ViewMode::Edit {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.clips.iter().any(|c| Some(c.id) == self.selected_clip))
                    .map(|t| t.id)
                    .or(self.selected_track)
            } else {
                self.selected_track
            };
            let label = current
                .and_then(|id| tracks.iter().find(|t| t.0 == id))
                .map_or("Select track", |t| t.1.as_str());
            let mut selected = current;
            egui::ComboBox::from_id_salt("source-track")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (id, name) in &tracks {
                        ui.selectable_value(&mut selected, Some(*id), name);
                    }
                });
            if selected != current
                && let Some(id) = selected
            {
                self.select_track(id, false);
                if self.view_mode == ViewMode::Edit {
                    let clip = self
                        .session
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.id == id)
                        .and_then(|t| t.clips.iter().min_by_key(|c| c.start_frame))
                        .map(|c| c.id);
                    if let Some(clip) = clip {
                        self.select_clip(clip, id, false);
                    } else {
                        self.selected_clip = None;
                        self.selected_clips.clear();
                    }
                    self.midi_editor.cancel();
                }
            }
        });
    }
    pub(super) fn units_view(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        // The first unit's top border shares this workspace divider.
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.max_rect().top() + 0.5,
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| Some(t.id) == self.selected_track)
            .cloned()
        else {
            ui.label("Select a MIDI track to configure its instrument. Add tracks in Timeline.");
            return;
        };
        if track.kind != TrackKind::Midi {
            ui.label("Audio tracks accept effects. Effects are not available yet.");
            return;
        }
        panels::PanelStyle::default().unit(ui, "Oscillator", |ui| {
            let Some(mut settings) = track.instrument.clone() else {
                ui.label("No instrument assigned");
                return;
            };
            egui::Grid::new(("oscillator-settings", track.id))
                .spacing(Vec2::new(12.0, 8.0))
                .show(ui, |ui| {
                    ui.label("Waveform");
                    let waveform = egui::ComboBox::from_id_salt("oscillator-waveform")
                        .selected_text(settings.waveform.label())
                        .show_ui(ui, |ui| {
                            for waveform in daw_core::Waveform::ALL {
                                ui.selectable_value(
                                    &mut settings.waveform,
                                    waveform,
                                    waveform.label(),
                                );
                            }
                        });
                    let width = waveform.response.rect.width();
                    ui.end_row();
                    ui.label("Transpose");
                    ui.scope(|ui| {
                        ui.spacing_mut().interact_size.x = width;
                        ui.add_sized(
                            Vec2::new(width, toolbars::CONTROL_HEIGHT),
                            egui::DragValue::new(&mut settings.transpose)
                                .range(-48..=48)
                                .suffix(" semitones"),
                        );
                    });
                    ui.end_row();
                    ui.label("Gain");
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, knobs::SIZE),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = theme::TOOLBAR_PADDING as f32;
                            knobs::gain(ui, &mut settings.gain_db);
                            let width = ui.available_width();
                            ui.spacing_mut().interact_size.x = width;
                            ui.add_sized(
                                Vec2::new(width, toolbars::CONTROL_HEIGHT),
                                egui::DragValue::new(&mut settings.gain_db)
                                    .range(-60.0..=12.0)
                                    .speed(0.1)
                                    .suffix(" dB"),
                            );
                        },
                    );
                    ui.end_row();
                });
            if Some(&settings) != track.instrument.as_ref() {
                self.edit(Edit::SetInstrument {
                    track_id: track.id,
                    instrument: Some(settings),
                });
            }
        });
    }
    fn editor_ruler(&mut self, ui: &mut egui::Ui) {
        let row = rows::centered(ui, RULER_HEIGHT, |ui| {
            ui.allocate_exact_size(Vec2::new(KEY_WIDTH, RULER_HEIGHT), Sense::hover());
            let rect = self.time_ruler(ui);
            #[cfg(test)]
            {
                self.midi_editor.ruler_bounds = rect;
            }
            #[cfg(not(test))]
            {
                let _ = rect;
            }
        });
        let y = row.response.rect.bottom() - 0.5;
        ui.painter().line_segment(
            [
                Pos2::new(ui.max_rect().left(), y),
                Pos2::new(ui.max_rect().right(), y),
            ],
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
    }
    pub(super) fn edit_view(&mut self, ui: &mut egui::Ui) {
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
            self.midi_editor.pattern_drag = None;
            if self.midi_editor.draw.take().is_some() {
                self.midi_editor.clear_note_selection();
            }
        }
        self.update_pattern_resize(ui);
        self.update_midi_notes(ui);
        let previous_track = self.midi_editor.active_track;
        let project = self
            .midi_editor
            .pattern_drag
            .as_ref()
            .and_then(|d| d.preview.as_ref())
            .unwrap_or(&self.session.project);
        let track = project
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| Some(c.id) == self.selected_clip))
            .or_else(|| {
                project
                    .tracks
                    .iter()
                    .find(|t| Some(t.id) == self.selected_track)
            })
            .cloned();
        let assets: Vec<_> = project
            .assets
            .iter()
            .filter(|asset| {
                track
                    .as_ref()
                    .is_some_and(|t| t.clips.iter().any(|c| c.asset_id == asset.id))
            })
            .cloned()
            .collect();
        // Selecting another pattern on the same track must not jump the shared viewport.
        self.midi_editor.source_origin = 0.0;
        if self.midi_editor.active_clip != self.selected_clip && !self.midi_editor.dragging() {
            self.midi_editor.clear_note_selection();
            self.midi_editor.active_clip = self.selected_clip;
        }
        if let Some(track) = &track
            && self.midi_editor.active_track != Some(track.id)
        {
            self.midi_editor.cancel();
            self.midi_editor.scroll = track
                .clips
                .iter()
                .find(|c| Some(c.id) == self.selected_clip)
                .map_or(0.0, |c| self.session.project.seconds(c.start_frame));
            self.midi_editor.active_track = Some(track.id);
            self.midi_editor.active_clip = self.selected_clip;
        }
        let viewport = Rect::from_min_max(
            Pos2::new(ui.max_rect().left() + KEY_WIDTH, ui.max_rect().top()),
            ui.max_rect().max,
        );
        self.timeline_pinch(ui, viewport);
        self.timeline_swipe(ui, viewport);
        self.editor_ruler(ui);
        let Some(track) = track else {
            ui.label("Select a MIDI track to edit its patterns.");
            return;
        };
        self.editor_track_strip(ui, &track, &assets);
        self.update_pattern_resize(ui);
        if track.kind != TrackKind::Midi {
            ui.label("The audio editor is not available yet.");
            return;
        }
        if !self.midi_editor.dragging()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.delete_selected_midi_notes();
        }
        let mut scroll = egui::ScrollArea::vertical().id_salt(("piano-roll", track.id));
        if previous_track != Some(track.id) {
            scroll = scroll.vertical_scroll_offset(55.0 * NOTE_HEIGHT);
        }
        scroll
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 15.0).max(0.0))
            .show(ui, |ui| {
                let (canvas, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 128.0 * NOTE_HEIGHT),
                    Sense::hover(),
                );
                let grid_left = canvas.left() + KEY_WIDTH;
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
                            if black_key(pitch) {
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
                            palette.text,
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
                    self.editor_clip_notes(ui, canvas, note_viewport, &track, clip, pattern);
                }
                self.update_midi_notes(ui);
                self.editor_note_selection(ui, note_viewport);
                let x = grid_left
                    + (self
                        .session
                        .project
                        .seconds(self.session.project.transport.playhead_frame)
                        - self.midi_editor.scroll) as f32
                        * self.midi_editor.zoom;
                if x >= grid_left && x <= canvas.right() {
                    p.line_segment(
                        [Pos2::new(x, canvas.top()), Pos2::new(x, canvas.bottom())],
                        Stroke::new(1.0, theme::accent(ui.ctx())),
                    );
                }
            });
        rows::centered(ui, 15.0, |ui| {
            ui.allocate_exact_size(Vec2::new(KEY_WIDTH, 15.0), Sense::hover());
            self.timeline_scrollbar(ui);
        });
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(draw) = self.midi_editor.draw.take()
        {
            self.edit(Edit::ReplaceMidiPattern {
                clip_id: draw.clip,
                pattern: draw.pattern,
            });
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(drag) = self.midi_editor.pattern_drag.take()
        {
            if let Some(error) = drag.error {
                self.fail(error);
            } else if drag.start != 0 || drag.end != i64::from(drag.original_length) {
                self.edit(Edit::ResizeMidiPattern {
                    clip_id: drag.clip,
                    start_tick: drag.start,
                    end_tick: drag.end,
                });
            }
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
    fn editor_select_clip(&mut self, clip: Id, track: Id) {
        if self.selected_clip != Some(clip) {
            self.midi_editor.clear_note_selection();
        }
        self.select_clip(clip, track, false);
        self.midi_editor.active_clip = Some(clip);
    }
    fn editor_pattern<'a>(&'a self, clip: Id, original: &'a MidiPattern) -> &'a MidiPattern {
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
        self.midi_editor.zoom * 60.0 / self.session.project.tempo_bpm
            * clip.stretch.map_or(1.0, |s| s.ratio() as f32)
    }
    fn editor_step(&self, clip: &Clip) -> u32 {
        (musical_time::Timeline::at_sample_rate(
            self.session.project.tempo_bpm,
            self.midi_editor.zoom * clip.stretch.map_or(1.0, |s| s.ratio() as f32),
            0.0,
            self.session.project.sample_rate_hz,
        )
        .visible_step()
            * f64::from(MIDI_TICKS_PER_BEAT))
        .round()
        .max(1.0) as u32
    }
    fn editor_clip_rect(&self, band: Rect, clip: &Clip) -> Rect {
        let left = band.left()
            + (self.session.project.seconds(clip.start_frame) - self.midi_editor.scroll) as f32
                * self.midi_editor.zoom;
        Rect::from_min_size(
            Pos2::new(left, band.top()),
            Vec2::new(
                self.session.project.seconds(clip.length_frames) as f32 * self.midi_editor.zoom,
                band.height(),
            ),
        )
    }
    fn editor_track_strip(
        &mut self,
        ui: &mut egui::Ui,
        track: &daw_core::Track,
        assets: &[daw_core::Asset],
    ) {
        let (band, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
        let lane = Rect::from_min_max(Pos2::new(band.left() + KEY_WIDTH, band.top()), band.max);
        let palette = theme::palette(ui.ctx());
        ui.painter().rect_filled(band, 0.0, palette.background);
        ui.painter()
            .with_clip_rect(Rect::from_min_max(band.min, lane.left_bottom()))
            .text(
                Pos2::new(band.left() + 4.0, band.center().y),
                egui::Align2::LEFT_CENTER,
                &track.name,
                egui::FontId::proportional(10.0),
                palette.text,
            );
        #[cfg(test)]
        {
            self.midi_editor.track_bounds = lane;
            self.midi_editor.clip_bounds.clear();
        }
        let painter = ui.painter_at(lane);
        self.paint_grid(&painter, lane);
        for clip in &track.clips {
            let block = self.editor_clip_rect(lane, clip);
            let header =
                Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
            #[cfg(test)]
            {
                self.midi_editor.clip_bounds.insert(clip.id, block);
                if Some(clip.id) == self.selected_clip {
                    self.midi_editor.pattern_bounds = header;
                }
            }
            if !block.intersects(lane) {
                continue;
            }
            let colors = theme::clip_colors(track.color, clip.color);
            painter.rect_filled(block, 0.0, colors.body);
            painter.rect_filled(header, 0.0, colors.header);
            painter.with_clip_rect(header.intersect(lane)).text(
                Pos2::new(header.left() + 6.0, header.center().y),
                egui::Align2::LEFT_CENTER,
                &clip.name,
                egui::FontId::proportional(11.0),
                colors.text,
            );
            let pattern = assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .and_then(|a| a.midi_pattern.as_ref());
            if let Some(pattern) = pattern {
                self.paint_midi_notes(
                    &painter,
                    block,
                    clip,
                    self.editor_pattern(clip.id, pattern),
                    colors.waveform,
                );
            }
            painter.rect_stroke(
                block,
                0.0,
                if Some(clip.id) == self.selected_clip {
                    theme::selection_outline(ui.ctx())
                } else {
                    Stroke::new(1.0, palette.secondary)
                },
                StrokeKind::Inside,
            );
            let header_response = ui
                .interact(
                    block.intersect(lane),
                    egui::Id::new(("editor-track-clip", clip.id)),
                    Sense::click(),
                )
                .on_hover_text("Select pattern. Drag either edge to resize it.");
            if header_response.clicked() && !self.midi_editor.dragging() {
                self.editor_select_clip(clip.id, track.id);
            }
            let Some(pattern) = pattern else {
                continue;
            };
            if Some(clip.id) == self.selected_clip {
                header_response.context_menu(|ui| {
                    if let Some(note) = pattern
                        .notes
                        .iter()
                        .find(|n| Some(n.id) == self.midi_editor.selected_note)
                    {
                        let mut velocity = note.velocity;
                        ui.label("Velocity");
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
                            self.delete_midi_note(clip.id, pattern);
                        }
                    } else {
                        ui.label("Select a note to edit its velocity.");
                    }
                });
            }
            for (edge, x) in [
                (daw_core::ClipEdge::Left, block.left()),
                (daw_core::ClipEdge::Right, block.right()),
            ] {
                // Keep adjacent patterns' handles on their own side of the boundary.
                let (left, right) = if edge == daw_core::ClipEdge::Left {
                    (x, x + 4.0)
                } else {
                    (x - 4.0, x)
                };
                let handle = Rect::from_min_max(
                    Pos2::new(left, band.top()),
                    Pos2::new(right, band.bottom()),
                )
                .intersect(lane);
                if !handle.is_positive() {
                    continue;
                }
                let response = ui
                    .interact(
                        handle,
                        egui::Id::new(("pattern-edge", clip.id, edge == daw_core::ClipEdge::Left)),
                        Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                    .on_hover_text("Resize pattern. Shift bypasses snapping; Escape cancels.");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        ui.is_enabled(),
                        if edge == daw_core::ClipEdge::Left {
                            "Pattern start"
                        } else {
                            "Pattern end"
                        },
                    )
                });
                if self.midi_editor.drag.is_none()
                    && self.midi_editor.draw.is_none()
                    && ui.is_enabled()
                    && ui.input(|i| i.focused)
                    && response.drag_started()
                    && let Some(origin) = ui.input(|i| i.pointer.press_origin())
                {
                    self.editor_select_clip(clip.id, track.id);
                    let beat_width = self.editor_beat_width(clip);
                    let min_start = -(self.session.project.seconds(clip.start_frame)
                        * f64::from(self.midi_editor.zoom)
                        / f64::from(beat_width)
                        * f64::from(MIDI_TICKS_PER_BEAT))
                    .floor() as i64;
                    let visible_ticks = (self.session.project.seconds(clip.length_frames)
                        * f64::from(self.midi_editor.zoom)
                        / f64::from(beat_width)
                        * f64::from(MIDI_TICKS_PER_BEAT))
                    .floor()
                    .max(1.0) as i64;
                    let max_start = (i64::from(pattern.length_ticks) - 1).min(visible_ticks - 1);
                    let min_end = (i64::from(pattern.length_ticks) - visible_ticks + 1).max(1);
                    self.midi_editor.pattern_drag = Some(PatternDrag {
                        clip: clip.id,
                        origin,
                        edge,
                        original_length: pattern.length_ticks,
                        min_start,
                        max_start,
                        min_end,
                        beat_width,
                        step: self.editor_step(clip),
                        start: 0,
                        end: i64::from(pattern.length_ticks),
                        preview: None,
                        error: None,
                    });
                }
            }
        }
        ui.painter().line_segment(
            [band.left_bottom(), band.right_bottom()],
            Stroke::new(1.0, palette.border),
        );
        let x = lane.left()
            + (self
                .session
                .project
                .seconds(self.session.project.transport.playhead_frame)
                - self.midi_editor.scroll) as f32
                * self.midi_editor.zoom;
        painter.line_segment(
            [Pos2::new(x, lane.top()), Pos2::new(x, lane.bottom())],
            Stroke::new(1.0, theme::accent(ui.ctx())),
        );
    }
    fn update_pattern_resize(&mut self, ui: &egui::Ui) {
        if let Some(drag) = &mut self.midi_editor.pattern_drag
            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
            && ui.input(|i| {
                i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
            })
        {
            let quantum = if ui.input(|i| i.modifiers.shift) {
                1
            } else {
                drag.step
            };
            let delta = (((pointer.x - drag.origin.x) / drag.beat_width
                * MIDI_TICKS_PER_BEAT as f32
                / quantum as f32)
                .round() as i64)
                .saturating_mul(i64::from(quantum));
            match drag.edge {
                daw_core::ClipEdge::Left => {
                    drag.start = delta.clamp(
                        (i64::from(drag.original_length) - i64::from(u32::MAX)).max(drag.min_start),
                        drag.max_start,
                    )
                }
                daw_core::ClipEdge::Right => {
                    drag.end = i64::from(drag.original_length)
                        .saturating_add(delta)
                        .clamp(drag.min_end, i64::from(u32::MAX))
                }
            }
            match self
                .session
                .project
                .resized_midi_pattern(drag.clip, drag.start, drag.end)
            {
                Ok(preview) => {
                    drag.preview = Some(preview);
                    drag.error = None;
                }
                Err(error) => {
                    drag.preview = None;
                    drag.error = Some(error.to_string());
                }
            }
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
        let block = self.editor_clip_rect(lane, clip);
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
            ((f64::from(x - block.left()) / f64::from(self.midi_editor.zoom)) * rate).max(0.0)
                as u64
        };
        let first = frame_at(visible.left()).saturating_add(phase) / base;
        let last = frame_at(visible.right()).saturating_add(phase) / base;
        let display = self.editor_pattern(clip.id, pattern).clone();
        // Below half a pixel per repetition, each visible pixel covers several
        // repeats. Paint their pitch coverage once instead of visiting every repeat.
        if clip.repeat.is_some()
            && self.session.project.seconds(base) as f32 * self.midi_editor.zoom < 0.5
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
                        theme::clip_colors(track.color, clip.color).header,
                    );
                }
            }
            painter.rect_stroke(
                block,
                0.0,
                Stroke::new(1.0, palette.secondary),
                StrokeKind::Inside,
            );
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
                    block.left() + start as f32 / rate as f32 * self.midi_editor.zoom,
                    canvas.top(),
                ),
                Pos2::new(
                    block.left() + end as f32 / rate as f32 * self.midi_editor.zoom,
                    canvas.bottom(),
                ),
            );
            let source_left = block.left()
                + (local - i128::from(clip.source_offset_frame)) as f32 / rate as f32
                    * self.midi_editor.zoom;
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
            if self.midi_editor.pattern_drag.is_some()
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
                    if Some(clip.id) == self.selected_clip {
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
                    && self.midi_editor.pattern_drag.is_none()
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
                    self.start_midi_note_drag(track, clip, note.id, mode, origin);
                }
            }
            if !self.midi_editor.dragging()
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
                let quantum = if ui.input(|i| i.modifiers.shift) {
                    1
                } else {
                    step
                };
                let start_tick = ((((pointer.x - grid.left()) / beat_width
                    * MIDI_TICKS_PER_BEAT as f32
                    / quantum as f32)
                    .floor() as u32)
                    .saturating_mul(quantum))
                .min(pattern.length_ticks - 1);
                let pitch = (127 - ((pointer.y - grid.top()) / NOTE_HEIGHT).floor() as i16)
                    .clamp(0, 127) as u8;
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
        painter.rect_stroke(
            block,
            0.0,
            Stroke::new(1.0, palette.secondary),
            StrokeKind::Inside,
        );
    }
    fn start_midi_note_drag(
        &mut self,
        track: &daw_core::Track,
        clip: &Clip,
        note: Id,
        mode: NoteDragMode,
        origin: Pos2,
    ) {
        let beat_width = self.editor_beat_width(clip);
        let step = self.editor_step(clip);
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
        self.midi_editor.selected_note = Some(note);
        self.midi_editor.selected_notes = selected;
        self.midi_editor.drag = Some(NoteDrag {
            anchor: (clip, note),
            patterns,
            mode,
            origin,
            beat_width,
            step,
            previous_selection,
            previous_note,
        });
    }
    fn update_midi_notes(&mut self, ui: &egui::Ui) {
        if let Some(drag) = &mut self.midi_editor.drag
            && ui.input(|i| {
                i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
            })
            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        {
            let quantum = if ui.input(|i| i.modifiers.shift) {
                1
            } else {
                drag.step
            };
            let delta = ((pointer.x - drag.origin.x) * MIDI_TICKS_PER_BEAT as f32
                / drag.beat_width
                / quantum as f32)
                .round() as i64
                * i64::from(quantum);
            // Use one project-space displacement so stretched patterns keep group spacing.
            let mut pixels =
                delta as f64 * f64::from(drag.beat_width) / f64::from(MIDI_TICKS_PER_BEAT);
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
                pixels = pixels.clamp(min_pixels, max_pixels);
                pitch_delta = pitch_delta.clamp(min_pitch_delta, max_pitch_delta);
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
            let quantum = if ui.input(|i| i.modifiers.shift) {
                1
            } else {
                draw.step
            };
            let end = if (pointer.x - draw.origin.x).abs() < 0.001 {
                draw.start_tick
                    .saturating_add(draw.step)
                    .clamp(draw.min_tick, draw.max_tick)
            } else {
                ((((pointer.x - draw.grid_left) / draw.beat_width * MIDI_TICKS_PER_BEAT as f32
                    / quantum as f32)
                    .round()
                    .max(0.0) as u32)
                    .saturating_mul(quantum))
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
    pub(super) fn paint_midi_notes(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
        pattern: &MidiPattern,
        color: egui::Color32,
    ) {
        let body = Rect::from_min_max(Pos2::new(block.left(), block.top() + 22.0), block.max);
        let min_pitch = pattern.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let max_pitch = pattern
            .notes
            .iter()
            .map(|n| n.pitch)
            .max()
            .unwrap_or(72)
            .max(min_pitch.saturating_add(12));
        let count = clip.repeat.map_or(1, |repeat| {
            (clip.length_frames / repeat.length_frames).saturating_add(2)
        });
        // Limit painting to visible repetitions instead of iterating an entire long clip.
        let visible_start = ((painter.clip_rect().left() - body.left()).max(0.0) / self.view_zoom()
            * self.session.project.sample_rate_hz as f32) as u64;
        let base = clip.repeat.map_or(clip.length_frames, |r| r.length_frames);
        let first = visible_start / base;
        let visible_count = (painter.clip_rect().width().max(0.0) / self.view_zoom()
            * self.session.project.sample_rate_hz as f32) as u64
            / base
            + 3;
        let pixels_per_repeat = self.session.project.seconds(base) as f32 * self.view_zoom();
        let stride = (0.5 / pixels_per_repeat).ceil().max(1.0) as usize;
        for repetition in (first..count.min(first.saturating_add(visible_count))).step_by(stride) {
            for note in &pattern.notes {
                let original_start = self.session.project.midi_frame(note.start_tick);
                let original_end = self
                    .session
                    .project
                    .midi_frame(note.start_tick + note.length_ticks);
                let mut start = clip
                    .stretch
                    .map_or(original_start, |s| s.scale(original_start).unwrap_or(0));
                let mut end = clip
                    .stretch
                    .map_or(original_end, |s| s.scale(original_end).unwrap_or(0));
                if let Some(repeat) = clip.repeat {
                    start = start.max(clip.source_offset_frame);
                    end = end.min(clip.source_offset_frame + repeat.length_frames);
                    if end <= start {
                        continue;
                    }
                }
                let phase = clip.repeat.map_or(0, |r| r.phase_frame);
                let local =
                    i128::from(start) - i128::from(clip.source_offset_frame) - i128::from(phase)
                        + i128::from(repetition) * i128::from(base);
                let finish = local + i128::from(end.saturating_sub(start));
                if finish <= 0 || local >= i128::from(clip.length_frames) {
                    continue;
                }
                let x = body.left()
                    + local.max(0) as f32 / self.session.project.sample_rate_hz as f32
                        * self.view_zoom();
                let right = body.left()
                    + finish.min(i128::from(clip.length_frames)) as f32
                        / self.session.project.sample_rate_hz as f32
                        * self.view_zoom();
                let y = body.bottom()
                    - (f32::from(note.pitch.saturating_sub(min_pitch)) + 1.0)
                        / f32::from(max_pitch - min_pitch + 2)
                        * body.height();
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x, y), Pos2::new(right.max(x + 1.0), y + 2.0)),
                    0.0,
                    color,
                );
            }
        }
    }
}
