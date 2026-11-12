//! Beat Merge retains the common grid, stretch and stable subtrack bindings.
use crate::split::note_occurrences;
use crate::{
    BeatPattern, BeatPatternSource, Clip, ClipLoop, Error, Id, MIDI_TICKS_PER_BEAT, MidiNote,
    MidiPattern, Project, Result,
};
use std::collections::{HashMap, HashSet};

const BAR_TICKS: u32 = 4 * MIDI_TICKS_PER_BEAT;
const MAX_HITS: usize = 4096;

impl Project {
    fn merge_beat_source(&self, clip: &Clip) -> Result<&BeatPatternSource> {
        self.assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .and_then(|asset| asset.beat_pattern.as_ref())
            .ok_or_else(|| Error("Merge requires beat patterns".into()))
    }

    // Work in the common source clock so the saved subdivision still owns gate length.
    fn merge_beat_tick(&self, clip: &Clip, frames: u64) -> Result<u32> {
        let ratio = clip.stretch.map_or(1.0, |s| s.ratio());
        let ticks =
            frames as f64 / ratio * f64::from(self.tempo_bpm) * f64::from(MIDI_TICKS_PER_BEAT)
                / (f64::from(self.sample_rate_hz) * 60.0);
        if !ticks.is_finite() || ticks.round() > f64::from(16 * BAR_TICKS) {
            return Err(Error("Merged beat pattern exceeds 16 source bars".into()));
        }
        Ok(ticks.round() as u32)
    }

    pub(crate) fn beat_merge_settings(&self, clips: &[&Clip]) -> Result<(Id, u8)> {
        let first = clips[0];
        let source = self.merge_beat_source(first)?;
        let step_ticks = source.pattern.resolution.step_ticks();
        for clip in clips {
            let current = self.merge_beat_source(clip)?;
            if current.instrument_id != source.instrument_id
                || current.pattern.resolution != source.pattern.resolution
                || clip.stretch != first.stretch
            {
                return Err(Error("Merge requires beat patterns with matching subdivision, stretch and instrument".into()));
            }
            let mut frames = vec![
                clip.start_frame - first.start_frame,
                clip.length_frames,
                clip.source_offset_frame,
            ];
            if let Some(repeat) = clip.repeat {
                frames.extend([repeat.length_frames, repeat.phase_frame]);
            }
            if frames.into_iter().any(|frames| {
                self.merge_beat_tick(first, frames)
                    .map_or(true, |ticks| !ticks.is_multiple_of(step_ticks))
            }) {
                return Err(Error("Beat Merge requires grid-aligned clip bounds, source offsets and loops within 16 source bars".into()));
            }
        }
        let length =
            self.merge_beat_tick(first, clips.last().unwrap().end() - first.start_frame)?;
        let mut bars = length.div_ceil(BAR_TICKS).max(1) as u8;
        let required = clips.last().unwrap().end() - first.start_frame;
        while first
            .source_length(self.midi_frame(u32::from(bars) * BAR_TICKS))
            .is_none_or(|frames| frames < required)
        {
            if bars == 16 {
                return Err(Error("Merged beat pattern exceeds 16 source bars".into()));
            }
            bars += 1;
        }
        Ok((source.instrument_id, bars))
    }

    /// Unfold visible steps into one pattern without changing lane instruments or sources.
    pub fn merge_beat_clips(&self, ids: &[Id]) -> Result<(Self, Id)> {
        let (track, clips) = self.pattern_merge_targets(ids)?;
        let (owner, bars) = self.beat_merge_settings(&clips)?;
        let first = clips[0];
        let resolution = self.merge_beat_source(first)?.pattern.resolution;
        let mut pattern = BeatPattern {
            bars,
            resolution,
            ..Default::default()
        };
        let mut used = HashSet::new();
        let mut remaining = MAX_HITS;
        for clip in &clips {
            let source = self.merge_beat_source(clip)?;
            let steps: HashMap<_, _> = source
                .pattern
                .visible_steps()
                .map(|step| (step.id, step))
                .collect();
            // Reuse the same source-window/repeat mapping as MIDI Merge and Split.
            let notes = MidiPattern {
                length_ticks: source.pattern.length_ticks(),
                notes: steps
                    .values()
                    .map(|step| MidiNote {
                        id: step.id,
                        start_tick: step.start_tick,
                        length_ticks: resolution.step_ticks(),
                        pitch: 60,
                        velocity: step.velocity,
                    })
                    .collect(),
            };
            let mut window = (*clip).clone();
            window.repeat.get_or_insert(ClipLoop {
                length_frames: clip.length_frames,
                phase_frame: 0,
            });
            let occurrences = note_occurrences(self, &window, &notes, remaining).map_err(|_| {
                Error("Beat Merge exceeds the 4,096 hit limit; shorten the selection".into())
            })?;
            remaining -= occurrences.len();
            for occurrence in occurrences {
                let start = self.merge_beat_tick(first, occurrence.start - first.start_frame)?;
                let end = self.merge_beat_tick(first, occurrence.end - first.start_frame)?;
                if !start.is_multiple_of(resolution.step_ticks())
                    || end - start != resolution.step_ticks()
                {
                    return Err(Error(
                        "Beat Merge cannot preserve a partial or off-grid hit".into(),
                    ));
                }
                let mut step = steps[&occurrence.note.id].clone();
                step.start_tick = start;
                if !used.insert(step.id) {
                    step.id = Id::new_v4();
                }
                pattern.steps.push(step);
            }
        }
        pattern
            .steps
            .sort_by_key(|step| (step.start_tick, step.subtrack_id, step.id));
        let asset = self.beat_asset(owner, first.name.clone(), pattern)?;
        self.finish_pattern_merge(track, &clips, asset, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeatEdit, BeatResolution, BeatSubtrack, ClipStretch, Edit};

    fn fixture(rate: u32, tempo: f32) -> (Project, Id, Vec<Id>, Vec<Id>) {
        let mut p = Project {
            sample_rate_hz: rate,
            tempo_bpm: tempo,
            ..Project::default()
        };
        let track = p.add_beat_track().unwrap();
        let mut rows = Vec::new();
        for name in ["Kick", "Hat"] {
            let row = BeatSubtrack::new(name.into());
            rows.push(row.id);
            p.edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: rows.len() - 1,
                },
            })
            .unwrap();
        }
        let bar = p.midi_frame(BAR_TICKS);
        let mut ids = Vec::new();
        for i in 0..3 {
            let id = p.add_beat_pattern(track, i * bar).unwrap();
            let pattern = BeatPattern::default()
                .with_step(rows[0], 0, Some(80))
                .unwrap()
                .with_step(rows[1], 960, Some(120))
                .unwrap();
            p.edit(Edit::ReplaceBeatPattern {
                clip_id: id,
                pattern,
            })
            .unwrap();
            ids.push(id);
        }
        (p, track, ids, rows)
    }

    fn pattern(p: &Project) -> &BeatPattern {
        &p.assets
            .last()
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap()
            .pattern
    }

    #[test]
    fn merge_beat_retains_grid_aligned_gaps_and_stretched_hit_times_at_all_rates() {
        for rate in crate::SAMPLE_RATES {
            for tempo in [120.0, 123.25] {
                for scale in [1, 2] {
                    let (mut p, _, mut ids, rows) = fixture(rate, tempo);
                    let starts = [BAR_TICKS, 3 * BAR_TICKS, 6 * BAR_TICKS]
                        .map(|tick| p.midi_frame(tick * scale));
                    let source_length = p.midi_frame(BAR_TICKS);
                    for (clip, start) in p.tracks[0].clips.iter_mut().zip(starts) {
                        clip.start_frame = start;
                        clip.stretch = (scale != 1).then(|| crate::ClipStretch::new(1, 2).unwrap());
                        clip.length_frames = clip.source_length(source_length).unwrap();
                    }
                    p.validate().unwrap();
                    let before = p.clone();
                    ids.reverse();
                    let (next, id) = p.merge_pattern_clips(&ids).unwrap_or_else(|error| {
                        panic!("rate={rate}, tempo={tempo}, scale={scale}: {error}")
                    });
                    assert!(p.can_merge_pattern_clips(&ids));
                    assert_eq!(p, before);
                    let clip = &next.tracks[0].clips[0];
                    assert_eq!(id, ids[2]);
                    assert_eq!(clip.start_frame, starts[0]);
                    assert_eq!(clip.end(), before.tracks[0].clips[2].end());
                    assert_eq!(clip.stretch, before.tracks[0].clips[0].stretch);
                    assert_eq!(&next.assets[..before.assets.len()], &before.assets);
                    let mut hits: Vec<_> = pattern(&next)
                        .steps
                        .iter()
                        .map(|s| (s.start_tick, s.subtrack_id, s.velocity))
                        .collect();
                    hits.sort_by_key(|s| s.0);
                    assert_eq!(
                        hits,
                        [
                            (0, rows[0], 80),
                            (960, rows[1], 120),
                            (2 * BAR_TICKS, rows[0], 80),
                            (2 * BAR_TICKS + 960, rows[1], 120),
                            (5 * BAR_TICKS, rows[0], 80),
                            (5 * BAR_TICKS + 960, rows[1], 120),
                        ]
                    );
                    assert_eq!(
                        serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap())
                            .unwrap(),
                        next
                    );
                }
            }
        }
    }

    #[test]
    fn merge_beat_preserves_three_clips_lanes_settings_assets_and_serialization() {
        for rate in crate::SAMPLE_RATES {
            for tempo in [120.0, 123.25] {
                let (p, track, mut ids, rows) = fixture(rate, tempo);
                let original = p.clone();
                ids.reverse();
                assert!(p.can_merge_pattern_clips(&ids));
                assert!(!p.can_merge_midi_clips(&ids));
                let (merged, id) = p.merge_pattern_clips(&ids).unwrap();
                assert_eq!(p, original);
                assert_eq!(id, ids[2]);
                assert_eq!(merged.tracks[0].clips.len(), 1);
                assert_eq!(
                    merged.tracks[0].clips[0].length_frames,
                    p.midi_frame(BAR_TICKS) * 3
                );
                assert_eq!(merged.beat(track), p.beat(track));
                assert_eq!(&merged.assets[..p.assets.len()], &p.assets);
                let steps = &pattern(&merged).steps;
                assert_eq!(
                    steps
                        .iter()
                        .map(|s| (s.subtrack_id, s.start_tick, s.velocity))
                        .collect::<Vec<_>>(),
                    vec![
                        (rows[0], 0, 80),
                        (rows[1], 960, 120),
                        (rows[0], BAR_TICKS, 80),
                        (rows[1], BAR_TICKS + 960, 120),
                        (rows[0], 2 * BAR_TICKS, 80),
                        (rows[1], 2 * BAR_TICKS + 960, 120)
                    ]
                );
                assert_eq!(
                    serde_json::from_str::<Project>(&serde_json::to_string(&merged).unwrap())
                        .unwrap(),
                    merged
                );
            }
        }
    }

    #[test]
    fn merge_beat_unfolds_phased_shared_sources_with_offsets_and_stretch() {
        let (mut p, _, ids, rows) = fixture(48_000, 120.0);
        let bar = p.midi_frame(BAR_TICKS);
        let asset = p.tracks[0].clips[0].asset_id;
        for (i, clip) in p.tracks[0].clips.iter_mut().enumerate() {
            clip.asset_id = asset;
            clip.start_frame = 1000 + i as u64 * bar;
            clip.length_frames = bar;
            clip.stretch = Some(ClipStretch::new(1, 2).unwrap());
            clip.repeat = Some(ClipLoop {
                length_frames: bar,
                phase_frame: bar / 2,
            });
        }
        // The repeat contains the first two source beats; start at the second beat.
        p.validate().unwrap();
        let (merged, _) = p.merge_beat_clips(&ids).unwrap();
        let steps = &pattern(&merged).steps;
        assert_eq!(
            steps
                .iter()
                .map(|s| (s.subtrack_id, s.start_tick))
                .collect::<Vec<_>>(),
            vec![
                (rows[1], 0),
                (rows[0], 960),
                (rows[1], 1920),
                (rows[0], 2880),
                (rows[1], 3840),
                (rows[0], 4800)
            ]
        );
        assert_eq!(
            steps.iter().map(|s| s.id).collect::<HashSet<_>>().len(),
            steps.len()
        );
        assert_eq!(
            merged.tracks[0].clips[0].stretch,
            Some(ClipStretch::new(1, 2).unwrap())
        );
        assert!(merged.tracks[0].clips[0].repeat.is_none());
        // A trimmed source window discards earlier hits and keeps its lane bindings.
        p.tracks[0].clips[0].repeat = None;
        p.tracks[0].clips[0].source_offset_frame = bar / 2;
        let (merged, _) = p.merge_beat_clips(&ids).unwrap();
        assert_eq!(pattern(&merged).steps[0].subtrack_id, rows[1]);
        assert_eq!(pattern(&merged).steps[0].start_tick, 0);
    }

    #[test]
    fn merge_beat_rejects_intervening_clips_mixed_grids_stretch_off_grid_and_oversized_ranges() {
        let (p, _, ids, _) = fixture(48_000, 120.0);
        assert!(!p.can_merge_pattern_clips(&[ids[0], ids[2]]));
        assert!(
            p.merge_beat_clips(&[ids[0], ids[2]])
                .unwrap_err()
                .to_string()
                .contains("unselected")
        );
        for mode in 0..4 {
            let mut changed = p.clone();
            match mode {
                0 => {
                    let asset = changed.tracks[0].clips[1].asset_id;
                    changed
                        .assets
                        .iter_mut()
                        .find(|a| a.id == asset)
                        .unwrap()
                        .beat_pattern
                        .as_mut()
                        .unwrap()
                        .pattern
                        .resolution = BeatResolution::Eighth;
                }
                1 => changed.tracks[0].clips[1].stretch = Some(ClipStretch::new(1, 2).unwrap()),
                2 => changed.tracks[0].clips[1].source_offset_frame = changed.midi_frame(120),
                _ => {
                    let bar = changed.midi_frame(BAR_TICKS);
                    for (i, clip) in changed.tracks[0].clips.iter_mut().enumerate() {
                        clip.start_frame = i as u64 * 8 * bar;
                        clip.length_frames = 8 * bar;
                        clip.repeat = Some(ClipLoop {
                            length_frames: bar,
                            phase_frame: 0,
                        });
                    }
                }
            }
            let original = changed.clone();
            assert!(!changed.can_merge_pattern_clips(&ids), "mode={mode}");
            assert!(changed.merge_beat_clips(&ids).is_err(), "mode={mode}");
            assert_eq!(changed, original);
        }
    }

    #[test]
    fn merge_beat_does_not_activate_hidden_hits_and_rejects_other_containers() {
        let (mut p, _, ids, rows) = fixture(48_000, 120.0);
        let asset = p.tracks[0].clips[0].asset_id;
        p.assets
            .iter_mut()
            .find(|a| a.id == asset)
            .unwrap()
            .beat_pattern
            .as_mut()
            .unwrap()
            .pattern
            .steps
            .push(crate::BeatStep {
                id: Id::new_v4(),
                subtrack_id: rows[0],
                start_tick: 120,
                velocity: 100,
            });
        let (merged, _) = p.merge_beat_clips(&ids).unwrap();
        assert_eq!(pattern(&merged).steps.len(), 6);
        let other = p.add_beat_track().unwrap();
        let other_clip = p.add_beat_pattern(other, 0).unwrap();
        assert!(p.merge_pattern_clips(&[ids[0], other_clip]).is_err());
        let midi = p.add_midi_track().unwrap();
        let midi_clip = p.add_midi_pattern(midi, 0).unwrap();
        assert!(p.merge_pattern_clips(&[ids[0], midi_clip]).is_err());
    }
    #[test]
    fn merge_beat_limits_unfolded_hits_across_the_selection() {
        let (mut p, track, ids, mut rows) = fixture(48_000, 120.0);
        p.edit(Edit::DeleteClip(ids[2])).unwrap();
        for i in 2..9 {
            let row = BeatSubtrack::new(format!("Lane {i}"));
            rows.push(row.id);
            p.edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: i,
                },
            })
            .unwrap();
        }
        let mut source = BeatPattern {
            resolution: BeatResolution::ThirtySecond,
            ..Default::default()
        };
        for row in rows {
            for tick in (0..BAR_TICKS).step_by(120) {
                source = source.with_step(row, tick, Some(100)).unwrap();
            }
        }
        for id in &ids[..2] {
            p.edit(Edit::ReplaceBeatPattern {
                clip_id: *id,
                pattern: source.clone(),
            })
            .unwrap();
        }
        let bar = p.midi_frame(BAR_TICKS);
        for (i, clip) in p.tracks[0].clips.iter_mut().enumerate() {
            clip.start_frame = i as u64 * 8 * bar;
            clip.length_frames = 8 * bar;
            clip.repeat = Some(ClipLoop {
                length_frames: bar,
                phase_frame: 0,
            });
        }
        p.validate().unwrap();
        let original = p.clone();
        assert!(
            p.merge_beat_clips(&ids[..2])
                .unwrap_err()
                .to_string()
                .contains("4,096")
        );
        assert_eq!(p, original);
    }
    #[test]
    fn merge_beat_accepts_sample_rounding_gap_at_130_bpm() {
        let (mut p, track, ids, rows) = fixture(48_000, 130.0);
        p.edit(Edit::DeleteClip(ids[2])).unwrap();
        let source = BeatPattern::default()
            .with_grid(2, BeatResolution::Eighth)
            .unwrap()
            .with_step(rows[0], 0, Some(100))
            .unwrap()
            .with_step(rows[1], 7200, Some(90))
            .unwrap();
        let source = p
            .beat_asset(p.beat(track).unwrap().id, "Pattern".into(), source)
            .unwrap();
        let asset = source.id;
        p.assets.push(source);
        let boundary = p.midi_frame(7680);
        for (i, clip) in p.tracks[0].clips.iter_mut().enumerate() {
            clip.asset_id = asset;
            clip.start_frame = if i == 0 { 0 } else { boundary + 1 };
            clip.length_frames = boundary - 1;
        }
        assert_eq!(p.tracks[0].clips[0].length_frames, 177_230);
        assert_eq!(p.tracks[0].clips[1].start_frame, 177_232);
        p.validate().unwrap();
        let original = p.clone();
        assert!(p.can_merge_pattern_clips(&ids[..2]));
        let (merged, _) = p.merge_pattern_clips(&ids[..2]).unwrap();
        assert_eq!(p, original);
        assert_eq!(merged.tracks[0].clips[0].length_frames, 354_462);
        assert_eq!(
            pattern(&merged)
                .steps
                .iter()
                .map(|s| (s.start_tick, s.velocity))
                .collect::<Vec<_>>(),
            vec![(0, 100), (7200, 90), (7680, 100), (14880, 90)]
        );
        // Off-grid clip positions still cannot preserve the saved beat subdivision.
        p.tracks[0].clips[1].start_frame = p.tracks[0].clips[0].end() + p.midi_frame(1);
        assert!(!p.can_merge_pattern_clips(&ids[..2]));
    }
    #[test]
    fn merge_consolidates_one_phased_beat_loop_and_retains_stretch() {
        let (mut p, track, ids, rows) = fixture(48_000, 120.0);
        p.tracks[0].clips.retain(|c| c.id == ids[0]);
        let beat = p.midi_frame(960);
        let clip = &mut p.tracks[0].clips[0];
        clip.start_frame = 1000;
        clip.source_offset_frame = 0;
        clip.stretch = Some(ClipStretch::new(1, 2).unwrap());
        clip.repeat = Some(ClipLoop {
            length_frames: beat * 4,
            phase_frame: beat * 2,
        });
        clip.length_frames = beat * 12;
        p.validate().unwrap();
        let original = p.clone();
        assert!(p.can_merge_pattern_clips(&ids[..1]));
        let (next, id) = p.merge_pattern_clips(&ids[..1]).unwrap();
        assert_eq!(p, original);
        assert_eq!(id, ids[0]);
        assert_eq!(next.beat(track), p.beat(track));
        assert_eq!(&next.assets[..p.assets.len()], &p.assets);
        let clip = &next.tracks[0].clips[0];
        assert_eq!(
            (
                clip.start_frame,
                clip.length_frames,
                clip.source_offset_frame
            ),
            (1000, beat * 12, 0)
        );
        assert!(clip.repeat.is_none());
        assert_eq!(clip.stretch, p.tracks[0].clips[0].stretch);
        assert_eq!(
            pattern(&next)
                .steps
                .iter()
                .map(|s| (s.subtrack_id, s.start_tick, s.velocity))
                .collect::<Vec<_>>(),
            vec![
                (rows[1], 0, 120),
                (rows[0], 960, 80),
                (rows[1], 1920, 120),
                (rows[0], 2880, 80),
                (rows[1], 3840, 120),
                (rows[0], 4800, 80)
            ]
        );
        assert_eq!(
            pattern(&next)
                .steps
                .iter()
                .map(|s| s.id)
                .collect::<HashSet<_>>()
                .len(),
            6
        );
    }

    #[test]
    fn merge_consolidates_single_beat_left_right_and_both_trims() {
        for (offset_ticks, length_ticks, expected) in [
            (0, 1920, vec![(0, 80), (960, 120)]),
            (960, 2880, vec![(0, 120)]),
            (960, 960, vec![(0, 120)]),
        ] {
            let (mut p, _, ids, _) = fixture(48_000, 120.0);
            let offset = p.midi_frame(offset_ticks);
            let length = p.midi_frame(length_ticks);
            p.tracks[0].clips[0].source_offset_frame = offset;
            p.tracks[0].clips[0].length_frames = length;
            let original = p.clone();
            assert!(p.can_merge_pattern_clips(&ids[..1]));
            let (next, _) = p.merge_pattern_clips(&ids[..1]).unwrap();
            assert_eq!(p, original);
            assert_eq!(next.tracks[0].clips[0].source_offset_frame, 0);
            assert_eq!(next.tracks[0].clips[0].length_frames, length);
            assert_eq!(
                pattern(&next)
                    .steps
                    .iter()
                    .map(|s| (s.start_tick, s.velocity))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(&next.assets[..p.assets.len()], &p.assets);
        }
        let (p, _, ids, _) = fixture(48_000, 120.0);
        assert!(p.can_merge_pattern_clips(&ids[..1]));
    }

    #[test]
    fn single_beat_consolidation_rejects_more_than_sixteen_source_bars() {
        let (mut p, _, ids, _) = fixture(48_000, 120.0);
        p.tracks[0].clips.retain(|c| c.id == ids[0]);
        let base = p.midi_frame(BAR_TICKS);
        p.tracks[0].clips[0].repeat = Some(ClipLoop {
            length_frames: base,
            phase_frame: 0,
        });
        p.tracks[0].clips[0].length_frames = base * 17;
        let original = p.clone();
        assert!(!p.can_merge_pattern_clips(&ids[..1]));
        assert!(p.merge_pattern_clips(&ids[..1]).is_err());
        assert_eq!(p, original);
    }
}
