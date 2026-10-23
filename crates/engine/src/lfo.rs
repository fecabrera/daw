//! Prepared note origins for continuous modulators. No repeated-pattern expansion or callback allocation.
use daw_core::Clip;
use daw_unit_core::ModulationSourceContext;

#[derive(Clone)]
pub(super) struct NoteClip {
    clip: Clip,
    retrigger: Vec<u64>,
    legato: Vec<u64>,
    previous: [Option<i128>; 2],
}
impl NoteClip {
    pub fn new(clip: Clip, events: impl Iterator<Item = (u64, bool)>) -> Self {
        let mut retrigger = Vec::new();
        let mut legato = Vec::new();
        let mut held = 0usize;
        for (frame, on) in events {
            let frame = clip.stretch.map_or(frame, |s| {
                (u128::from(frame) * u128::from(s.output_frames))
                    .div_ceil(u128::from(s.source_frames))
                    .min(u128::from(u64::MAX)) as u64
            });
            if on {
                retrigger.push(frame);
                if held == 0 {
                    legato.push(frame);
                }
                held += 1;
            } else {
                held = held.saturating_sub(1);
            }
        }
        Self {
            clip,
            retrigger,
            legato,
            previous: [None; 2],
        }
    }
    fn origin(&self, frame: u64, phrase: usize) -> Option<i128> {
        let events = if phrase == 0 {
            &self.retrigger
        } else {
            &self.legato
        };
        let local = frame
            .saturating_sub(self.clip.start_frame)
            .min(self.clip.length_frames - 1);
        let position = self.clip.source_frame(local);
        let index = events.partition_point(|e| *e <= position);
        let (event, cycle) = if let Some(repeat) = self.clip.repeat {
            let elapsed = u128::from(local) + u128::from(repeat.phase_frame);
            let cycle = (elapsed / u128::from(repeat.length_frames)) as i128;
            if index > 0 {
                (events[index - 1], cycle)
            } else if cycle > 0 {
                let end = self
                    .clip
                    .source_offset_frame
                    .saturating_add(repeat.length_frames);
                let last = events.partition_point(|e| *e < end);
                if last == 0 {
                    return self.previous[phrase];
                }
                (events[last - 1], cycle - 1)
            } else {
                return self.previous[phrase];
            }
        } else if index > 0 {
            (events[index - 1], 0)
        } else {
            return self.previous[phrase];
        };
        let repeat_offset = self.clip.repeat.map_or(0, |r| {
            cycle * i128::from(r.length_frames) - i128::from(r.phase_frame)
        });
        Some(
            i128::from(self.clip.start_frame) + repeat_offset + i128::from(event)
                - i128::from(self.clip.source_offset_frame),
        )
    }
}
#[derive(Clone)]
pub(super) struct NoteSchedule {
    clips: Vec<NoteClip>,
}
impl NoteSchedule {
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        vec_bytes(&self.clips)
            + self
                .clips
                .iter()
                .map(|c| {
                    c.clip.name.capacity() as u64 + vec_bytes(&c.retrigger) + vec_bytes(&c.legato)
                })
                .sum::<u64>()
    }

    pub fn new(mut clips: Vec<NoteClip>) -> Self {
        clips.sort_by_key(|c| c.clip.start_frame);
        let mut previous = [None; 2];
        for clip in &mut clips {
            clip.previous = previous;
            previous = [
                clip.origin(clip.clip.end() - 1, 0),
                clip.origin(clip.clip.end() - 1, 1),
            ];
        }
        Self { clips }
    }
    pub fn context(&self, frame: u64, tempo_bpm: f32) -> ModulationSourceContext {
        let index = self.clips.partition_point(|c| c.clip.start_frame <= frame);
        let origins = index.checked_sub(1).map_or([None; 2], |i| {
            [
                self.clips[i].origin(frame, 0),
                self.clips[i].origin(frame, 1),
            ]
        });
        ModulationSourceContext {
            frame,
            tempo_bpm,
            note_origin: origins[0],
            legato_origin: origins[1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{ClipLoop, ClipStretch, Id};
    fn clip(start_frame: u64, source_offset_frame: u64, length_frames: u64) -> Clip {
        Clip {
            muted: false,
            id: Id::new_v4(),
            asset_id: Id::new_v4(),
            name: String::new(),
            color: None,
            start_frame,
            source_offset_frame,
            length_frames,
            repeat: None,
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
        }
    }
    fn notes() -> impl Iterator<Item = (u64, bool)> {
        [
            (10, true),
            (20, true),
            (30, false),
            (40, false),
            (50, true),
            (60, false),
        ]
        .into_iter()
    }
    fn origins(s: &NoteSchedule, frame: u64) -> [Option<i128>; 2] {
        let c = s.context(frame, 120.0);
        [c.note_origin, c.legato_origin]
    }
    #[test]
    fn overlapping_and_adjacent_notes_have_distinct_restart_origins_across_gaps() {
        let s = NoteSchedule::new(vec![
            NoteClip::new(clip(100, 0, 80), notes()),
            NoteClip::new(
                clip(200, 0, 80),
                [(10, true), (20, false), (20, true), (30, false)].into_iter(),
            ),
        ]);
        assert_eq!(origins(&s, 0), [None, None]);
        assert_eq!(origins(&s, 109), [None, None]);
        assert_eq!(origins(&s, 125), [Some(120), Some(110)]);
        assert_eq!(origins(&s, 190), [Some(150), Some(150)]);
        assert_eq!(origins(&s, 205), [Some(150), Some(150)]);
        assert_eq!(origins(&s, 220), [Some(220), Some(220)]);
        assert_eq!(origins(&s, 400), [Some(220), Some(220)]);
    }
    #[test]
    fn trims_stretch_and_repeat_phase_use_the_audible_source_clock_without_expansion() {
        let mut c = clip(0, 30, 200);
        c.stretch = ClipStretch::new(1, 2);
        let s = NoteSchedule::new(vec![NoteClip::new(c, notes())]);
        assert_eq!(origins(&s, 0), [Some(-10), Some(-10)]);
        assert_eq!(origins(&s, 10), [Some(10), Some(-10)]);
        assert_eq!(origins(&s, 70), [Some(70), Some(70)]);
        let mut c = clip(100, 0, 10_000_000_000);
        c.repeat = Some(ClipLoop {
            length_frames: 80,
            phase_frame: 25,
        });
        let s = NoteSchedule::new(vec![NoteClip::new(c, notes())]);
        assert_eq!(origins(&s, 100), [Some(95), Some(85)]);
        // Before the first note of the new cycle, keep the preceding cycle's origin.
        assert_eq!(origins(&s, 155), [Some(125), Some(125)]);
        assert_eq!(origins(&s, 165), [Some(165), Some(165)]);
        assert_eq!(origins(&s, 175), [Some(175), Some(165)]);
        assert_eq!(
            origins(&s, 8_000_000_175),
            [Some(8_000_000_175), Some(8_000_000_165)]
        );
    }
}
