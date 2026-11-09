//! MIDI edge edits retain the source and all hidden notes. Only expansion pads a source.
use crate::{Clip, Error, Id, MIDI_TICKS_PER_BEAT, Project, Result};

fn range_error() -> Error {
    Error("MIDI resize exceeds source or timeline bounds".into())
}

impl Project {
    pub(crate) fn apply_midi_pattern_resize(
        &mut self,
        clip_id: Id,
        start_tick: i64,
        end_tick: i64,
    ) -> Result<()> {
        if end_tick.checked_sub(start_tick).is_none_or(|n| n <= 0) {
            return Err(range_error());
        }
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
        let pattern = original.midi_pattern.as_ref().unwrap();
        pattern.validate()?;
        if start_tick == 0 && end_tick == i64::from(pattern.length_ticks) {
            return Ok(());
        }
        // Arguments describe changes at the current visible edges; the saved
        // source length remains the right-edge tick anchor after a trim.
        let right_ticks = end_tick
            .checked_sub(i64::from(pattern.length_ticks))
            .ok_or_else(range_error)?;
        let delta = |ticks: i64| -> Result<i128> {
            let magnitude = u32::try_from(ticks.unsigned_abs()).map_err(|_| range_error())?;
            let frames = clip
                .source_length(self.midi_frame(magnitude))
                .ok_or_else(range_error)?;
            Ok(if ticks < 0 {
                -i128::from(frames)
            } else {
                i128::from(frames)
            })
        };
        let left = delta(start_tick)?;
        let right = delta(right_ticks)?;
        let base = clip.repeat.map_or(clip.length_frames, |r| r.length_frames);
        let length = u64::try_from(i128::from(base) + right - left)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(range_error)?;
        let full_length = u64::try_from(i128::from(clip.length_frames) + right - left)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(range_error)?;
        let start_frame =
            u64::try_from(i128::from(clip.start_frame) + left).map_err(|_| range_error())?;
        let frames_per_tick = f64::from(self.sample_rate_hz) * 60.0
            / (f64::from(self.tempo_bpm) * f64::from(MIDI_TICKS_PER_BEAT))
            * clip.stretch.map_or(1.0, |s| s.ratio());
        let offset = i128::from(clip.source_offset_frame) + left;
        let padding_ticks = if offset < 0 {
            self.midi_padding_ticks(
                &clip,
                u64::try_from(-offset).map_err(|_| range_error())?,
                frames_per_tick,
            )?
        } else {
            0
        };
        let padding_frames = clip
            .source_length(self.midi_frame(padding_ticks))
            .ok_or_else(range_error)?;
        let source_offset =
            u64::try_from(offset + i128::from(padding_frames)).map_err(|_| range_error())?;
        let required = source_offset.checked_add(length).ok_or_else(range_error)?;
        let mut source_ticks = pattern
            .length_ticks
            .checked_add(padding_ticks)
            .ok_or_else(range_error)?;
        let mut available = clip
            .source_length(self.midi_frame(source_ticks))
            .ok_or_else(range_error)?;
        if required > available {
            let extra = self.midi_padding_ticks(&clip, required - available, frames_per_tick)?;
            source_ticks = source_ticks.checked_add(extra).ok_or_else(range_error)?;
            available = clip
                .source_length(self.midi_frame(source_ticks))
                .ok_or_else(range_error)?;
            while available < required {
                source_ticks = source_ticks.checked_add(1).ok_or_else(range_error)?;
                available = clip
                    .source_length(self.midi_frame(source_ticks))
                    .ok_or_else(range_error)?;
            }
        }
        if source_ticks != pattern.length_ticks || padding_ticks != 0 {
            // Keep every note, including hidden notes and complete crossing gates.
            // Prepending silence shifts source ticks, never project note timing.
            let mut padded = pattern.clone();
            padded.length_ticks = source_ticks;
            for note in &mut padded.notes {
                note.start_tick = note
                    .start_tick
                    .checked_add(padding_ticks)
                    .ok_or_else(range_error)?;
            }
            let asset = self.midi_asset(original.name.clone(), padded)?;
            clip.asset_id = asset.id;
            self.assets.push(asset);
        }
        clip.start_frame = start_frame;
        clip.source_offset_frame = source_offset;
        if let Some(repeat) = &mut clip.repeat {
            repeat.length_frames = length;
            repeat.phase_frame = repeat.phase_frame.min(length - 1);
            clip.length_frames = full_length.max(length);
        } else {
            clip.length_frames = full_length;
        }
        self.apply_placements(&[crate::ClipPlacement { track_id, clip }])
    }

    fn midi_padding_ticks(&self, clip: &Clip, frames: u64, frames_per_tick: f64) -> Result<u32> {
        let estimate = (frames as f64 / frames_per_tick).floor().max(1.0);
        if !estimate.is_finite() || estimate > f64::from(u32::MAX) {
            return Err(range_error());
        }
        let mut ticks = estimate as u32;
        while clip
            .source_length(self.midi_frame(ticks))
            .ok_or_else(range_error)?
            < frames
        {
            ticks = ticks.checked_add(1).ok_or_else(range_error)?;
        }
        Ok(ticks)
    }
}
