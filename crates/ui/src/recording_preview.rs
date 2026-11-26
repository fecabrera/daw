//! Bounded capture thumbnails and the transient Timeline overlay.
use super::*;
use audio_analysis::waveform;
use daw_project::midi_recording::TrackPreview;
use egui::Color32;
use std::sync::Arc;

const MAX_PEAKS: usize = 2048;
const CHUNK_FRAMES: usize = 1024;

#[derive(Clone)]
pub(super) struct AudioPreview {
    pub tracks: Vec<(Id, usize)>,
    pub frames: u64,
    bin_frames: u64,
    peaks: Vec<Vec<[f32; 2]>>,
}

impl AudioPreview {
    fn append_chunk(&mut self, mut samples: &[[f32; 2]]) {
        let routes = self.peaks.len();
        while !samples.is_empty() {
            if self.frames / self.bin_frames == MAX_PEAKS as u64 {
                for peaks in &mut self.peaks {
                    for index in 0..MAX_PEAKS / 2 {
                        peaks[index] = waveform::merge(peaks[2 * index], peaks[2 * index + 1]);
                    }
                    peaks.truncate(MAX_PEAKS / 2);
                }
                self.bin_frames *= 2;
            }
            let index = (self.frames / self.bin_frames) as usize;
            let remaining = self.bin_frames - self.frames % self.bin_frames;
            let length = (samples.len() / routes).min(remaining as usize);
            let (bin, rest) = samples.split_at(length * routes);
            for (route, peaks) in self.peaks.iter_mut().enumerate() {
                let peak = bin[route..].iter().step_by(routes).fold(
                    peaks.get(index).copied().unwrap_or(waveform::EMPTY),
                    |peak, &[left, right]| {
                        waveform::merge(peak, [left.min(right), left.max(right)])
                    },
                );
                if index == peaks.len() {
                    peaks.push(peak);
                } else {
                    peaks[index] = peak;
                }
            }
            self.frames += length as u64;
            samples = rest;
        }
    }
}

pub(super) struct AudioPreviewBuilder {
    waveform: AudioPreview,
    pending: Vec<[f32; 2]>,
    published: Option<Arc<AudioPreview>>,
}

impl AudioPreviewBuilder {
    pub fn new(tracks: Vec<(Id, usize)>, route_count: usize) -> Self {
        let waveform = AudioPreview {
            tracks,
            frames: 0,
            bin_frames: 64,
            peaks: (0..route_count)
                .map(|_| Vec::with_capacity(MAX_PEAKS))
                .collect(),
        };
        Self {
            waveform,
            pending: Vec::with_capacity(CHUNK_FRAMES * route_count),
            published: None,
        }
    }

    pub fn push(&mut self, samples: &[[f32; 2]]) {
        self.pending.extend_from_slice(samples);
        if self.pending.len() == CHUNK_FRAMES * self.waveform.peaks.len() {
            self.waveform.append_chunk(&self.pending);
            self.pending.clear();
            self.published = None;
        }
    }

    pub fn snapshot(&mut self) -> Arc<AudioPreview> {
        self.published
            .get_or_insert_with(|| Arc::new(self.waveform.clone()))
            .clone()
    }
}

pub(super) struct TakePreview {
    pub project: Id,
    pub start: u64,
    pub audio: Option<Arc<AudioPreview>>,
    pub midi: Vec<TrackPreview>,
    pub sealed_at: Option<u64>,
}

impl DawUi {
    pub(super) fn advance_recording_preview(&mut self) {
        if self.midi.preview.is_some() && self.midi.recording && !self.midi.finalizing {
            self.midi.preview_head = self
                .midi
                .preview_head
                .max(self.session.project.transport.playhead_frame);
        }
    }

    pub(super) fn recording_preview_end(&self) -> u64 {
        self.midi.preview.as_ref().map_or(0, |take| {
            take.sealed_at
                .unwrap_or(self.midi.preview_head)
                .max(take.start)
        })
    }

    pub(super) fn paint_recording_preview(&self, ui: &egui::Ui, viewport: Rect) {
        let Some(preview) = self.midi.preview.as_ref() else {
            return;
        };
        let end = self.recording_preview_end();
        let relative_x =
            |frames: u64| self.session.project.seconds(frames) as f32 * self.view_zoom();
        if let Some(audio) = &preview.audio {
            for &(track, route) in &audio.tracks {
                self.paint_recording_clip(
                    ui,
                    viewport,
                    track,
                    preview.start,
                    end.saturating_sub(preview.start),
                    |painter, body, color| {
                        if audio.frames == 0 {
                            return;
                        }
                        let middle = body.center().y;
                        let scale = body.height() * 0.46;
                        let x = |frame| body.left() + relative_x(frame);
                        let painter = painter.with_clip_rect(Rect::from_min_max(
                            body.min,
                            Pos2::new(x(audio.frames), body.bottom()),
                        ));
                        let bin_width = relative_x(audio.bin_frames);
                        let peaks = &audio.peaks[route];
                        let first_bin = (((painter.clip_rect().left() - body.left()) / bin_width)
                            .floor() as usize)
                            .min(peaks.len());
                        let last_bin = (((painter.clip_rect().right() - body.left()) / bin_width)
                            .ceil() as usize)
                            .clamp(first_bin, peaks.len());
                        let bins_per_pixel = bin_width.recip().ceil().max(1.0) as usize;
                        for (index, bins) in peaks[first_bin..last_bin]
                            .chunks(bins_per_pixel)
                            .enumerate()
                        {
                            let index = first_bin + index * bins_per_pixel;
                            let [low, high] = bins
                                .iter()
                                .fold(waveform::EMPTY, |peak, &bin| waveform::merge(peak, bin));
                            let first = index as u64 * audio.bin_frames;
                            let last =
                                (first + bins.len() as u64 * audio.bin_frames).min(audio.frames);
                            let left = x(first);
                            let right = x(last);
                            let top = middle - high.clamp(-1.0, 1.0) * scale;
                            let bottom = middle - low.clamp(-1.0, 1.0) * scale;
                            painter.rect_filled(
                                Rect::from_min_max(
                                    Pos2::new(left, top),
                                    Pos2::new(right.max(left + 1.0), bottom.max(top + 1.0)),
                                ),
                                0.0,
                                color,
                            );
                        }
                    },
                );
            }
        }
        for track in &preview.midi {
            let track_end = if track.active {
                end
            } else {
                track.end.min(end)
            };
            let length = track_end.saturating_sub(preview.start);
            self.paint_recording_clip(
                ui,
                viewport,
                track.track,
                preview.start,
                length,
                |painter, body, color| {
                    if length == 0 || track.notes.is_empty() {
                        return;
                    }
                    let min = track
                        .notes
                        .iter()
                        .map(|note| note.pitch)
                        .min()
                        .unwrap_or(60);
                    let max = track
                        .notes
                        .iter()
                        .map(|note| note.pitch)
                        .max()
                        .unwrap_or(72)
                        .max(min.saturating_add(12));
                    let x =
                        |frame: u64| body.left() + relative_x(frame.saturating_sub(preview.start));
                    for note in &track.notes {
                        if note.start >= track_end {
                            continue;
                        }
                        let note_end = if note.held {
                            track_end
                        } else {
                            note.end.min(track_end)
                        };
                        let left = x(note.start);
                        let y = egui::lerp(
                            (body.bottom() - 2.0)..=(body.top() + 2.0),
                            f32::from(note.pitch - min) / f32::from(max - min),
                        );
                        painter.hline(
                            left..=x(note_end).max(left + 1.0),
                            y,
                            Stroke::new(2.0, color),
                        );
                    }
                },
            );
        }
    }

    fn paint_recording_clip(
        &self,
        ui: &egui::Ui,
        viewport: Rect,
        track: Id,
        start: u64,
        length: u64,
        paint_content: impl FnOnce(&egui::Painter, Rect, Color32),
    ) {
        let Some(&lane) = self.lane_bounds.get(&track) else {
            return;
        };
        let visible = viewport.intersect(lane);
        let block = self.clip_frame_block(lane, start, length);
        if !visible.is_positive() || !visible.intersects(block) {
            return;
        }
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|candidate| candidate.id == track)
        else {
            return;
        };
        let painter = ui.painter_at(visible.intersect(block));
        let colors = theme::clip_colors(track.color, None);
        painter.rect_filled(block, 2.0, colors.body);
        let header = clip_paint::paint_header(&painter, block, "Recording", colors, false, false);
        let body = Rect::from_min_max(Pos2::new(block.left(), header.bottom()), block.max);
        if body.is_positive() {
            paint_content(&painter.with_clip_rect(body), body, colors.waveform);
        }
        painter.rect_stroke(
            block,
            2.0,
            Stroke::new(1.0, theme::accent(ui.ctx())),
            egui::StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_preview_keeps_source_peaks_when_its_history_compacts() {
        let tracks = vec![(Id::new_v4(), 0), (Id::new_v4(), 0)];
        let mut builder = AudioPreviewBuilder::new(tracks.clone(), 2);
        for frame in 0..MAX_PEAKS * 64 + CHUNK_FRAMES {
            let sample = if frame == 74 { [-0.75, 0.5] } else { [0.0; 2] };
            builder.push(&[sample, [0.25; 2]]);
        }
        let preview = builder.snapshot();
        assert_eq!(preview.frames, (MAX_PEAKS * 64 + CHUNK_FRAMES) as u64);
        assert_eq!(preview.tracks, tracks);
        assert_eq!(preview.bin_frames, 128);
        assert_eq!(preview.peaks[0].len(), MAX_PEAKS / 2 + CHUNK_FRAMES / 128);
        assert_eq!(preview.peaks[0][0], [-0.75, 0.5]);
        assert_eq!(preview.peaks[1][0], [0.25; 2]);
        for _ in 0..CHUNK_FRAMES - 1 {
            builder.push(&[[-0.8, 0.2], [0.0; 2]]);
        }
        assert!(Arc::ptr_eq(&preview, &builder.snapshot()));
        builder.push(&[[0.0; 2], [0.0; 2]]);
        let next = builder.snapshot();
        assert_eq!(next.frames, preview.frames + CHUNK_FRAMES as u64);
        assert_eq!(next.peaks[0][preview.peaks[0].len()], [-0.8, 0.2]);
    }

    #[test]
    fn mixed_preview_paints_both_lanes_without_editing_the_project() {
        let mut app = DawUi::default();
        let audio = app.session.project.add_track().unwrap();
        let midi = app.session.project.add_midi_track().unwrap();
        app.midi.recording = true;
        let audio_lane = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(400.0, 60.0));
        let midi_lane = audio_lane.translate(Vec2::new(0.0, 60.0));
        app.lane_bounds.insert(audio, audio_lane);
        app.lane_bounds.insert(midi, midi_lane);
        let mut waveform = AudioPreviewBuilder::new(vec![(audio, 0)], 1);
        for _ in 0..4096 {
            waveform.push(&[[-0.4, 0.4]]);
        }
        app.midi.preview = Some(Box::new(TakePreview {
            project: app.session.project.project_id,
            start: 0,
            audio: Some(waveform.snapshot()),
            midi: vec![TrackPreview {
                track: midi,
                active: true,
                end: 4_096,
                notes: vec![daw_project::midi_recording::PreviewNote {
                    start: 1000,
                    end: 4_096,
                    pitch: 65,
                    held: true,
                }],
            }],
            sealed_at: None,
        }));
        app.session.project.transport.playhead_frame = 12_000;
        app.advance_recording_preview();
        let original = app.session.project.clone();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.paint_recording_preview(ui, audio_lane.union(midi_lane));
        });
        output.textures_delta.clear();
        for lane in [audio_lane, midi_lane] {
            assert!(output.shapes.iter().any(|shape| {
                shape.clip_rect.intersects(lane)
                    && matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == "Recording")
            }));
        }
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect.top() > audio_lane.top() + clip_paint::HEADER_HEIGHT && rect.rect.height() > 10.0 && rect.rect.bottom() < audio_lane.bottom())
        }));
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if midi_lane.contains(points[0]) && stroke.width == 2.0)
        }));
        assert_eq!(app.session.project, original);
        assert!(!app.dirty);

        let waveform_end = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.top() > audio_lane.top() + clip_paint::HEADER_HEIGHT
                            && rect.rect.height() > 10.0
                            && rect.rect.bottom() < audio_lane.bottom() =>
                    {
                        Some(rect.rect.right())
                    }
                    _ => None,
                })
                .fold(0.0, f32::max)
        };
        app.session.project.transport.playhead_frame = 24_000;
        app.advance_recording_preview();
        let mut later = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.paint_recording_preview(ui, audio_lane.union(midi_lane));
        });
        later.textures_delta.clear();
        let expected_edge = app.clip_frame_block(audio_lane, 0, 24_000).right();
        for lane in [audio_lane, midi_lane] {
            assert!(later.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect.top() == lane.top() && rect.rect.bottom() == lane.bottom() && rect.rect.right() == expected_edge)
            }));
        }
        assert_eq!(waveform_end(&output.shapes), waveform_end(&later.shapes));
        assert!(waveform_end(&later.shapes) < expected_edge);
        assert!(later.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if midi_lane.contains(points[0]) && stroke.width == 2.0 && points[1].x == expected_edge)
        }));
        app.midi.finalizing = true;
        app.session.project.transport.playhead_frame = 48_000;
        app.advance_recording_preview();
        assert_eq!(app.recording_preview_end(), 24_000);
    }
}
