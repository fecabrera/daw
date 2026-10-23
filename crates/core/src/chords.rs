//! Built-in root-position chord presets and atomic one-bar MIDI placement.
use crate::{
    ClipPlacement, Edit, Error, Id, MIDI_TICKS_PER_BEAT, MidiNote, MidiPattern, Project, Result,
    TrackKind,
};

pub const CHORD_ROOTS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordKind {
    Major,
    Minor,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Sixth,
    MinorSixth,
    Seventh,
    MajorSeventh,
    MinorSeventh,
    HalfDiminished,
}
impl ChordKind {
    pub const ALL: [Self; 12] = [
        Self::Major,
        Self::Minor,
        Self::Diminished,
        Self::Augmented,
        Self::Sus2,
        Self::Sus4,
        Self::Sixth,
        Self::MinorSixth,
        Self::Seventh,
        Self::MajorSeventh,
        Self::MinorSeventh,
        Self::HalfDiminished,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Diminished => "diminished",
            Self::Augmented => "augmented",
            Self::Sus2 => "sus2",
            Self::Sus4 => "sus4",
            Self::Sixth => "6",
            Self::MinorSixth => "minor 6",
            Self::Seventh => "7",
            Self::MajorSeventh => "major 7",
            Self::MinorSeventh => "minor 7",
            Self::HalfDiminished => "half-diminished 7",
        }
    }
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented => &[0, 4, 8],
            Self::Sus2 => &[0, 2, 7],
            Self::Sus4 => &[0, 5, 7],
            Self::Sixth => &[0, 4, 7, 9],
            Self::MinorSixth => &[0, 3, 7, 9],
            Self::Seventh => &[0, 4, 7, 10],
            Self::MajorSeventh => &[0, 4, 7, 11],
            Self::MinorSeventh => &[0, 3, 7, 10],
            Self::HalfDiminished => &[0, 3, 6, 10],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub root: u8,
    pub kind: ChordKind,
}
impl Chord {
    pub fn name(self) -> Result<String> {
        let root = CHORD_ROOTS
            .get(usize::from(self.root))
            .ok_or_else(|| Error("Chord root must be in 0–11".into()))?;
        Ok(format!("{root} {}", self.kind.name()))
    }
    /// C4–B4 roots; every note starts together and lasts one 4/4 bar.
    pub fn pattern(self) -> Result<MidiPattern> {
        self.name()?;
        let length_ticks = 4 * MIDI_TICKS_PER_BEAT;
        Ok(MidiPattern {
            length_ticks,
            notes: self
                .kind
                .intervals()
                .iter()
                .map(|interval| MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks,
                    pitch: 60 + self.root + interval,
                    velocity: 100,
                })
                .collect(),
        })
    }
}
impl Project {
    /// Place a catalog group in order. Any failure retains the complete original project.
    pub fn add_midi_chords(
        &mut self,
        track: Option<Id>,
        start_frame: u64,
        chords: &[Chord],
    ) -> Result<(Id, Vec<Id>)> {
        if chords.is_empty() {
            return Err(Error("No chords selected".into()));
        }
        let mut next = self.clone();
        let track = match track {
            Some(id) => {
                if !next
                    .tracks
                    .iter()
                    .any(|t| t.id == id && t.kind == TrackKind::Midi)
                {
                    return Err(Error("Chords require a MIDI track".into()));
                }
                id
            }
            None => next.add_midi_track()?,
        };
        let mut start = start_frame;
        let mut placements = Vec::with_capacity(chords.len());
        for chord in chords {
            let (asset, clip) = next.midi_clip(chord.name()?, chord.pattern()?, start)?;
            start = start
                .checked_add(clip.length_frames)
                .ok_or_else(|| Error("Chord placement exceeds timeline bounds".into()))?;
            next.assets.push(asset);
            placements.push(ClipPlacement {
                track_id: track,
                clip,
            });
        }
        let ids = placements.iter().map(|p| p.clip.id).collect();
        next.edit(Edit::OverwriteClips(placements))?;
        *self = next;
        Ok((track, ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chord_presets_have_known_intervals_independent_notes_and_one_bar_timing() {
        assert_eq!(
            Chord {
                root: 0,
                kind: ChordKind::Major
            }
            .pattern()
            .unwrap()
            .notes
            .iter()
            .map(|n| n.pitch)
            .collect::<Vec<_>>(),
            [60, 64, 67]
        );
        assert_eq!(
            Chord {
                root: 9,
                kind: ChordKind::MinorSeventh
            }
            .pattern()
            .unwrap()
            .notes
            .iter()
            .map(|n| n.pitch)
            .collect::<Vec<_>>(),
            [69, 72, 76, 79]
        );
        for root in 0..12 {
            for kind in ChordKind::ALL {
                let chord = Chord { root, kind };
                let pattern = chord.pattern().unwrap();
                pattern.validate().unwrap();
                assert_eq!(pattern.length_ticks, 3840);
                assert!(
                    pattern
                        .notes
                        .iter()
                        .all(|n| n.start_tick == 0 && n.length_ticks == 3840 && n.velocity == 100)
                );
                let copy = chord.pattern().unwrap();
                assert!(
                    pattern
                        .notes
                        .iter()
                        .all(|a| copy.notes.iter().all(|b| a.id != b.id))
                );
                for rate in crate::SAMPLE_RATES {
                    let mut project = Project {
                        sample_rate_hz: rate,
                        tempo_bpm: 137.5,
                        ..Default::default()
                    };
                    let (track, ids) = project.add_midi_chords(None, 1234, &[chord]).unwrap();
                    let clip = &project.tracks.iter().find(|t| t.id == track).unwrap().clips[0];
                    assert_eq!(clip.id, ids[0]);
                    assert_eq!(
                        clip.length_frames,
                        (f64::from(rate) * 240.0 / 137.5).round() as u64
                    );
                    project.validate().unwrap();
                    let reopened: Project =
                        serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
                    reopened.validate().unwrap();
                    assert_eq!(
                        reopened.assets[0].midi_pattern,
                        project.assets[0].midi_pattern
                    );
                }
            }
        }
    }
    #[test]
    fn chord_group_overwrites_neighbors_and_failure_is_atomic() {
        let mut project = Project::default();
        let chord = Chord {
            root: 0,
            kind: ChordKind::Major,
        };
        let (track, _) = project
            .add_midi_chords(None, 0, &[chord, chord, chord])
            .unwrap();
        let sources = project.assets.clone();
        let bar = project.midi_frame(3840);
        let (_, inserted) = project
            .add_midi_chords(
                Some(track),
                bar / 2,
                &[Chord {
                    root: 7,
                    kind: ChordKind::Seventh,
                }],
            )
            .unwrap();
        assert_eq!(project.tracks.len(), 1);
        let clips = &project.tracks[0].clips;
        assert_eq!(clips[0].length_frames, bar / 2);
        assert_eq!(clips[1].id, inserted[0]);
        assert_eq!(clips[2].source_offset_frame, bar / 2);
        assert_eq!(sources[0].midi_pattern, project.assets[0].midi_pattern);
        let audio = project.add_track().unwrap();
        let before = serde_json::to_string(&project).unwrap();
        for (target, start, chords) in [
            (Some(audio), 0, vec![chord]),
            (Some(Id::new_v4()), 0, vec![chord]),
            (None, u64::MAX - 1, vec![chord]),
            (None, 0, vec![Chord { root: 12, ..chord }]),
            (None, 0, vec![]),
        ] {
            assert!(project.add_midi_chords(target, start, &chords).is_err());
            assert_eq!(serde_json::to_string(&project).unwrap(), before);
        }
    }
}
