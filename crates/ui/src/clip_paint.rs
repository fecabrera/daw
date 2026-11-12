//! Shared clip presentation for Timeline, the Editor track strip, and drag previews.
use super::*;
use daw_core::MidiPattern;

pub(super) const HEADER_HEIGHT: f32 = 22.0;
/// Half on each clip edge leaves one logical point between touching clips.
pub(super) const PAINT_INSET: f32 = 0.5;

pub(super) enum ClipAppearance {
    Normal(bool),
    Repeated,
}

impl From<bool> for ClipAppearance {
    fn from(selected: bool) -> Self {
        Self::Normal(selected)
    }
}

/// Up to two visual boundaries split a logical clip into at most three paint regions.
/// Keep the logical seam continuous when a crossfade owns audio beyond that seam.
fn clip_paint_regions(block: Rect, visual: Rect) -> [Rect; 3] {
    let mut regions = [Rect::NOTHING; 3];
    let mut count = 0;
    let mut cursor = block.left();
    let mut boundaries = [visual.left(), visual.right()];
    boundaries.sort_by(f32::total_cmp);
    let mut emit = |left: f32, right: f32| {
        if right > left {
            regions[count] = Rect::from_min_max(
                Pos2::new(left, block.top() + PAINT_INSET),
                Pos2::new(right, block.bottom() - PAINT_INSET),
            );
            count += 1;
        }
    };
    for boundary in boundaries {
        let left = (boundary - PAINT_INSET).clamp(block.left(), block.right());
        let right = (boundary + PAINT_INSET).clamp(block.left(), block.right());
        if right > cursor {
            emit(cursor, left.max(cursor));
            cursor = right;
        }
    }
    emit(cursor, block.right());
    regions
}

/// Shared temporal painting with separate MIDI pitch and unbounded beat-row coordinates.
enum NotePreview<'a> {
    Midi(&'a MidiPattern),
    Beat(&'a crate::beat_thumbnail::Level),
}
impl NotePreview<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Midi(pattern) => pattern.notes.len(),
            Self::Beat(level) => level.notes.len(),
        }
    }
    fn note(&self, index: usize) -> crate::beat_thumbnail::Note {
        match self {
            Self::Midi(pattern) => {
                let note = &pattern.notes[index];
                crate::beat_thumbnail::Note {
                    start_tick: note.start_tick,
                    length_ticks: note.length_ticks,
                    row: usize::from(note.pitch),
                }
            }
            Self::Beat(level) => level.notes[index],
        }
    }
    fn bounds(&self) -> (usize, usize) {
        match self {
            Self::Midi(pattern) => {
                let min = pattern.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
                let max = pattern
                    .notes
                    .iter()
                    .map(|n| n.pitch)
                    .max()
                    .unwrap_or(72)
                    .max(min.saturating_add(12));
                (usize::from(min), usize::from(max))
            }
            Self::Beat(level) => (0, level.max_row),
        }
    }
    fn max_length(&self) -> Option<u32> {
        match self {
            Self::Midi(_) => None,
            Self::Beat(level) => Some(level.max_length),
        }
    }
    fn partition_point(&self, predicate: impl Fn(crate::beat_thumbnail::Note) -> bool) -> usize {
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let mid = low + (high - low) / 2;
            if predicate(self.note(mid)) {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }
}

impl DawUi {
    pub(super) fn paint_clip(
        &self,
        painter: &egui::Painter,
        track_color: daw_core::RgbColor,
        block: Rect,
        clip: &Clip,
        appearance: ClipAppearance,
        prepared: Option<PreparedClipSource<'_>>,
    ) {
        let selected = matches!(appearance, ClipAppearance::Normal(true));
        let cached = prepared
            .and_then(|source| match source {
                PreparedClipSource::Audio(audio) => Some(audio),
                _ => None,
            })
            .or_else(|| self.session.audio_for_clip(clip));
        let audio = cached.or_else(|| self.session.audio.get(&clip.asset_id));
        let pattern = prepared
            .and_then(|source| match source {
                PreparedClipSource::Midi(pattern) => Some(pattern),
                _ => None,
            })
            .or_else(|| {
                self.session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .and_then(|a| a.midi_pattern.as_ref())
            });
        let live_beat = self
            .session
            .project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .and_then(|a| a.beat_pattern.as_ref());
        let beat = prepared
            .and_then(|source| match source {
                PreparedClipSource::Beat(beat) => Some(beat),
                _ => None,
            })
            .or(live_beat);
        let compound = self.session.compound_views.contains_key(&clip.asset_id);
        let missing = audio.is_none() && pattern.is_none() && beat.is_none() && !compound;
        let lane = painter.clip_rect();
        let colors = if missing {
            theme::ClipColors::default()
        } else {
            theme::clip_colors(track_color, clip.color)
        };
        let colors = if clip.muted {
            colors.dimmed(painter.ctx())
        } else {
            colors
        };
        let colors = if matches!(appearance, ClipAppearance::Repeated) {
            colors.repeated(painter.ctx())
        } else {
            colors
        };
        let header = if pattern.is_none() && beat.is_none() {
            self.clip_fade_header_rect(block, clip, painter.ctx())
        } else {
            block
        };
        let body_corners = egui::CornerRadius {
            nw: if header.left() == block.left() { 2 } else { 0 },
            sw: if header.left() == block.left() { 2 } else { 0 },
            ne: if header.right() == block.right() {
                2
            } else {
                0
            },
            se: if header.right() == block.right() {
                2
            } else {
                0
            },
        };
        // Crossfade ownership moves the visual boundary into the adjacent clip.
        // Split only painting at that boundary; clocks and gesture bounds stay unchanged.
        for region in clip_paint_regions(block, header) {
            if !region.is_positive() {
                continue;
            }
            let clip_painter = painter.with_clip_rect(region.intersect(lane));
            clip_painter.rect_filled(
                block,
                body_corners,
                if missing && !clip.muted {
                    theme::palette(painter.ctx()).missing
                } else {
                    colors.body
                },
            );
            let painted_header =
                paint_header(&clip_painter, header, &clip.name, colors, missing, selected);
            let body = Rect::from_min_max(
                Pos2::new(
                    block.left(),
                    (block.top() + clip_fades::WAVEFORM_TOP).min(block.bottom()),
                ),
                block.max,
            );
            if pattern.is_none() && beat.is_none() {
                if let Some(audio) = audio {
                    waveforms::paint_background(&clip_painter, body, audio, colors);
                }
                self.paint_clip_fade_background(&clip_painter, block, clip);
                self.paint_neighbor_crossfades(&clip_painter, block, clip, true);
            }
            if !clip.muted && !missing {
                // Recover the lane origin from the clip's project position, not
                // the painter's clip rect (which can start inside a scrolled clip).
                let origin = block.left()
                    - (self.session.project.seconds(clip.start_frame) - self.view_scroll()) as f32
                        * self.view_zoom();
                let grid = Rect::from_min_max(
                    Pos2::new(
                        block.left(),
                        (block.top() + HEADER_HEIGHT).min(block.bottom()),
                    ),
                    block.max,
                )
                .intersect(clip_painter.clip_rect());
                if grid.is_positive() {
                    let mut grid_painter = clip_painter.with_clip_rect(grid);
                    grid_painter.multiply_opacity(0.1);
                    // Colored clip fills need the same contrast in every theme.
                    self.musical_timeline().paint_grid_with_palette(
                        &grid_painter,
                        Rect::from_min_max(Pos2::new(origin, grid.top()), grid.max),
                        theme::DARK,
                    );
                }
            }
            if missing {
                clip_painter.text(
                    block.center(),
                    egui::Align2::CENTER_CENTER,
                    "Missing source",
                    egui::FontId::proportional(13.0),
                    theme::palette(painter.ctx()).warning,
                );
            } else if let Some(beat) = beat {
                let rows: Vec<_> = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .filter_map(|track| track.beat.as_ref())
                    .find(|container| container.id == beat.instrument_id)
                    .map(|container| container.subtracks.iter().map(|row| row.id).collect())
                    .unwrap_or_default();
                let step_frames = self
                    .session
                    .project
                    .midi_frame(beat.pattern.resolution.step_ticks());
                let step_frames = clip.stretch.map_or(step_frames, |stretch| {
                    stretch.scale(step_frames).unwrap_or(0)
                });
                let pixels = self.session.project.seconds(step_frames) as f32 * self.view_zoom();
                // Staged source-growth IDs are disposable. Do not retain their overview
                // allocations in the live-source cache across pointer frames.
                let level = if live_beat.is_none() {
                    crate::beat_thumbnail::Cache::default().level(
                        (self.session.project.project_id, self.change_revision),
                        clip.asset_id,
                        &rows,
                        &beat.pattern,
                        pixels,
                    )
                } else {
                    self.beat_ui.thumbnails.borrow_mut().level(
                        (self.session.project.project_id, self.change_revision),
                        clip.asset_id,
                        &rows,
                        &beat.pattern,
                        pixels,
                    )
                };
                self.paint_pattern_notes(
                    &clip_painter,
                    block,
                    clip,
                    NotePreview::Beat(&level),
                    colors.waveform,
                );
            } else if let Some(pattern) = pattern {
                self.paint_midi_notes(&clip_painter, block, clip, pattern, colors.waveform);
            } else if let Some(audio) = audio {
                waveforms::paint_peaks_with_gain(
                    &clip_painter,
                    body,
                    audio,
                    clip,
                    f64::from(self.session.project.sample_rate_hz) / f64::from(self.view_zoom()),
                    colors,
                    if cached.is_none() { clip.stretch } else { None },
                    self.waveform_fade_gain(clip, painter.ctx()),
                );
            }

            if compound && audio.is_none() {
                self.paint_compound_peaks(&clip_painter, body, clip, colors);
                self.paint_compound_transients(
                    &clip_painter,
                    Rect::from_min_max(
                        Pos2::new(body.left(), painted_header.bottom().min(body.bottom())),
                        body.max,
                    ),
                    clip,
                );
            }
            if pattern.is_none() && beat.is_none() {
                if let Some(audio) = audio {
                    self.paint_editor_transients(
                        &clip_painter,
                        Rect::from_min_max(
                            Pos2::new(body.left(), painted_header.bottom().min(body.bottom())),
                            body.max,
                        ),
                        clip,
                        audio,
                        if cached.is_none() { clip.stretch } else { None },
                    );
                }
                self.paint_neighbor_crossfade_waveforms(&clip_painter, block, clip);
                self.paint_clip_fades(&clip_painter, block, clip);
            }

            if let Some(repeat) = clip.repeat {
                let step =
                    self.session.project.seconds(repeat.length_frames) as f32 * self.view_zoom();
                if step >= 4.0 {
                    let first = repeat.length_frames - repeat.phase_frame;
                    let visible = self.session.project.frames(f64::from(
                        (lane.left() - block.left()).max(0.0) / self.view_zoom(),
                    ));
                    let skipped = visible.saturating_sub(first) / repeat.length_frames;
                    let mut local =
                        first.saturating_add(skipped.saturating_mul(repeat.length_frames));
                    while local < clip.length_frames {
                        let x = block.left()
                            + self.session.project.seconds(local) as f32 * self.view_zoom();
                        if x > lane.right() {
                            break;
                        }
                        if x >= lane.left() {
                            clip_painter.line_segment(
                                [Pos2::new(x, block.top()), Pos2::new(x, block.bottom())],
                                Stroke::new(1.0, colors.text.gamma_multiply(0.25)),
                            );
                        }
                        let Some(next) = local.checked_add(repeat.length_frames) else {
                            break;
                        };
                        local = next;
                    }
                }
            }
            if pattern.is_none() && beat.is_none() {
                self.paint_neighbor_crossfades(&clip_painter, block, clip, false);
            }
        }
        if pattern.is_none() && beat.is_none() {
            self.paint_clip_fade_controls(painter, block, clip);
            self.paint_neighbor_fade_controls(painter, block, clip);
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
        self.paint_pattern_notes(painter, block, clip, NotePreview::Midi(pattern), color);
    }

    fn paint_pattern_notes(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
        pattern: NotePreview<'_>,
        color: egui::Color32,
    ) {
        let body = Rect::from_min_max(
            Pos2::new(block.left(), block.top() + HEADER_HEIGHT),
            block.max,
        );
        let (min_row, max_row) = pattern.bounds();
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
            let range = if let Some(max_ticks) = pattern.max_length() {
                // Cached beat levels are sorted by start. Restrict traversal to notes
                // that can overlap this visible source interval, including boundary hits.
                let mapped = |ticks| {
                    let frames = self.session.project.midi_frame(ticks);
                    clip.stretch
                        .map_or(frames, |s| s.scale(frames).unwrap_or(0))
                };
                let origin = i128::from(clip.source_offset_frame)
                    + i128::from(clip.repeat.map_or(0, |r| r.phase_frame))
                    - i128::from(repetition) * i128::from(base);
                let left = origin + i128::from(visible_start);
                let right = origin
                    + ((painter.clip_rect().right() - body.left()).max(0.0) / self.view_zoom()
                        * self.session.project.sample_rate_hz as f32)
                        .ceil() as i128;
                let longest = i128::from(mapped(max_ticks).saturating_add(2));
                let first = pattern
                    .partition_point(|note| i128::from(mapped(note.start_tick)) + longest < left);
                let last =
                    pattern.partition_point(|note| i128::from(mapped(note.start_tick)) <= right);
                first..last.max(first)
            } else {
                0..pattern.len()
            };
            for index in range {
                let note = pattern.note(index);
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
                if right < painter.clip_rect().left() || x > painter.clip_rect().right() {
                    continue;
                }
                // Keep the fixed two-point note marker inside compact track bodies.
                let y = (body.bottom()
                    - (note.row.saturating_sub(min_row) as f32 + 1.0)
                        / (max_row - min_row + 2) as f32
                        * body.height())
                .min(body.bottom() - 2.0);
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x, y), Pos2::new(right.max(x + 1.0), y + 2.0)),
                    0.0,
                    color,
                );
            }
        }
    }
}

/// Resolve header appearance consistently across clip painters.
pub(super) fn header_colors(
    ctx: &egui::Context,
    colors: theme::ClipColors,
    missing: bool,
    selected: bool,
) -> theme::ClipColors {
    let colors = if missing {
        theme::ClipColors {
            header: theme::palette(ctx).border,
            selection_header: theme::GREY_200,
            text: theme::palette(ctx).text,
            ..colors
        }
    } else {
        colors
    };
    if selected {
        colors.selected_header()
    } else {
        colors
    }
}

/// Shared audio/MIDI clip title strip, also used inside the Sampler preview.
pub(super) fn paint_header(
    painter: &egui::Painter,
    block: Rect,
    name: &str,
    colors: theme::ClipColors,
    missing: bool,
    selected: bool,
) -> Rect {
    let colors = header_colors(painter.ctx(), colors, missing, selected);
    let header = Rect::from_min_max(
        block.min,
        Pos2::new(block.right(), block.top() + HEADER_HEIGHT),
    );
    painter.rect_filled(
        header,
        egui::CornerRadius {
            nw: 2,
            ne: 2,
            sw: 0,
            se: 0,
        },
        colors.header,
    );
    painter
        .with_clip_rect(header.intersect(painter.clip_rect()))
        .text(
            Pos2::new(header.left() + 7.0, header.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            colors.text,
        );
    header
}
