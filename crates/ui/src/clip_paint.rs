//! Shared clip presentation for Timeline, the Editor track strip, and drag previews.
use super::*;
use daw_core::MidiPattern;

impl DawUi {
    pub(super) fn paint_clip(
        &self,
        painter: &egui::Painter,
        track_color: daw_core::RgbColor,
        block: Rect,
        clip: &Clip,
        selected: bool,
        prepared: Option<PreparedClipSource<'_>>,
    ) {
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
        let missing = audio.is_none() && pattern.is_none();
        let lane = painter.clip_rect();
        let colors = if missing {
            theme::ClipColors::default()
        } else {
            theme::clip_colors(track_color, clip.color)
        };
        painter.rect_filled(
            block,
            2.0,
            if missing {
                theme::palette(painter.ctx()).missing
            } else {
                colors.body
            },
        );
        let clip_painter = painter.with_clip_rect(block.intersect(lane));
        paint_header(&clip_painter, block, &clip.name, colors, missing);
        if missing {
            clip_painter.text(
                block.center(),
                egui::Align2::CENTER_CENTER,
                "Missing source",
                egui::FontId::proportional(13.0),
                theme::palette(painter.ctx()).warning,
            );
        } else if let Some(pattern) = pattern {
            self.paint_midi_notes(&clip_painter, block, clip, pattern, colors.waveform);
        } else if let Some(audio) = audio {
            let body = Rect::from_min_max(Pos2::new(block.left(), block.top() + 22.0), block.max);
            waveforms::paint(
                &clip_painter,
                body,
                audio,
                clip,
                f64::from(self.session.project.sample_rate_hz) / f64::from(self.view_zoom()),
                colors,
                if cached.is_none() { clip.stretch } else { None },
            );
        }

        if let Some(repeat) = clip.repeat {
            let step = self.session.project.seconds(repeat.length_frames) as f32 * self.view_zoom();
            if step >= 4.0 {
                let first = repeat.length_frames - repeat.phase_frame;
                let visible = self.session.project.frames(f64::from(
                    (lane.left() - block.left()).max(0.0) / self.view_zoom(),
                ));
                let skipped = visible.saturating_sub(first) / repeat.length_frames;
                let mut local = first.saturating_add(skipped.saturating_mul(repeat.length_frames));
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
        painter.rect_stroke(
            block,
            2.0,
            if selected {
                theme::selection_outline(painter.ctx())
            } else {
                Stroke::new(1.0, theme::palette(painter.ctx()).divider)
            },
            StrokeKind::Inside,
        );
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

/// Shared audio/MIDI clip title strip, also used inside the Sampler preview.
pub(super) fn paint_header(
    painter: &egui::Painter,
    block: Rect,
    name: &str,
    colors: theme::ClipColors,
    missing: bool,
) -> Rect {
    let header = Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
    painter.rect_filled(
        header,
        2.0,
        if missing {
            theme::palette(painter.ctx()).border
        } else {
            colors.header
        },
    );
    painter
        .with_clip_rect(header.intersect(painter.clip_rect()))
        .text(
            Pos2::new(header.left() + 7.0, header.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            if missing {
                theme::palette(painter.ctx()).text
            } else {
                colors.text
            },
        );
    header
}
