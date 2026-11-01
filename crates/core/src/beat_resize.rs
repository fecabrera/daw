//! Source-aware body-edge limits shared by Timeline and Editor.
use crate::{Asset, Clip, ClipEdge, Error, MIDI_TICKS_PER_BEAT, Project, Result};

const BAR_TICKS: u32 = 4 * MIDI_TICKS_PER_BEAT;
const MAX_BARS: u8 = 16;

pub struct PatternTrimBounds {
    edge_frame: u64,
    limits: std::ops::RangeInclusive<u64>,
}
impl PatternTrimBounds {
    pub fn edge_frame(&self) -> u64 {
        self.edge_frame
    }
    pub fn frame_limits(&self) -> std::ops::RangeInclusive<u64> {
        self.limits.clone()
    }
}

impl Project {
    /// MIDI retains its tick bounds; unlooped beats can grow up to their grid limit.
    pub fn pattern_trim_bounds(&self, clip: &Clip, edge: ClipEdge) -> Result<PatternTrimBounds> {
        if let Ok(bounds) = self.midi_pattern_bounds(clip, edge) {
            return Ok(PatternTrimBounds {
                edge_frame: bounds.edge_frame(),
                limits: bounds.frame_limits(),
            });
        }
        let source = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .and_then(|a| a.beat_pattern.as_ref())
            .filter(|_| clip.repeat.is_none())
            .ok_or_else(|| Error("Clip has no expandable pattern source".into()))?;
        source.validate()?;
        let scaled = |ticks| {
            clip.source_length(self.midi_frame(ticks))
                .ok_or_else(|| Error("Beat source exceeds timing bounds".into()))
        };
        let limits = match edge {
            ClipEdge::Right => {
                let length = scaled(u32::from(MAX_BARS) * BAR_TICKS)?
                    .checked_sub(clip.source_offset_frame)
                    .ok_or_else(|| Error("Beat source offset exceeds its bounds".into()))?;
                clip.start_frame.saturating_add(1)..=clip.start_frame.saturating_add(length)
            }
            ClipEdge::Left => {
                // Hidden steps must also fit after prepending bars. Never discard them.
                let occupied = source
                    .pattern
                    .steps
                    .iter()
                    .map(|s| s.start_tick + 1)
                    .max()
                    .unwrap_or(0)
                    .max(source.pattern.length_ticks())
                    .div_ceil(BAR_TICKS);
                let padding = scaled(
                    (u32::from(source.pattern.bars) + u32::from(MAX_BARS) - occupied) * BAR_TICKS,
                )? - scaled(source.pattern.length_ticks())?;
                clip.start_frame
                    .saturating_sub(clip.source_offset_frame.saturating_add(padding))
                    ..=clip.end().saturating_sub(1)
            }
        };
        Ok(PatternTrimBounds {
            edge_frame: match edge {
                ClipEdge::Left => clip.start_frame,
                ClipEdge::Right => clip.end(),
            },
            limits,
        })
    }

    pub(crate) fn beat_trim_result(
        &self,
        original: &Clip,
        candidate: &Clip,
        edge: ClipEdge,
    ) -> Result<(Clip, Option<Asset>)> {
        let old = self
            .assets
            .iter()
            .find(|a| a.id == original.asset_id)
            .ok_or_else(|| Error("Beat source not found".into()))?;
        let source = old
            .beat_pattern
            .as_ref()
            .ok_or_else(|| Error("Clip is not a beat pattern".into()))?;
        // Looped clips retain the existing trim contract; header edges extend repetitions.
        if original.repeat.is_some() {
            return Ok((candidate.clone(), None));
        }
        let bounds = self.pattern_trim_bounds(original, edge)?;
        let target = match edge {
            ClipEdge::Left => candidate.start_frame,
            ClipEdge::Right => candidate.end(),
        };
        if candidate.length_frames == 0 || !bounds.frame_limits().contains(&target) {
            return Err(Error(
                "Beat resize exceeds source or timeline bounds".into(),
            ));
        }
        let mut clip = original.clone();
        clip.start_frame = candidate.start_frame;
        clip.length_frames = candidate.length_frames;
        let mut offset = i128::from(original.source_offset_frame);
        if edge == ClipEdge::Left {
            offset += i128::from(candidate.start_frame) - i128::from(original.start_frame);
        }
        let mut pattern = source.pattern.clone();
        if offset < 0 {
            // Prepend whole bars to keep every stored hit on its original step grid.
            // The extra fractional-bar padding stays outside the clip via its offset.
            let old_frames = original
                .source_length(old.decoded_frame_count)
                .ok_or_else(|| Error("Beat source exceeds timing bounds".into()))?;
            let padding_frames = |bars| {
                original
                    .source_length(
                        self.midi_frame((u32::from(pattern.bars) + u32::from(bars)) * BAR_TICKS),
                    )
                    .and_then(|frames| frames.checked_sub(old_frames))
            };
            let padding = (1..=MAX_BARS - pattern.bars)
                .find(|bars| {
                    padding_frames(*bars).is_some_and(|frames| i128::from(frames) >= -offset)
                })
                .ok_or_else(|| Error("Beat resize exceeds 16 bars".into()))?;
            let ticks = u32::from(padding) * BAR_TICKS;
            offset += i128::from(
                padding_frames(padding)
                    .ok_or_else(|| Error("Beat padding exceeds timing bounds".into()))?,
            );
            for step in &mut pattern.steps {
                step.start_tick = step
                    .start_tick
                    .checked_add(ticks)
                    .ok_or_else(|| Error("Beat step exceeds timing bounds".into()))?;
            }
            pattern.bars += padding;
        }
        clip.source_offset_frame = u64::try_from(offset)
            .map_err(|_| Error("Beat source offset exceeds timing bounds".into()))?;
        let required = clip
            .source_offset_frame
            .checked_add(clip.length_frames)
            .ok_or_else(|| Error("Beat resize exceeds timing bounds".into()))?;
        while original
            .source_length(self.midi_frame(pattern.length_ticks()))
            .is_none_or(|frames| frames < required)
        {
            if pattern.bars == MAX_BARS {
                return Err(Error("Beat resize exceeds 16 bars".into()));
            }
            pattern.bars += 1;
        }
        if pattern == source.pattern {
            return Ok((clip, None));
        }
        let asset = self.beat_asset(source.instrument_id, old.name.clone(), pattern)?;
        clip.asset_id = asset.id;
        Ok((clip, Some(asset)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeatEdit, BeatPattern, BeatSubtrack, Edit, Id};
    fn fixture(rate: u32) -> (Project, Id, Id) {
        let mut p = Project {
            sample_rate_hz: rate,
            tempo_bpm: 123.0,
            ..Project::default()
        };
        let track = p.add_beat_track().unwrap();
        let row = BeatSubtrack::new("Kick".into());
        let row_id = row.id;
        p.edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
        let clip = p
            .add_beat_pattern(track, p.midi_frame(2 * BAR_TICKS))
            .unwrap();
        p.edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern: BeatPattern::default()
                .with_step(row_id, 960, Some(91))
                .unwrap(),
        })
        .unwrap();
        (p, track, clip)
    }
    #[test]
    fn right_extension_grows_only_the_edited_source_and_retains_frame_trimming() {
        let (mut p, track, _) = fixture(48_000);
        let old = p.tracks[0].clips[0].clone();
        let mut copy = old.clone();
        copy.id = Id::new_v4();
        copy.start_frame = p.midi_frame(8 * BAR_TICKS);
        p.edit(Edit::InsertClip {
            track_id: track,
            clip: copy.clone(),
        })
        .unwrap();
        let mut candidate = old.clone();
        candidate.length_frames += p.midi_frame(480);
        let next = p
            .trimmed_clips(&[candidate.clone()], ClipEdge::Right)
            .unwrap();
        let resized = &next.tracks[0].clips[0];
        assert_eq!(resized.length_frames, candidate.length_frames);
        assert_ne!(resized.asset_id, old.asset_id);
        assert_eq!(next.tracks[0].clips[1], copy);
        let source = next
            .assets
            .iter()
            .find(|a| a.id == resized.asset_id)
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap();
        assert_eq!(source.pattern.bars, 2);
        assert_eq!(
            source.pattern.steps,
            p.assets
                .iter()
                .find(|a| a.id == old.asset_id)
                .unwrap()
                .beat_pattern
                .as_ref()
                .unwrap()
                .pattern
                .steps
        );
        let mut trimmed = resized.clone();
        trimmed.length_frames /= 2;
        let shorter = next
            .trimmed_clips(&[trimmed.clone()], ClipEdge::Right)
            .unwrap();
        assert_eq!(shorter.tracks[0].clips[0], trimmed);
        assert_eq!(shorter.assets, next.assets);
        assert_eq!(p.tracks[0].clips[0], old);
    }
    #[test]
    fn left_extension_preserves_hit_timing_and_stretch_at_all_rates() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let (mut p, _, _) = fixture(rate);
            let scale = crate::ClipStretch::new(2, 3).unwrap();
            p.tracks[0].clips[0].stretch = Some(scale);
            p.tracks[0].clips[0].length_frames = scale.scale(p.midi_frame(BAR_TICKS)).unwrap();
            p.validate().unwrap();
            let old = p.tracks[0].clips[0].clone();
            let delta = p.midi_frame(320);
            let mut candidate = old.clone();
            candidate.start_frame -= delta;
            candidate.length_frames += delta;
            let next = p
                .trimmed_clips(&[candidate.clone()], ClipEdge::Left)
                .unwrap();
            let resized = &next.tracks[0].clips[0];
            assert_eq!(resized.start_frame, candidate.start_frame);
            assert_eq!(resized.end(), old.end());
            assert_eq!(resized.stretch, old.stretch);
            let source = next
                .assets
                .iter()
                .find(|a| a.id == resized.asset_id)
                .unwrap()
                .beat_pattern
                .as_ref()
                .unwrap();
            let step = &source.pattern.steps[0];
            assert_eq!(step.start_tick, BAR_TICKS + 960);
            let previous = old.start_frame + scale.scale(p.midi_frame(960)).unwrap();
            let current = resized.start_frame + scale.scale(p.midi_frame(step.start_tick)).unwrap()
                - resized.source_offset_frame;
            assert!(previous.abs_diff(current) <= 2);
            assert_eq!(source.pattern.bars, 2);
        }
    }
    #[test]
    fn extension_limits_keep_hidden_hits_and_neighbor_overwrites_atomic() {
        let (mut p, track, _) = fixture(48_000);
        let old = p.tracks[0].clips[0].clone();
        let mut neighbor = old.clone();
        neighbor.id = Id::new_v4();
        neighbor.start_frame = old.end();
        p.edit(Edit::InsertClip {
            track_id: track,
            clip: neighbor.clone(),
        })
        .unwrap();
        let mut candidate = old.clone();
        candidate.length_frames += p.midi_frame(960);
        let next = p.trimmed_clips(&[candidate], ClipEdge::Right).unwrap();
        assert_eq!(
            next.tracks[0].clips[1].start_frame,
            next.tracks[0].clips[0].end()
        );
        assert_eq!(next.tracks[0].clips[1].end(), neighbor.end());
        let bounds = p.pattern_trim_bounds(&old, ClipEdge::Right).unwrap();
        let mut excessive = old.clone();
        excessive.length_frames = bounds.frame_limits().end() - old.start_frame + 1;
        assert!(p.trimmed_clips(&[excessive], ClipEdge::Right).is_err());
        let source = p
            .assets
            .iter_mut()
            .find(|a| a.id == old.asset_id)
            .unwrap()
            .beat_pattern
            .as_mut()
            .unwrap();
        source.pattern.steps[0].start_tick = 15 * BAR_TICKS;
        let left = p.pattern_trim_bounds(&old, ClipEdge::Left).unwrap();
        assert_eq!(*left.frame_limits().start(), old.start_frame);
        p.validate().unwrap();
    }
}
