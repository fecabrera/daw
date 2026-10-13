use daw_core::{Clip, Id, Project, Track, TrackKind, linear_gain};
use daw_envelope::Envelope;
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
struct LayerVoice {
    sources: Vec<OscillatorVoice>,
    envelope: Envelope,
    gate: Envelope,
    envelope_mix: f32,
    envelope_mix_target: f32,
    envelope_mix_remaining: u32,
    gain: [f32; 2],
    gain_target: [f32; 2],
    gain_remaining: u32,
}
struct Voice {
    layers: Vec<LayerVoice>,
    current: Option<Id>,
    velocity: f32,
    pitch: u8,
    reconstructed_at: u64,
    previous: [f32; 2],
    transition_from: [f32; 2],
    transition: u32,
    fade_frames: u32,
}
struct OscillatorVoice {
    source: Oscillator,
    gain: f32,
    gain_target: f32,
    gain_remaining: u32,
    transpose: i8,
    frequency: f32,
    frequency_target: f32,
    frequency_remaining: u32,
}
impl Voice {
    fn note_on(&mut self, id: Id, pitch: u8, velocity: u8) {
        let idle = self.transition == 0 && self.layers.iter().all(LayerVoice::is_idle);
        if idle {
            for layer in &mut self.layers {
                layer.gain = layer.gain_target;
                layer.gain_remaining = 0;
                layer.envelope_mix = layer.envelope_mix_target;
                layer.envelope_mix_remaining = 0;
            }
        }
        self.transition_from = self.previous;
        self.transition = self.fade_frames;
        self.current = Some(id);
        for layer in &mut self.layers {
            layer.envelope.note_on();
            layer.gate.note_on();
        }
        self.velocity = f32::from(velocity) / 127.0;
        self.pitch = pitch;
        for oscillator in self.layers.iter_mut().flat_map(|layer| &mut layer.sources) {
            oscillator.source.reset();
            oscillator.frequency = note_frequency(pitch, oscillator.transpose);
            oscillator.frequency_target = oscillator.frequency;
            oscillator.frequency_remaining = 0;
            oscillator.source.set_frequency(oscillator.frequency);
        }
    }
    fn end_note(&mut self) {
        self.current = None;
        for layer in &mut self.layers {
            layer.envelope.note_off();
            layer.gate.note_off();
        }
    }
    fn reset(&mut self) {
        self.end_note();
        for layer in &mut self.layers {
            layer.envelope.reset();
            layer.gate.reset();
            layer.envelope_mix = layer.envelope_mix_target;
            layer.envelope_mix_remaining = 0;
        }
        self.reconstructed_at = 0;
        self.previous = [0.0; 2];
        self.transition = 0;
        for oscillator in self.layers.iter_mut().flat_map(|layer| &mut layer.sources) {
            oscillator.source.reset();
        }
    }
    fn advance_envelopes(&mut self, frames: u64) {
        for layer in &mut self.layers {
            layer.envelope.advance(frames);
            layer.gate.advance(frames);
        }
    }
    fn sample(&mut self) -> [f32; 2] {
        if self.layers.iter().all(LayerVoice::is_idle) && self.transition == 0 {
            self.previous = [0.0; 2];
            return [0.0; 2];
        }
        let mut sample = [0.0; 2];
        for layer in &mut self.layers {
            let value = layer.sample();
            for ch in 0..2 {
                sample[ch] += value[ch];
            }
        }
        for value in &mut sample {
            *value *= self.velocity;
        }
        if self.transition > 0 {
            let progress = 1.0 - self.transition as f32 / self.fade_frames as f32;
            for (ch, value) in sample.iter_mut().enumerate() {
                *value = self.transition_from[ch] * (1.0 - progress) + *value * progress;
            }
            self.transition -= 1;
        }
        self.previous = sample;
        sample
    }
}
impl LayerVoice {
    fn is_idle(&self) -> bool {
        self.envelope.is_idle() && self.gate.is_idle()
    }
    fn sample(&mut self) -> [f32; 2] {
        if self.gain_remaining > 0 {
            for ch in 0..2 {
                self.gain[ch] +=
                    (self.gain_target[ch] - self.gain[ch]) / self.gain_remaining as f32;
            }
            self.gain_remaining -= 1;
        }
        if self.envelope_mix_remaining > 0 {
            self.envelope_mix +=
                (self.envelope_mix_target - self.envelope_mix) / self.envelope_mix_remaining as f32;
            self.envelope_mix_remaining -= 1;
        }
        // Keep ADSR and the short gate advancing so bypass never restarts a held note.
        let envelope = self.envelope.next_sample();
        let gate = self.gate.next_sample();
        let envelope = if self.envelope_mix >= 1.0 {
            envelope
        } else if self.envelope_mix <= 0.0 {
            gate
        } else {
            gate + (envelope - gate) * self.envelope_mix
        };
        let sources = self
            .sources
            .iter_mut()
            .map(|s| {
                if s.frequency_remaining > 0 {
                    s.frequency +=
                        (s.frequency_target - s.frequency) / s.frequency_remaining as f32;
                    s.frequency_remaining -= 1;
                    s.source.set_frequency(s.frequency);
                }
                if s.gain_remaining > 0 {
                    s.gain += (s.gain_target - s.gain) / s.gain_remaining as f32;
                    s.gain_remaining -= 1;
                }
                s.source.next_sample() * s.gain
            })
            .sum::<f32>();
        self.gain.map(|gain| sources * envelope * gain)
    }
}

/// Center is unity per channel so legacy instruments retain their original gain and track pan.
/// Muted and solo-excluded layers continue advancing all sources/envelopes with zero output.
fn layer_gains(layer: &daw_core::InstrumentLayerSettings, any_solo: bool) -> [f32; 2] {
    if !layer.oscillator_enabled || !daw_core::mix_audible(layer.muted, layer.soloed, any_solo) {
        return [0.0; 2];
    }
    let pan = super::pan_coefficients(layer.pan, true);
    let center = super::pan_coefficients(0.0, true);
    let gain = linear_gain(layer.gain_db);
    std::array::from_fn(|ch| gain * (pan[ch] / center[ch]))
}

fn note_frequency(pitch: u8, transpose: i8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(pitch) + f32::from(transpose) - 69.0) / 12.0)
}

pub(crate) struct MidiTrackRuntime {
    voices: Vec<Voice>,
    clips: Vec<PatternClip>,
    sample_rate_hz: u32,
    fingerprint: u64,
    held_sample: [f32; 2],
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
        // UI order and names do not change routing. Keep DSP order stable across row moves.
        let mut layers: Vec<_> = instrument.layers.iter().collect();
        layers.sort_by_key(|layer| layer.id);
        let any_solo = layers.iter().any(|layer| layer.soloed);
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
                    // Reserve the supported release range so live release edits never
                    // change slot assignments, steal tails, or allocate in the callback.
                    let release_frames = (f64::from(daw_core::MAX_ENVELOPE_TIME_MS)
                        * f64::from(project.sample_rate_hz)
                        / 1000.0) as u64;
                    let release_source_frames = clip.stretch.map_or(release_frames, |stretch| {
                        stretch.original_frame(release_frames).saturating_add(1)
                    });
                    let available = end.saturating_add(release_source_frames);
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
        format!("{:?}", track.clips).hash(&mut hash);
        instrument.version.hash(&mut hash);
        instrument.layers.len().hash(&mut hash);
        // Stable layer IDs preserve independent controls and identify topology changes.
        for layer in &layers {
            layer.id.hash(&mut hash);
            layer.has_envelope.hash(&mut hash);
            layer.oscillators.len().hash(&mut hash);
            for settings in &layer.oscillators {
                format!("{:?}", settings.waveform).hash(&mut hash);
            }
        }
        project.tempo_bpm.to_bits().hash(&mut hash);
        project.sample_rate_hz.hash(&mut hash);
        for clip in &track.clips {
            if let Some(asset) = project.assets.iter().find(|a| a.id == clip.asset_id) {
                format!("{:?}", asset.midi_pattern).hash(&mut hash);
            }
        }
        Some(Self {
            fingerprint: hash.finish(),
            held_sample: [0.0; 2],
            voices: (0..voice_count)
                .map(|_| Voice {
                    layers: layers
                        .iter()
                        .map(|layer| LayerVoice {
                            sources: layer
                                .oscillators
                                .iter()
                                .map(|settings| OscillatorVoice {
                                    source: Oscillator::new(
                                        settings.waveform,
                                        ProcessingContext {
                                            sample_rate_hz: project.sample_rate_hz,
                                            max_block_frames: project.buffer_size_frames,
                                        },
                                    ),
                                    // Muted sources still advance; retain their saved gain for unmuting.
                                    gain: if settings.muted {
                                        0.0
                                    } else {
                                        linear_gain(settings.gain_db)
                                    },
                                    gain_target: if settings.muted {
                                        0.0
                                    } else {
                                        linear_gain(settings.gain_db)
                                    },
                                    gain_remaining: 0,
                                    transpose: settings.transpose,
                                    frequency: 440.0,
                                    frequency_target: 440.0,
                                    frequency_remaining: 0,
                                })
                                .collect(),
                            gain: layer_gains(layer, any_solo),
                            gain_target: layer_gains(layer, any_solo),
                            gain_remaining: 0,
                            gate: Envelope::new(
                                daw_core::EnvelopeSettings::legacy(),
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            ),
                            envelope_mix: f32::from(layer.envelope_enabled),
                            envelope_mix_target: f32::from(layer.envelope_enabled),
                            envelope_mix_remaining: 0,
                            envelope: Envelope::new(
                                layer.effective_envelope(),
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            ),
                        })
                        .collect(),
                    current: None,
                    velocity: 0.0,
                    pitch: 69,
                    reconstructed_at: 0,
                    previous: [0.0; 2],
                    transition_from: [0.0; 2],
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
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    /// Retarget compatible voices before swapping them into the new renderer.
    /// All storage stays owned by the two prepared renderers; no DSP state is dropped here.
    pub fn retarget_controls(&mut self, prepared: &Self) {
        let fade_frames = (self.sample_rate_hz / 200).max(1);
        for (voice, next) in self.voices.iter_mut().zip(&prepared.voices) {
            let idle = voice.transition == 0 && voice.layers.iter().all(LayerVoice::is_idle);
            for (layer, next_layer) in voice.layers.iter_mut().zip(&next.layers) {
                layer.envelope.set_settings(next_layer.envelope.settings());
                if layer.envelope_mix_target != next_layer.envelope_mix_target {
                    layer.envelope_mix_target = next_layer.envelope_mix_target;
                    if idle {
                        layer.envelope_mix = next_layer.envelope_mix_target;
                        layer.envelope_mix_remaining = 0;
                    } else {
                        layer.envelope_mix_remaining = fade_frames;
                    }
                }
                if layer.gain_target != next_layer.gain_target {
                    layer.gain_target = next_layer.gain_target;
                    if idle {
                        // Future notes start at the new controls, including exact mute.
                        layer.gain = next_layer.gain_target;
                        layer.gain_remaining = 0;
                    } else {
                        layer.gain_remaining = fade_frames;
                    }
                }
                for (source, target) in layer.sources.iter_mut().zip(&next_layer.sources) {
                    if source.transpose != target.transpose {
                        source.transpose = target.transpose;
                        source.frequency_target = note_frequency(voice.pitch, source.transpose);
                        source.frequency_remaining = fade_frames;
                    }
                    if source.gain_target != target.gain_target {
                        source.gain_target = target.gain_target;
                        source.gain_remaining = fade_frames;
                    }
                }
            }
        }
    }
    fn voice_sample(&mut self) -> [f32; 2] {
        let mut sum = [0.0; 2];
        for voice in &mut self.voices {
            let sample = voice.sample();
            for ch in 0..2 {
                sum[ch] += sample[ch];
            }
        }
        sum
    }
    pub fn held_sample(&self) -> [f32; 2] {
        self.held_sample
    }
    pub fn reset(&mut self) {
        self.held_sample = [0.0; 2];
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
    pub fn sample(&mut self, frame: u64) -> [f32; 2] {
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
                    let event_frame = clip.stretch.map_or(event.frame, |stretch| {
                        // Events fire when the floor-mapped source clock reaches their frame.
                        (u128::from(event.frame) * u128::from(stretch.output_frames))
                            .div_ceil(u128::from(stretch.source_frames))
                            .min(u128::from(u64::MAX)) as u64
                    });
                    voice.advance_envelopes(event_frame.saturating_sub(voice.reconstructed_at));
                    if let Some((pitch, velocity)) = event.on {
                        voice.note_on(event.note, pitch, velocity);
                    } else if voice.current == Some(event.note) {
                        voice.end_note();
                    }
                    voice.reconstructed_at = event_frame;
                }
                for voice in &mut self.voices {
                    voice.advance_envelopes(position.saturating_sub(voice.reconstructed_at));
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
            let fade = super::sample_fade(
                clip,
                frame - clip.start_frame,
                position,
                self.sample_rate_hz,
            );
            self.voice_sample().map(|sample| sample * fade)
        } else {
            if self.last_clip.take().is_some() {
                // Clip fades have already reached zero; do not resurrect a release tail in a gap.
                for voice in &mut self.voices {
                    voice.reset();
                }
            }
            self.last_source = None;
            self.voice_sample()
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
        project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
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
        for frame in 48_001..60_300 {
            runtime.sample(frame);
        }
        assert_eq!(runtime.held_sample(), [0.0; 2]);
    }

    #[test]
    fn seeks_reconstruct_adsr_in_project_time_for_stretched_and_trimmed_clips() {
        for stretch in [
            None,
            daw_core::ClipStretch::new(2, 3),
            daw_core::ClipStretch::new(8, 1),
        ] {
            let mut project = Project::default();
            let track = project.add_midi_track().unwrap();
            let clip = project.add_midi_pattern(track, 0).unwrap();
            let note = Id::new_v4();
            project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: clip,
                    pattern: MidiPattern {
                        length_ticks: 3840,
                        notes: vec![MidiNote {
                            id: note,
                            start_tick: 0,
                            length_ticks: 1920,
                            pitch: 60,
                            velocity: 127,
                        }],
                    },
                })
                .unwrap();
            project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
            let instrument = project.tracks[0].instrument.as_mut().unwrap();
            instrument.layers[0].envelope.attack.time_ms = 200.0;
            instrument.layers[0].envelope.release.time_ms = 500.0;
            let mut layer = daw_core::InstrumentLayerSettings::default();
            layer.oscillators.push(Default::default());
            layer.has_envelope = true;
            layer.envelope.attack.time_ms = 350.0;
            layer.envelope.release.time_ms = 800.0;
            instrument.layers.push(layer);
            project.tracks[0].clips[0].stretch = stretch;
            project.tracks[0].clips[0].length_frames =
                stretch.map_or(96_000, |s| s.scale(96_000).unwrap());
            for frame in [1000, 4000, 8000] {
                let mut played = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                for position in 0..=frame {
                    played.sample(position);
                }
                let mut sought = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                sought.sample(frame);
                let played = played.voices.iter().find(|v| v.pitch == 60).unwrap();
                let sought = sought.voices.iter().find(|v| v.pitch == 60).unwrap();
                for (sought, played) in sought.layers.iter().zip(&played.layers) {
                    assert_eq!(sought.envelope.stage(), played.envelope.stage());
                    assert!((sought.envelope.level() - played.envelope.level()).abs() < 1e-6);
                }
            }
            // Trimming starts inside the source; a seek reconstructs its existing envelope.
            let clip = &mut project.tracks[0].clips[0];
            clip.source_offset_frame = 1000;
            clip.length_frames -= 1000;
            let mut trimmed = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
            trimmed.sample(0);
            let voice = trimmed
                .voices
                .iter()
                .find(|v| v.current == Some(note))
                .unwrap();
            assert!(voice.layers[0].envelope.level() > 0.0);
        }
    }
}
