//! Worker-only previews rendered by the normal instrument and bus graph.
use super::*;

impl Session {
    /// Render an isolated chord without changing the session or transport.
    /// Limit the gate to eight seconds, followed by one second of release/effect tail.
    pub fn audition_chord(
        &self,
        track: Id,
        chord: daw_core::Chord,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Vec<[f32; 2]>> {
        self.audition_pattern(
            track,
            chord.pattern().map_err(error)?,
            8 * u64::from(self.project.sample_rate_hz),
            cancelled,
        )
    }

    /// Render a short isolated MIDI note through the selected instrument and buses.
    pub fn audition_note(
        &self,
        track: Id,
        pitch: u8,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Vec<[f32; 2]>> {
        if pitch > 127 {
            return Err(error("MIDI pitch must be in 0–127"));
        }
        let ticks = daw_core::MIDI_TICKS_PER_BEAT;
        let pattern = daw_core::MidiPattern {
            length_ticks: ticks,
            notes: vec![daw_core::MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: ticks,
                pitch,
                velocity: 100,
            }],
        };
        self.audition_pattern(
            track,
            pattern,
            u64::from(self.project.sample_rate_hz) / 2,
            cancelled,
        )
    }

    fn audition_pattern(
        &self,
        track: Id,
        pattern: daw_core::MidiPattern,
        maximum_gate: u64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Vec<[f32; 2]>> {
        let (mut renderer, length) =
            self.audition_renderer(track, pattern, maximum_gate, cancelled)?;
        let rate = u64::from(self.project.sample_rate_hz);
        let mut samples = vec![[0.0; 2]; length as usize];
        for block in samples.chunks_mut(512) {
            if cancelled() {
                return Err(error("MIDI audition cancelled"));
            }
            renderer.render_offline(block);
        }
        // Bound tails and avoid a discontinuity at either end, including continuous sources.
        let fade = (rate / 200).max(1) as usize;
        let count = samples.len();
        for (i, sample) in samples.iter_mut().enumerate() {
            let gain = (i as f32 / fade as f32)
                .min((count - 1 - i) as f32 / fade as f32)
                .min(1.0);
            for channel in sample {
                *channel *= gain;
            }
        }
        Ok(samples)
    }
    fn audition_renderer(
        &self,
        track: Id,
        pattern: daw_core::MidiPattern,
        maximum_gate: u64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<(daw_engine::Renderer, u64)> {
        self.project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .filter(|t| t.kind == daw_core::TrackKind::Midi)
            .ok_or_else(|| error("Select a MIDI track to audition notes"))?;
        if cancelled() {
            return Err(error("MIDI audition cancelled"));
        }
        let mut session = self.clone();
        // Keep the selected track's group path, including its mix controls and effects.
        session
            .project
            .tracks
            .retain(|t| t.id == track || self.project.track_is_descendant(track, t.id));
        for t in &mut session.project.tracks {
            t.clips.clear();
        }
        // Other soloed tracks must not gate this isolated preview. Local mute remains audible behavior.
        for t in &mut session.project.tracks {
            t.soloed = false;
        }
        session.project.transport.playhead_frame = 0;
        session.project.transport.r#loop.enabled = false;
        let frames = session.project.midi_frame(pattern.length_ticks);
        let clip_id = session.project.add_midi_pattern(track, 0).map_err(error)?;
        session
            .project
            .edit(daw_core::Edit::ReplaceMidiPattern { clip_id, pattern })
            .map_err(error)?;
        let rate = u64::from(session.project.sample_rate_hz);
        let clip = session
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .clips
            .iter_mut()
            .find(|c| c.id == clip_id)
            .unwrap();
        clip.length_frames = frames.min(maximum_gate).max(1);
        let length = clip.length_frames + rate;
        session.prepare_stretches_cancellable(cancelled)?;
        let renderer = daw_engine::Renderer::new(session.plan());
        Ok((renderer, length))
    }

    /// Prepare one held note, with unknown-duration envelope timing and bounded chunk output.
    pub fn held_note_audition(
        &self,
        track: Id,
        pitch: u8,
        cancelled: &impl Fn() -> bool,
    ) -> Result<NoteAudition> {
        if pitch > 127 {
            return Err(error("MIDI pitch must be in 0–127"));
        }
        let ticks = daw_core::MIDI_TICKS_PER_BEAT;
        let pattern = daw_core::MidiPattern {
            length_ticks: ticks,
            notes: vec![daw_core::MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: ticks,
                pitch,
                velocity: 100,
            }],
        };
        let rate = self.project.sample_rate_hz;
        let minimum = self
            .project
            .midi_frame(ticks)
            .min(u64::from(rate) / 2)
            .max(1);
        let (mut renderer, _) = self.audition_renderer(track, pattern, minimum, cancelled)?;
        renderer.begin_note_audition(track, pitch);
        Ok(NoteAudition {
            renderer,
            rate,
            minimum,
            frame: 0,
            release_frame: None,
            block: self.project.buffer_size_frames as usize,
        })
    }
}

/// Worker-owned instrument history for press/release audition. No full held-note buffer.
pub struct NoteAudition {
    renderer: daw_engine::Renderer,
    rate: u32,
    minimum: u64,
    frame: u64,
    release_frame: Option<u64>,
    block: usize,
}
impl NoteAudition {
    /// Adapt worker batches to the device callback, with a fixed memory bound.
    pub fn set_chunk_frames(&mut self, frames: usize) {
        self.block = frames.clamp(1, daw_engine::NOTE_AUDITION_MAX_CHUNK_FRAMES);
    }
    /// Render the next bounded block. An early release waits for the previous preview gate.
    pub fn next_chunk(&mut self, released: bool) -> Option<Vec<[f32; 2]>> {
        if released && self.release_frame.is_none() && self.frame >= self.minimum {
            self.renderer.release_note_audition();
            self.release_frame = Some(self.frame);
        }
        let end = self
            .release_frame
            .map(|frame| frame.saturating_add(u64::from(self.rate)));
        if end.is_some_and(|end| self.frame >= end) {
            return None;
        }
        let mut count = self.block;
        if released && self.frame < self.minimum {
            count = count.min((self.minimum - self.frame) as usize);
        }
        if let Some(end) = end {
            count = count.min((end - self.frame) as usize);
        }
        let mut samples = vec![[0.0; 2]; count];
        self.renderer.render_offline(&mut samples);
        let fade = u64::from((self.rate / 200).max(1));
        for (i, sample) in samples.iter_mut().enumerate() {
            let frame = self.frame + i as u64;
            let gain = (frame as f32 / fade as f32)
                .min(1.0)
                .min(end.map_or(1.0, |end| (end - frame - 1) as f32 / fade as f32));
            sample.iter_mut().for_each(|value| *value *= gain);
        }
        self.frame = self.frame.saturating_add(count as u64);
        Some(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{Chord, ChordKind, Edit, OscillatorSettings};
    fn session() -> (Session, Id) {
        let mut session = Session::default();
        let track = session.project.add_midi_track().unwrap();
        session.project.tracks[0].instrument = Some(OscillatorSettings::default().into());
        session.project.transport.playhead_frame = 12345;
        (session, track)
    }
    #[test]
    fn held_notes_keep_rendering_until_release_and_quick_clicks_keep_the_minimum() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let (mut session, track) = session();
            session.project.sample_rate_hz = rate;
            let before = session.project.clone();
            let mut quick = session.held_note_audition(track, 60, &|| false).unwrap();
            quick.set_chunk_frames(rate as usize + 17);
            let mut quick_samples = Vec::new();
            while let Some(chunk) = quick.next_chunk(true) {
                quick_samples.extend(chunk);
            }
            assert_eq!(quick_samples.len(), (rate + rate / 2) as usize);
            assert_eq!(*quick_samples.last().unwrap(), [0.0; 2]);
            let mut held = session.held_note_audition(track, 60, &|| false).unwrap();
            held.set_chunk_frames(rate as usize / 200 + 3);
            let mut frames = 0;
            let mut last = Vec::new();
            while frames < rate as usize * 2 {
                last = held.next_chunk(false).unwrap();
                frames += last.len();
            }
            assert!(last.iter().flatten().any(|s| s.abs() > 0.01));
            let mut tail = 0;
            while let Some(chunk) = held.next_chunk(true) {
                tail += chunk.len();
            }
            assert_eq!(tail, rate as usize);
            assert_eq!(session.project, before);
        }
    }

    #[test]
    fn note_preview_is_bounded_at_all_rates_and_keeps_the_live_project() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let (mut session, track) = session();
            session.project.sample_rate_hz = rate;
            let before = session.project.clone();
            let samples = session.audition_note(track, 60, &|| false).unwrap();
            assert_eq!(samples.len(), (rate + rate / 2) as usize);
            assert!(samples.iter().flatten().all(|s| s.is_finite()));
            assert!(samples.iter().flatten().any(|s| s.abs() > 0.01));
            assert_eq!(samples[0], [0.0; 2]);
            assert_eq!(*samples.last().unwrap(), [0.0; 2]);
            assert_eq!(session.project, before);
            assert!(session.audition_note(track, 128, &|| false).is_err());
            assert!(session.audition_note(track, 60, &|| true).is_err());
        }
    }

    #[test]
    fn chord_preview_uses_selected_instrument_and_bus_mix_without_changing_project() {
        let (mut session, track) = session();
        let other = session.project.add_midi_track().unwrap();
        session.project.tracks[1].soloed = true;
        session
            .project
            .add_midi_chords(
                Some(other),
                0,
                &[Chord {
                    root: 9,
                    kind: ChordKind::Minor,
                }],
            )
            .unwrap();
        let group = Id::new_v4();
        session
            .project
            .edit(Edit::GroupTracks {
                track_ids: vec![track],
                group_id: group,
                name: "Bus".into(),
            })
            .unwrap();
        let before = serde_json::to_string(&session.project).unwrap();
        let chord = Chord {
            root: 0,
            kind: ChordKind::Major,
        };
        let samples = session.audition_chord(track, chord, &|| false).unwrap();
        assert_eq!(samples.len(), 144_000);
        assert!(samples.iter().flatten().any(|s| s.abs() > 0.01));
        assert_eq!(samples[0], [0.0; 2]);
        assert_eq!(*samples.last().unwrap(), [0.0; 2]);
        assert_eq!(serde_json::to_string(&session.project).unwrap(), before);
        session
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == group)
            .unwrap()
            .gain_db = -6.0;
        let quieter = session.audition_chord(track, chord, &|| false).unwrap();
        let gain = daw_core::linear_gain(-6.0);
        for (a, b) in samples.iter().zip(&quieter) {
            for ch in 0..2 {
                assert!((a[ch] * gain - b[ch]).abs() < 0.00001);
            }
        }
        session
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .muted = true;
        assert!(
            session
                .audition_chord(track, chord, &|| false)
                .unwrap()
                .iter()
                .flatten()
                .all(|s| *s == 0.0)
        );
    }
    #[test]
    fn preview_rejects_non_midi_targets_and_cancellation_and_caps_slow_tempo() {
        let (mut session, track) = session();
        let chord = Chord {
            root: 0,
            kind: ChordKind::Major,
        };
        assert!(session.audition_chord(track, chord, &|| true).is_err());
        let audio = session.project.add_track().unwrap();
        assert!(session.audition_chord(audio, chord, &|| false).is_err());
        assert!(
            session
                .audition_chord(Id::new_v4(), chord, &|| false)
                .is_err()
        );
        session.project.tempo_bpm = 10.0;
        assert_eq!(
            session
                .audition_chord(track, chord, &|| false)
                .unwrap()
                .len(),
            9 * 48_000
        );
    }
}
