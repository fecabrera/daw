//! Control-thread pitch mailboxes. Workers and offline clones never inherit live inputs.
use daw_core::{Id, Project};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
type Key = (Id, Id, Id);
const ACTIVE: u64 = 1 << 8;
const RESTORED: u64 = 1 << 9;
const GENERATION_SHIFT: u32 = 10;
#[derive(Clone)]
struct Entry {
    base: u8,
    acknowledged: u64,
    value: Arc<AtomicU64>,
}
#[derive(Clone, Default)]
pub struct LiveMidiPitches {
    project: Option<Id>,
    entries: HashMap<Key, Entry>,
}
impl LiveMidiPitches {
    /// Retain mailboxes by project/track/clip/note identity. A committed pitch acknowledges
    /// its preview without changing the mailbox still used by the old playing renderer.
    pub fn prepare(project: &Project, previous: &Self) -> Self {
        let mut entries = HashMap::new();
        for track in &project.tracks {
            for clip in &track.clips {
                let Some(pattern) = project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .and_then(|a| a.midi_pattern.as_ref())
                else {
                    continue;
                };
                for note in &pattern.notes {
                    let key = (track.id, clip.id, note.id);
                    let mut entry = previous
                        .entries
                        .get(&key)
                        .filter(|_| previous.project == Some(project.project_id))
                        .cloned()
                        .unwrap_or_else(|| Entry {
                            base: note.pitch,
                            acknowledged: 0,
                            value: Arc::new(AtomicU64::new(0)),
                        });
                    let state = entry.value.load(Ordering::Acquire);
                    if entry.base != note.pitch
                        && (state & ACTIVE == 0 || state as u8 == note.pitch)
                    {
                        entry.acknowledged = state >> GENERATION_SHIFT;
                    }
                    entry.base = note.pitch;
                    entries.insert(key, entry);
                }
            }
        }
        Self {
            project: Some(project.project_id),
            entries,
        }
    }
    pub fn set(&self, track: Id, clip: Id, note: Id, pitch: u8, active: bool) -> bool {
        if pitch > 127 {
            return false;
        }
        let Some(entry) = self.entries.get(&(track, clip, note)) else {
            return false;
        };
        let _ = entry
            .value
            .fetch_update(Ordering::Release, Ordering::Relaxed, |v| {
                Some(
                    (((v >> GENERATION_SHIFT).wrapping_add(1)) << GENERATION_SHIFT)
                        | u64::from(pitch)
                        | if active { ACTIVE } else { 0 },
                )
            });
        true
    }
    /// Restore a released override to this complete schedule's pitch, not the
    /// possibly newer editor pitch. An active drag always owns its newer value.
    pub fn restore_released(&self, track: Id, clip: Id, note: Id) {
        let Some(entry) = self.entries.get(&(track, clip, note)) else {
            return;
        };
        let _ = entry
            .value
            .fetch_update(Ordering::Release, Ordering::Relaxed, |state| {
                // Keep the gesture generation, but make each bound renderer use
                // its own prepared base. Writing this entry's numeric base could
                // leak the old pitch into a replacement already being prepared.
                (state & (ACTIVE | RESTORED) == 0).then_some(state | RESTORED)
            });
    }
    pub(super) fn matches_project(&self, id: Id) -> bool {
        self.project == Some(id)
    }
    pub(super) fn bind(&self, track: Id, clip: Id, note: Id) -> PitchInput {
        self.entries
            .get(&(track, clip, note))
            .map_or_else(PitchInput::default, |e| PitchInput {
                value: Some(e.value.clone()),
                acknowledged: e.acknowledged,
            })
    }
}
#[derive(Default)]
pub(super) struct PitchInput {
    value: Option<Arc<AtomicU64>>,
    acknowledged: u64,
}
impl Clone for PitchInput {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PitchInput {
    pub fn read(&self, base: u8) -> (u8, bool) {
        let state = self.value.as_ref().map_or(0, |v| v.load(Ordering::Acquire));
        if state & RESTORED == 0 && state >> GENERATION_SHIFT > self.acknowledged {
            (state as u8, state & ACTIVE != 0)
        } else {
            (base, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{Edit, MidiNote, MidiPattern};
    #[test]
    fn live_pitch_mailboxes_coalesce_acknowledge_commit_undo_and_keep_newer_drags() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let clip = p.add_midi_pattern(track, 0).unwrap();
        let note = Id::new_v4();
        let pattern = |pitch| MidiPattern {
            length_ticks: 960,
            notes: vec![MidiNote {
                id: note,
                start_tick: 0,
                length_ticks: 960,
                pitch,
                velocity: 100,
            }],
        };
        p.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(60),
        })
        .unwrap();
        let input = LiveMidiPitches::prepare(&p, &Default::default());
        let old = input.bind(track, clip, note);
        for pitch in 0..128 {
            assert!(input.set(track, clip, note, pitch, true));
        }
        assert_eq!(old.read(60), (127, true));
        assert!(!input.set(track, clip, note, 128, true));
        input.set(track, clip, note, 64, true);
        p.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(64),
        })
        .unwrap();
        let committed = LiveMidiPitches::prepare(&p, &input);
        let bound = committed.bind(track, clip, note);
        assert_eq!(bound.read(64), (64, false));
        assert_eq!(old.read(60), (64, true));
        input.set(track, clip, note, 67, true);
        assert_eq!(bound.read(64), (67, true));
        input.set(track, clip, note, 67, false);
        p.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(67),
        })
        .unwrap();
        let next = LiveMidiPitches::prepare(&p, &committed);
        assert_eq!(next.bind(track, clip, note).read(67), (67, false));
        p.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(60),
        })
        .unwrap();
        let undo = LiveMidiPitches::prepare(&p, &next);
        assert_eq!(undo.bind(track, clip, note).read(60), (60, false));
        input.set(track, clip, note, 72, true);
        assert_eq!(undo.bind(track, clip, note).read(60), (72, true));
        assert_eq!(old.clone().read(60), (60, false));
        p.project_id = Id::new_v4();
        let other = LiveMidiPitches::prepare(&p, &undo);
        assert_eq!(other.bind(track, clip, note).read(60), (60, false));
    }
    #[test]
    fn released_pending_pitches_restore_prepared_state_and_preserve_newer_drags() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let clip = project.add_midi_pattern(track, 0).unwrap();
        let note = Id::new_v4();
        let pattern = |pitch| MidiPattern {
            length_ticks: 960,
            notes: vec![MidiNote {
                id: note,
                start_tick: 0,
                length_ticks: 960,
                pitch,
                velocity: 100,
            }],
        };
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(60),
            })
            .unwrap();
        let prepared = LiveMidiPitches::prepare(&project, &Default::default());
        let old = prepared.bind(track, clip, note);
        // Repeated pending editor edits must not become the rollback pitch.
        for pitch in [64, 67, 72] {
            prepared.set(track, clip, note, pitch, true);
            prepared.restore_released(track, clip, note);
            assert_eq!(old.read(60), (pitch, true));
            prepared.set(track, clip, note, pitch, false);
            project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: clip,
                    pattern: pattern(pitch),
                })
                .unwrap();
            prepared.restore_released(track, clip, note);
            prepared.restore_released(track, clip, note);
            assert_eq!(old.read(60), (60, false));
        }
        // Retry publishes the complete target without changing the old renderer.
        let retried = LiveMidiPitches::prepare(&project, &prepared);
        let current = retried.bind(track, clip, note);
        assert_eq!(old.read(60), (60, false));
        assert_eq!(current.read(72), (72, false));
        retried.set(track, clip, note, 76, true);
        retried.restore_released(track, clip, note);
        assert_eq!(current.read(72), (76, true));
        retried.set(track, clip, note, 76, false);
        retried.restore_released(track, clip, note);
        assert_eq!(current.read(72), (72, false));
        // Undo/Redo still acknowledge complete schedules, not transient previews.
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(60),
            })
            .unwrap();
        let undone = LiveMidiPitches::prepare(&project, &retried);
        assert_eq!(undone.bind(track, clip, note).read(60), (60, false));
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(72),
            })
            .unwrap();
        let redone = LiveMidiPitches::prepare(&project, &undone);
        assert_eq!(redone.bind(track, clip, note).read(72), (72, false));
    }
    #[test]
    fn restoration_uses_each_schedule_base_during_overlapping_preparation() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let clip = project.add_midi_pattern(track, 0).unwrap();
        let note = Id::new_v4();
        let edit = |pitch| Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: note,
                    start_tick: 0,
                    length_ticks: 960,
                    pitch,
                    velocity: 100,
                }],
            },
        };
        project.edit(edit(60)).unwrap();
        let previous = LiveMidiPitches::prepare(&project, &Default::default());
        let old = previous.bind(track, clip, note);
        project.edit(edit(64)).unwrap();
        previous.set(track, clip, note, 67, true);
        // A complete replacement for 64 is prepared during a newer drag to 67.
        let replacement = LiveMidiPitches::prepare(&project, &previous);
        let next = replacement.bind(track, clip, note);
        assert_eq!(next.read(64), (67, true));
        previous.set(track, clip, note, 67, false);
        previous.restore_released(track, clip, note);
        assert_eq!(old.read(60), (60, false));
        assert_eq!(next.read(64), (64, false));
        // Newer gestures still override both schedules after restoration.
        replacement.set(track, clip, note, 69, true);
        previous.restore_released(track, clip, note);
        assert_eq!(old.read(60), (69, true));
        assert_eq!(next.read(64), (69, true));
        replacement.set(track, clip, note, 69, false);
        replacement.restore_released(track, clip, note);
        assert_eq!(old.read(60), (60, false));
        assert_eq!(next.read(64), (64, false));
    }
}
