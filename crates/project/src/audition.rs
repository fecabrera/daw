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
        self.project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .filter(|t| t.kind == daw_core::TrackKind::Midi)
            .ok_or_else(|| error("Select a MIDI track to audition chords"))?;
        if cancelled() {
            return Err(error("Chord audition cancelled"));
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
        let (_, clips) = session
            .project
            .add_midi_chords(Some(track), 0, &[chord])
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
            .find(|c| c.id == clips[0])
            .unwrap();
        clip.length_frames = clip.length_frames.min(rate * 8);
        let length = clip.length_frames + rate;
        session.prepare_stretches_cancellable(cancelled)?;
        let mut renderer = daw_engine::Renderer::new(session.plan());
        let mut samples = vec![[0.0; 2]; length as usize];
        for block in samples.chunks_mut(512) {
            if cancelled() {
                return Err(error("Chord audition cancelled"));
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
