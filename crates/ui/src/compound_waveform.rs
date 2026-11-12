//! Sparse compound waveforms. Painting reads prepared clocks and cached peaks only.
use super::*;
impl DawUi {
    pub(super) fn paint_compound_peaks(
        &self,
        painter: &egui::Painter,
        body: Rect,
        clip: &Clip,
        colors: theme::ClipColors,
    ) {
        let fade = self.waveform_fade_gain(clip, painter.ctx());
        self.paint_compound_peaks_with_gain(painter, body, clip, colors, 0, fade);
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_compound_peaks_with_gain(
        &self,
        painter: &egui::Painter,
        body: Rect,
        clip: &Clip,
        colors: theme::ClipColors,
        depth: usize,
        fade: impl Fn(u64) -> f32,
    ) {
        let painter = painter.with_clip_rect(body.intersect(painter.clip_rect()));
        let frames_per_pixel =
            f64::from(self.session.project.sample_rate_hz) / f64::from(self.view_zoom());
        for channel in 0..2 {
            let height = body.height() / 2.0;
            let center = body.top() + height * (channel as f32 + 0.5);
            for pixel in body.left().max(painter.clip_rect().left()) as i32
                ..=body.right().min(painter.clip_rect().right()) as i32
            {
                let local = ((f64::from(pixel) - f64::from(body.left())) * frames_per_pixel)
                    .max(0.0) as u64;
                let peak = self.compound_clip_extrema(
                    clip,
                    local,
                    frames_per_pixel.ceil() as u64 + 1,
                    channel,
                    depth,
                );
                let scale = fade(local);
                painter.line_segment(
                    [
                        egui::pos2(
                            pixel as f32,
                            center - (peak[1] * scale).clamp(-1.0, 1.0) * height * 0.42,
                        ),
                        egui::pos2(
                            pixel as f32,
                            center - (peak[0] * scale).clamp(-1.0, 1.0) * height * 0.42,
                        ),
                    ],
                    Stroke::new(1.0, colors.waveform),
                );
            }
        }
    }
    pub(super) fn compound_clip_extrema(
        &self,
        clip: &Clip,
        local: u64,
        count: u64,
        channel: usize,
        depth: usize,
    ) -> [f32; 2] {
        if depth >= 64
            || (depth > 0
                && (clip.muted
                    || !(if channel == 0 {
                        clip.channels.left
                    } else {
                        clip.channels.right
                    })))
        {
            return [0.0; 2];
        }
        let input_channel = if depth > 0 && clip.channels.swapped {
            1 - channel
        } else {
            channel
        };
        let mut result = [0.0_f32; 2];
        if let Some(audio) = self.session.audio_for_clip(clip) {
            result = waveforms::extrema(audio, clip, local, count, input_channel);
        } else if let Some(view) = self.session.compound_views.get(&clip.asset_id) {
            for (start, length) in waveforms::source_windows(clip, local, count) {
                if length == 0 {
                    continue;
                }
                let (start, end) = if let Some(stretch) = clip.stretch {
                    (
                        stretch.original_frame(start),
                        stretch
                            .original_frame(start.saturating_add(length - 1))
                            .saturating_add(1),
                    )
                } else {
                    (start, start.saturating_add(length))
                };
                let first = view.track.clips.partition_point(|c| c.end() <= start);
                for (index, member) in view
                    .track
                    .clips
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take_while(|(_, c)| c.start_frame < end)
                {
                    let from = start.max(member.start_frame);
                    let to = end.min(member.end());
                    let peak = self.compound_clip_extrema(
                        member,
                        from - member.start_frame,
                        to - from,
                        input_channel,
                        depth + 1,
                    );
                    let mut scale = 1.0;
                    for clock in &view.envelopes[index] {
                        scale *= clock.fades.gain(
                            clock.offset + from - member.start_frame,
                            clock.length_frames,
                            self.session.project.sample_rate_hz,
                        );
                    }
                    result[0] = result[0].min(peak[0] * scale);
                    result[1] = result[1].max(peak[1] * scale);
                }
            }
        }
        // Match ordinary source waveforms: wrapper controls do not alter the source preview.
        let scale = if depth == 0 {
            1.0
        } else {
            daw_core::linear_gain(clip.gain_db) * if clip.channels.inverted { -1.0 } else { 1.0 }
        };
        if scale < 0.0 {
            [result[1] * scale, result[0] * scale]
        } else {
            [result[0] * scale, result[1] * scale]
        }
    }
}

pub(super) enum ContinuationSource<'a> {
    Audio(&'a daw_media::AudioData, Option<daw_core::ClipStretch>),
    Compound,
}
pub(super) struct ContinuationWaveform<'a> {
    pub clip: Clip,
    pub source: ContinuationSource<'a>,
    pub gain: f32,
    pub channels: daw_core::ClipChannels,
}
impl DawUi {
    /// Resolve an exposed window from prepared clocks, without decoding or baking PCM.
    pub(super) fn continuation_waveform(
        &self,
        owner: &Clip,
        left: bool,
        length: u64,
    ) -> Option<ContinuationWaveform<'_>> {
        let start = if left {
            owner.start_frame.checked_sub(length)?
        } else {
            owner.end()
        };
        let mut local = if left {
            -i128::from(length)
        } else {
            i128::from(owner.length_frames)
        };
        let mut current = owner.clone();
        let mut gain_db = 0.0;
        let mut channels = daw_core::ClipChannels::default();
        for depth in 0..64 {
            if depth > 0 {
                if current.muted {
                    return None;
                }
                gain_db += current.gain_db;
                channels = current.channels.then(channels);
            }
            let cached = self.session.audio_for_clip(&current);
            let context = daw_media::prepared_clip_audio(
                &current,
                &self.session.audio,
                &self.session.stretched_audio,
                self.session.project.stretch_engine,
            )
            .and_then(|p| p.context.as_ref());
            if let Some(audio) = context
                .map(|c| &c.audio)
                .or(cached)
                .or_else(|| self.session.audio.get(&current.asset_id))
            {
                let prefix = context.map_or(0, |c| c.start_frame);
                let mut visual = current.clone();
                if let Some(repeat) = current.repeat {
                    visual.repeat = Some(repeat.shifted(local));
                    visual.source_offset_frame = visual.source_offset_frame.checked_add(prefix)?;
                } else {
                    visual.source_offset_frame = u64::try_from(
                        i128::from(current.source_offset_frame) + local + i128::from(prefix),
                    )
                    .ok()?;
                    if cached.is_some()
                        && visual.source_offset_frame.checked_add(length)? > audio.frame_count()
                    {
                        return None;
                    }
                }
                visual.start_frame = start;
                visual.length_frames = length;
                return Some(ContinuationWaveform {
                    clip: visual,
                    source: ContinuationSource::Audio(
                        audio,
                        if cached.is_none() {
                            current.stretch
                        } else {
                            None
                        },
                    ),
                    gain: daw_core::linear_gain(gain_db),
                    channels,
                });
            }
            let view = self.session.compound_views.get(&current.asset_id)?;
            let point = current.extended_source_frame(local).map_or_else(
                || i128::from(current.source_offset_frame) + local,
                i128::from,
            );
            let size = self
                .session
                .project
                .audio_asset(current.asset_id)?
                .decoded_frame_count;
            if current.repeat.is_some()
                || (point >= 0 && point + i128::from(length) <= i128::from(size))
            {
                if let Some(repeat) = current.repeat {
                    current.repeat = Some(repeat.shifted(local));
                } else {
                    current.source_offset_frame = u64::try_from(point).ok()?;
                }
                current.start_frame = start;
                current.length_frames = length;
                current.gain_db = gain_db;
                current.channels = channels;
                current.muted = false;
                return Some(ContinuationWaveform {
                    clip: current,
                    source: ContinuationSource::Compound,
                    gain: 1.0,
                    channels: Default::default(),
                });
            }
            let leaf = if point < 0 {
                view.track.clips.first()?
            } else {
                view.track.clips.last()?
            };
            local = point - i128::from(leaf.start_frame);
            current = leaf.clone();
        }
        None
    }
}
