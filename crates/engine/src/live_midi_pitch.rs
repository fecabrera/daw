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
                    if entry.base != note.pitch && (state & 256 == 0 || state as u8 == note.pitch) {
                        entry.acknowledged = state >> 9;
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
                    (((v >> 9).wrapping_add(1)) << 9) | u64::from(pitch) | (u64::from(active) << 8),
                )
            });
        true
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
        if state >> 9 > self.acknowledged {
            (state as u8, state & 256 != 0)
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
}
