use daw_core::{Clip, Id, Project, Track, TrackKind, linear_gain};
use daw_oscillator::Oscillator;
use daw_unit_core::{ProcessingContext, SourceUnit};

struct NoteEvent {
    frame: u64,
    note: Id,
    slot: usize,
    on: Option<(u8, u8)>,
}
struct PatternClip {
    clip: Clip,
    events: Vec<NoteEvent>,
}
struct Voice {
    source: Oscillator,
    current: Option<Id>,
    envelope: f32,
    target: f32,
    velocity: f32,
    step: f32,
    gain: f32,
    transpose: i8,
    previous: f32,
    transition_from: f32,
    transition: u32,
    fade_frames: u32,
}
impl Voice {
    fn note_on(&mut self, id: Id, pitch: u8, velocity: u8) {
        self.transition_from = self.previous;
        self.transition = self.fade_frames;
        self.current = Some(id);
        self.envelope = 0.0;
        self.target = 1.0;
        self.velocity = f32::from(velocity) / 127.0;
        self.source.reset();
        self.source.set_frequency(
            440.0 * 2.0_f32.powf((f32::from(pitch) + f32::from(self.transpose) - 69.0) / 12.0),
        );
    }
    fn end_note(&mut self) {
        self.current = None;
        self.target = 0.0;
    }
    fn reset(&mut self) {
        self.end_note();
        self.envelope = 0.0;
        self.previous = 0.0;
        self.transition = 0;
        self.source.reset();
    }
    fn sample(&mut self) -> f32 {
        if self.envelope == 0.0 && self.target == 0.0 && self.transition == 0 {
            self.previous = 0.0;
            return 0.0;
        }
        if self.envelope < self.target {
            self.envelope = (self.envelope + self.step).min(self.target);
        } else {
            self.envelope = (self.envelope - self.step).max(self.target);
        }
        let mut sample = self.source.next_sample() * self.envelope * self.velocity * self.gain;
        if self.transition > 0 {
            let progress = 1.0 - self.transition as f32 / self.fade_frames as f32;
            sample = self.transition_from * (1.0 - progress) + sample * progress;
            self.transition -= 1;
        }
        self.previous = sample;
        sample
    }
}

pub(crate) struct MidiTrackRuntime {
    voices: Vec<Voice>,
    clips: Vec<PatternClip>,
    sample_rate_hz: u32,
    fingerprint: u64,
    held_sample: f32,
    last_clip: Option<Id>,
    last_source: Option<u64>,
    next_event: usize,
}
impl MidiTrackRuntime {
    /// Build schedules and DSP state before publication. No callback allocations.
    pub fn new(project: &Project, track: &Track) -> Option<Self> {
        if track.kind != TrackKind::Midi {
            return None;
        }
        let instrument = track.instrument.as_ref()?;
        let fade_frames = (project.sample_rate_hz / 200).max(1);
        let mut voice_count = 0;
        let clips = track
            .clips
            .iter()
            .filter_map(|clip| {
                let pattern = project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)?
                    .midi_pattern
                    .as_ref()?;
                let mut events = Vec::with_capacity(pattern.notes.len() * 2);
                let mut ordered: Vec<_> = pattern.notes.iter().collect();
                ordered.sort_by_key(|note| (note.start_tick, note.id));
                let mut free_at = Vec::new();
                for note in ordered {
                    let start = project.midi_frame(note.start_tick);
                    let end = project
                        .midi_frame(note.start_tick + note.length_ticks)
                        .max(start + 1);
                    let slot = free_at
                        .iter()
                        .position(|until| *until <= start)
                        .unwrap_or(free_at.len());
                    let available = end.saturating_add(u64::from(fade_frames));
                    if slot == free_at.len() {
                        free_at.push(available);
                    } else {
                        free_at[slot] = available;
                    }
                    events.push(NoteEvent {
                        slot,
                        frame: start,
                        note: note.id,
                        on: Some((note.pitch, note.velocity)),
                    });
                    events.push(NoteEvent {
                        slot,
                        frame: end,
                        note: note.id,
                        on: None,
                    });
                }
                voice_count = voice_count.max(free_at.len());
                // Ends precede starts at a shared boundary; ID breaks simultaneous-on ties.
                events.sort_by_key(|e| (e.frame, e.on.is_some(), e.note));
                Some(PatternClip {
                    clip: clip.clone(),
                    events,
                })
            })
            .collect();
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}{:?}", track.clips, track.instrument).hash(&mut hash);
        project.tempo_bpm.to_bits().hash(&mut hash);
        project.sample_rate_hz.hash(&mut hash);
        for clip in &track.clips {
            if let Some(asset) = project.assets.iter().find(|a| a.id == clip.asset_id) {
                format!("{:?}", asset.midi_pattern).hash(&mut hash);
            }
        }
        Some(Self {
            fingerprint: hash.finish(),
            held_sample: 0.0,
            voices: (0..voice_count)
                .map(|_| Voice {
                    source: Oscillator::new(
                        instrument.waveform,
                        ProcessingContext {
                            sample_rate_hz: project.sample_rate_hz,
                            max_block_frames: project.buffer_size_frames,
                        },
                    ),
                    current: None,
                    envelope: 0.0,
                    target: 0.0,
                    velocity: 0.0,
                    step: 1.0 / (project.sample_rate_hz / 200).max(1) as f32,
                    gain: linear_gain(instrument.gain_db),
                    transpose: instrument.transpose,
                    previous: 0.0,
                    transition_from: 0.0,
                    transition: 0,
                    fade_frames,
                })
                .collect(),
            clips,
            sample_rate_hz: project.sample_rate_hz,
            last_clip: None,
            last_source: None,
            next_event: 0,
        })
    }
    pub fn compatible(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
    pub fn held_sample(&self) -> f32 {
        self.held_sample
    }
    pub fn reset(&mut self) {
        self.held_sample = 0.0;
        for voice in &mut self.voices {
            voice.reset();
        }
        self.last_clip = None;
        self.last_source = None;
    }
    pub fn end_notes(&mut self) {
        for voice in &mut self.voices {
            voice.end_note();
        }
        self.last_clip = None;
        self.last_source = None;
    }
    pub fn sample(&mut self, frame: u64) -> f32 {
        let sample = if let Some(prepared) = self
            .clips
            .iter()
            .find(|c| frame >= c.clip.start_frame && frame < c.clip.end())
        {
            let clip = &prepared.clip;
            let position = clip.source_frame(frame - clip.start_frame);
            let source = clip
                .stretch
                .map_or(position, |s| s.original_frame(position));
            let restarted = self.last_clip != Some(clip.id)
                || self.last_source.is_none_or(|last| source < last);
            if restarted {
                for voice in &mut self.voices {
                    voice.reset();
                }
                self.next_event = prepared.events.partition_point(|e| e.frame <= source);
                // Reconstruct every held note after seeks, trims, and repeat boundaries.
                // Slots and release-tail capacity were assigned before publication.
                for event in &prepared.events[..self.next_event] {
                    let voice = &mut self.voices[event.slot];
                    if let Some((pitch, velocity)) = event.on {
                        voice.note_on(event.note, pitch, velocity);
                    } else if voice.current == Some(event.note) {
                        voice.end_note();
                    }
                }
            } else {
                while let Some(event) = prepared
                    .events
                    .get(self.next_event)
                    .filter(|e| e.frame <= source)
                {
                    let voice = &mut self.voices[event.slot];
                    if let Some((pitch, velocity)) = event.on {
                        voice.note_on(event.note, pitch, velocity);
                    } else if voice.current == Some(event.note) {
                        voice.end_note();
                    }
                    self.next_event += 1;
                }
            }
            self.last_clip = Some(clip.id);
            self.last_source = Some(source);
            self.voices.iter_mut().map(Voice::sample).sum::<f32>()
                * super::sample_fade(
                    clip,
                    frame - clip.start_frame,
                    position,
                    self.sample_rate_hz,
                )
        } else {
            if self.last_clip.take().is_some() {
                // Clip fades have already reached zero; do not resurrect a release tail in a gap.
                for voice in &mut self.voices {
                    voice.reset();
                }
            }
            self.last_source = None;
            self.voices.iter_mut().map(Voice::sample).sum::<f32>()
        };
        self.held_sample = sample;
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{Edit, MidiNote, MidiPattern};
    #[test]
    fn overlapping_notes_have_independent_voices_and_note_offs() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let clip = project.add_midi_pattern(track, 0).unwrap();
        let first = Id::new_v4();
        let second = Id::new_v4();
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![
                        MidiNote {
                            id: first,
                            start_tick: 0,
                            length_ticks: 960,
                            pitch: 60,
                            velocity: 127,
                        },
                        MidiNote {
                            id: second,
                            start_tick: 480,
                            length_ticks: 1440,
                            pitch: 64,
                            velocity: 127,
                        },
                    ],
                },
            })
            .unwrap();
        project.tracks[0].clips[0].length_frames = 96_000;
        let mut runtime = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        for frame in 0..12_001 {
            runtime.sample(frame);
        }
        assert!(runtime.voices.iter().any(|v| v.current == Some(first)));
        assert!(runtime.voices.iter().any(|v| v.current == Some(second)));
        for frame in 12_001..24_001 {
            runtime.sample(frame);
        }
        assert!(!runtime.voices.iter().any(|v| v.current == Some(first)));
        assert!(runtime.voices.iter().any(|v| v.current == Some(second)));
        for frame in 24_001..48_001 {
            runtime.sample(frame);
        }
        assert!(runtime.voices.iter().all(|v| v.current.is_none()));
        for frame in 48_001..48_300 {
            runtime.sample(frame);
        }
        assert_eq!(runtime.held_sample(), 0.0);
    }
}
