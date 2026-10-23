use crate::{Asset, Clip, ClipEdge, Edit, Error, Id, Project, Result, Source, SourceMetadata};
use serde::{Deserialize, Serialize};

pub const MIDI_TICKS_PER_BEAT: u32 = 960;

/// Map a visible MIDI clip edge to source ticks without limiting expansion to its source.
pub struct MidiPatternBounds {
    edge: ClipEdge,
    edge_frame: u64,
    length_ticks: u32,
    frames_per_tick: f64,
    min_delta: i64,
    max_delta: i64,
}
impl MidiPatternBounds {
    pub fn edge_frame(&self) -> u64 {
        self.edge_frame
    }
    pub fn length_ticks(&self) -> u32 {
        self.length_ticks
    }
    fn frame_at_delta(&self, delta: i64) -> u64 {
        (i128::from(self.edge_frame) + (delta as f64 * self.frames_per_tick).round() as i128)
            .clamp(0, i128::from(u64::MAX)) as u64
    }
    pub fn frame_limits(&self) -> std::ops::RangeInclusive<u64> {
        self.frame_at_delta(self.min_delta)..=self.frame_at_delta(self.max_delta)
    }
    pub fn ticks_at_frame(&self, frame: u64) -> (i64, i64) {
        let delta = ((i128::from(frame) - i128::from(self.edge_frame)) as f64
            / self.frames_per_tick)
            .round() as i64;
        let delta = delta.clamp(self.min_delta, self.max_delta);
        match self.edge {
            ClipEdge::Left => (delta, i64::from(self.length_ticks)),
            ClipEdge::Right => (0, i64::from(self.length_ticks) + delta),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    #[default]
    Audio,
    Midi,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiNote {
    pub id: Id,
    pub start_tick: u32,
    pub length_ticks: u32,
    pub pitch: u8,
    pub velocity: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPattern {
    pub length_ticks: u32,
    pub notes: Vec<MidiNote>,
}
impl Default for MidiPattern {
    fn default() -> Self {
        Self {
            length_ticks: MIDI_TICKS_PER_BEAT,
            notes: Vec::new(),
        }
    }
}
impl MidiPattern {
    /// Incoming notes win only on their own pitch. Preserve other pitches for chords.
    pub fn with_overwritten_note(&self, note: MidiNote) -> Result<Self> {
        self.validate()?;
        let mut next = Self {
            length_ticks: self.length_ticks,
            notes: vec![note.clone()],
        };
        next.validate()?;
        let start = note.start_tick;
        let end = start + note.length_ticks;
        for original in &self.notes {
            if original.id == note.id {
                continue;
            }
            let original_end = original.start_tick + original.length_ticks;
            if original.pitch != note.pitch || original_end <= start || original.start_tick >= end {
                next.notes.push(original.clone());
                continue;
            }
            let left = original.start_tick < start;
            if left {
                next.notes.push(MidiNote {
                    length_ticks: start - original.start_tick,
                    ..original.clone()
                });
            }
            if original_end > end {
                next.notes.push(MidiNote {
                    id: if left { Id::new_v4() } else { original.id },
                    start_tick: end,
                    length_ticks: original_end - end,
                    ..original.clone()
                });
            }
        }
        next.notes.sort_by_key(|n| (n.start_tick, n.pitch, n.id));
        next.validate()?;
        Ok(next)
    }

    pub fn validate(&self) -> Result<()> {
        if self.length_ticks == 0 {
            return Err(Error("MIDI pattern length must be positive".into()));
        }
        let mut ids = std::collections::HashSet::new();
        for note in &self.notes {
            if !ids.insert(note.id)
                || note.length_ticks == 0
                || note.pitch > 127
                || !(1..=127).contains(&note.velocity)
                || note
                    .start_tick
                    .checked_add(note.length_ticks)
                    .is_none_or(|end| end > self.length_ticks)
            {
                return Err(Error(
                    "Invalid MIDI note ID, range, pitch, or velocity".into(),
                ));
            }
        }
        Ok(())
    }
}
impl Project {
    pub fn midi_pattern_bounds(&self, clip: &Clip, edge: ClipEdge) -> Result<MidiPatternBounds> {
        let pattern = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .and_then(|a| a.midi_pattern.as_ref())
            .ok_or_else(|| Error("Clip is not a MIDI pattern".into()))?;
        let stretch = clip
            .stretch
            .map_or(1.0, |s| s.output_frames as f64 / s.source_frames as f64);
        let frames_per_tick = f64::from(self.sample_rate_hz) * 60.0
            / (f64::from(self.tempo_bpm) * f64::from(MIDI_TICKS_PER_BEAT))
            * stretch;
        let visible_ticks = (clip.length_frames as f64 / frames_per_tick)
            .floor()
            .max(1.0) as i64;
        let length = i64::from(pattern.length_ticks);
        let inward = (length - 1).min(visible_ticks - 1);
        let edge_frame = match edge {
            ClipEdge::Left => clip.start_frame,
            ClipEdge::Right => clip.end(),
        };
        let (min_delta, max_delta) = match edge {
            ClipEdge::Left => (
                (length - i64::from(u32::MAX))
                    .max(-((clip.start_frame as f64 / frames_per_tick).floor() as i64)),
                inward,
            ),
            ClipEdge::Right => (
                -inward,
                (i64::from(u32::MAX) - length)
                    .min(((u64::MAX - edge_frame) as f64 / frames_per_tick).floor() as i64),
            ),
        };
        Ok(MidiPatternBounds {
            edge,
            edge_frame,
            length_ticks: pattern.length_ticks,
            frames_per_tick,
            min_delta,
            max_delta,
        })
    }

    /// Body trims resize MIDI content while audio retains ordinary source-bound trimming.
    /// Prepare each selected source from the original project, then resolve neighbors once.
    pub fn trimmed_clips(&self, clips: &[Clip], edge: ClipEdge) -> Result<Self> {
        let mut next = self.clone();
        let mut resized = Vec::with_capacity(clips.len());
        for candidate in clips {
            let original = self
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| c.id == candidate.id)
                .ok_or_else(|| Error("Clip not found".into()))?;
            if match edge {
                ClipEdge::Left => candidate.end() != original.end(),
                ClipEdge::Right => candidate.start_frame != original.start_frame,
            } {
                return Err(Error("Resize must keep the opposite endpoint fixed".into()));
            }
            if self
                .assets
                .iter()
                .find(|a| a.id == original.asset_id)
                .is_some_and(|a| a.midi_pattern.is_some())
            {
                let bounds = self.midi_pattern_bounds(original, edge)?;
                let target = match edge {
                    ClipEdge::Left => candidate.start_frame,
                    ClipEdge::Right => candidate.end(),
                };
                let (start, end) = bounds.ticks_at_frame(target);
                let source = self.resized_midi_pattern(original.id, start, end)?;
                let clip = source
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .find(|c| c.id == original.id)
                    .unwrap()
                    .clone();
                if clip.asset_id != original.asset_id {
                    next.assets.push(
                        source
                            .assets
                            .iter()
                            .find(|a| a.id == clip.asset_id)
                            .unwrap()
                            .clone(),
                    );
                    next.tracks
                        .iter_mut()
                        .flat_map(|t| &mut t.clips)
                        .find(|c| c.id == clip.id)
                        .unwrap()
                        .asset_id = clip.asset_id;
                }
                resized.push(clip);
            } else {
                resized.push(candidate.clone());
            }
        }
        next.apply_pattern_resizes(&resized, edge)?;
        next.validate()?;
        Ok(next)
    }

    /// Resize the edited source and its clip as one transaction. Other source users stay intact.
    pub fn resized_midi_pattern(
        &self,
        clip_id: Id,
        start_tick: i64,
        end_tick: i64,
    ) -> Result<Self> {
        let mut next = self.clone();
        next.edit(Edit::ResizeMidiPattern {
            clip_id,
            start_tick,
            end_tick,
        })?;
        Ok(next)
    }

    pub(crate) fn apply_midi_pattern_resize(
        &mut self,
        clip_id: Id,
        start_tick: i64,
        end_tick: i64,
    ) -> Result<()> {
        let length_ticks = end_tick
            .checked_sub(start_tick)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| Error("Pattern length exceeds supported bounds".into()))?;
        let (track_id, mut clip) = self
            .tracks
            .iter()
            .find_map(|track| {
                track
                    .clips
                    .iter()
                    .find(|c| c.id == clip_id)
                    .map(|c| (track.id, c.clone()))
            })
            .ok_or_else(|| Error("Clip not found".into()))?;
        let original = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id && a.midi_pattern.is_some())
            .ok_or_else(|| Error("Clip is not a MIDI pattern".into()))?;
        let old_pattern = original.midi_pattern.as_ref().unwrap();
        old_pattern.validate()?;
        if start_tick == 0 && end_tick == i64::from(old_pattern.length_ticks) {
            return Ok(());
        }
        let notes = old_pattern
            .notes
            .iter()
            .filter_map(|note| {
                let start = i64::from(note.start_tick).max(start_tick);
                let end = i64::from(note.start_tick + note.length_ticks).min(end_tick);
                (end > start).then(|| MidiNote {
                    start_tick: (start - start_tick) as u32,
                    length_ticks: (end - start) as u32,
                    ..note.clone()
                })
            })
            .collect();
        let old_available = clip
            .source_length(original.decoded_frame_count)
            .ok_or_else(|| Error("Stretched MIDI source exceeds supported bounds".into()))?;
        let previous_length = clip.repeat.map_or(clip.length_frames, |r| r.length_frames);
        let ended_at_source =
            clip.source_offset_frame.checked_add(previous_length) == Some(old_available);
        let asset = self.midi_asset(
            original.name.clone(),
            MidiPattern {
                length_ticks,
                notes,
            },
        )?;
        let available = clip
            .source_length(asset.decoded_frame_count)
            .filter(|n| *n > 0)
            .ok_or_else(|| Error("Pattern resize would make the clip empty".into()))?;
        // Move the source origin with its left boundary. Subtracting start_tick
        // from retained notes then preserves their project timing (within frame rounding).
        let start_ticks = u32::try_from(start_tick.unsigned_abs())
            .map_err(|_| Error("Pattern start exceeds supported bounds".into()))?;
        let shift = self.midi_frame(start_ticks);
        let shift = clip
            .stretch
            .map_or(Some(shift), |s| s.scale(shift))
            .ok_or_else(|| Error("Pattern start exceeds supported bounds".into()))?;
        clip.start_frame = if start_tick < 0 {
            clip.start_frame.checked_sub(shift)
        } else {
            clip.start_frame.checked_add(shift)
        }
        .ok_or_else(|| Error("Pattern resize exceeds timeline bounds".into()))?;
        clip.asset_id = asset.id;
        clip.source_offset_frame = clip.source_offset_frame.min(available - 1);
        let remaining = available - clip.source_offset_frame;
        let length = if ended_at_source {
            remaining
        } else {
            // A trimmed clip's visible edge follows the same requested duration
            // change as the source, without exposing unavailable source frames.
            (i128::from(previous_length) + i128::from(available) - i128::from(old_available))
                .clamp(1, i128::from(remaining)) as u64
        };
        if let Some(repeat) = &mut clip.repeat {
            repeat.length_frames = length;
            repeat.phase_frame = repeat.phase_frame.min(length - 1);
            clip.length_frames = u64::try_from(
                i128::from(clip.length_frames) + i128::from(available) - i128::from(old_available),
            )
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| Error("Pattern resize would make the clip empty".into()))?
            .max(length);
        } else {
            clip.length_frames = length;
        }
        self.assets.push(asset);
        self.apply_placements(&[crate::ClipPlacement { track_id, clip }])
    }

    pub(crate) fn bound_retimed_midi_sources(&mut self) -> Result<()> {
        for clip in self
            .tracks
            .iter_mut()
            .filter(|t| t.kind == TrackKind::Midi)
            .flat_map(|t| &mut t.clips)
        {
            let asset = self
                .assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .ok_or_else(|| Error("Clip references a missing asset record".into()))?;
            let available = clip
                .source_length(asset.decoded_frame_count)
                .ok_or_else(|| Error("Stretched MIDI source exceeds supported bounds".into()))?;
            // Source ticks and timeline frames round independently. Keep a full-source
            // range within its new bound instead of rejecting a valid tempo/rate change.
            clip.source_offset_frame = clip.source_offset_frame.min(available);
            let remaining = available - clip.source_offset_frame;
            if let Some(repeat) = &mut clip.repeat {
                repeat.length_frames = repeat.length_frames.min(remaining);
                repeat.phase_frame = repeat
                    .phase_frame
                    .min(repeat.length_frames.saturating_sub(1));
            } else {
                clip.length_frames = clip.length_frames.min(remaining);
            }
        }
        Ok(())
    }

    pub fn midi_frame(&self, tick: u32) -> u64 {
        (f64::from(tick) * f64::from(self.sample_rate_hz) * 60.0
            / (f64::from(self.tempo_bpm) * f64::from(MIDI_TICKS_PER_BEAT)))
        .round() as u64
    }
    pub fn add_midi_track(&mut self) -> Result<Id> {
        let mut next = self.clone();
        let id = next.add_track()?;
        let index = next.tracks.len();
        let track = next.tracks.last_mut().unwrap();
        track.kind = TrackKind::Midi;
        track.instrument = Some(Default::default());
        track.name = format!("MIDI {index}");
        next.validate()?;
        *self = next;
        Ok(id)
    }
    pub fn add_midi_pattern(&mut self, track_id: Id, start_frame: u64) -> Result<Id> {
        let mut next = self.clone();
        let id = Id::new_v4();
        let asset = next.midi_asset("MIDI pattern".into(), MidiPattern::default())?;
        let clip = Clip {
            muted: false,
            id,
            asset_id: asset.id,
            name: asset.name.clone(),
            color: None,
            start_frame,
            source_offset_frame: 0,
            length_frames: asset.decoded_frame_count,
            repeat: None,
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
        };
        next.assets.push(asset);
        next.edit(Edit::OverwriteClips(vec![crate::ClipPlacement {
            track_id,
            clip,
        }]))?;
        *self = next;
        Ok(id)
    }
    pub(crate) fn midi_asset(&self, name: String, pattern: MidiPattern) -> Result<Asset> {
        pattern.validate()?;
        let frames = self.midi_frame(pattern.length_ticks);
        if frames == 0 || frames == u64::MAX {
            return Err(Error("MIDI pattern exceeds supported timing bounds".into()));
        }
        Ok(Asset {
            id: Id::new_v4(),
            name,
            decoded_frame_count: frames,
            source: Source {
                kind: "midi_pattern".into(),
                path: String::new(),
                path_kind: "embedded".into(),
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: self.sample_rate_hz,
                channels: 1,
                sample_format: "midi".into(),
                bits_per_sample: 0,
            },
            midi_pattern: Some(pattern),
        })
    }
    pub(crate) fn replace_midi_pattern(&mut self, clip_id: Id, pattern: MidiPattern) -> Result<()> {
        let clip = self
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == clip_id)
            .ok_or_else(|| Error("Clip not found".into()))?;
        let original = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id && a.midi_pattern.is_some())
            .ok_or_else(|| Error("Clip is not a MIDI pattern".into()))?;
        if original.midi_pattern.as_ref() == Some(&pattern) {
            return Ok(());
        }
        let asset = self.midi_asset(original.name.clone(), pattern)?;
        // A pattern edit changes only this source reference. Placement and repeats stay intact.
        let id = asset.id;
        self.assets.push(asset);
        self.tracks
            .iter_mut()
            .flat_map(|t| &mut t.clips)
            .find(|c| c.id == clip_id)
            .unwrap()
            .asset_id = id;
        Ok(())
    }
}
