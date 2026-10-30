use super::continuous_modulation::ProviderControl;
use super::modulation::{Control, Route, SharedControls};
use daw_core::{Clip, Id, Project, Track, TrackKind, linear_gain};
use daw_core::{EnvelopeMode, ModulationSourceId, ModulationValue};
use daw_envelope::Envelope;
use daw_oscillator::Oscillator;
use daw_unit_core::{ModulationContext, process_modulation};
use daw_unit_core::{ProcessingContext, SourceUnit};

#[derive(Clone)]
struct NoteEvent {
    frame: u64,
    note: Id,
    slot: usize,
    on: Option<(u8, u8)>,
    duration_frames: u64,
    pitch_index: usize,
}
#[derive(Clone)]
struct PatternClip {
    clip: Clip,
    events: Vec<NoteEvent>,
    note_states: Vec<super::modulation::NoteState>,
    slot_starts: Vec<Vec<usize>>,
    pitches: Vec<NotePitch>,
}
#[derive(Clone)]
struct NotePitch {
    note: Id,
    base: u8,
    input: super::live_midi_pitch::PitchInput,
}
#[derive(Clone)]
struct LayerVoice {
    // Display-only contribution for the legacy voice-mix path, including its post-gain transition.
    mixed_output: [f32; 2],
    mixed_transition_from: [f32; 2],
    previous: [f32; 2],
    transition_from: [f32; 2],
    output: [f32; 2],
    sources: Vec<OscillatorVoice>,
    oscillator_level: daw_oscillator::OscillatorOutput,
    oscillator_level_routes: Vec<Route>,
    sampler: Option<daw_sampler::Sampler>,
    sampler_routes: Vec<Route>,
    one_shot: bool,
    one_shot_target: bool,
    gate: Envelope,
    amplitude: daw_amplitude::Amplitude,
    routes: Vec<Route>,
    gain: [f32; 2],
    gain_target: [f32; 2],
    gain_remaining: u32,
}
#[derive(Clone)]
struct Voice {
    active_last_sample: bool,
    sleeping: bool,
    layers: Vec<LayerVoice>,
    controls: Vec<Control>,
    control_order: Vec<usize>,
    values: Vec<ModulationValue>,
    local_sources: daw_unit_core::ModulationSourceIndex,
    current: Option<Id>,
    pitch_index: Option<usize>,
    velocity: f32,
    pitch: u8,
    reconstructed_at: u64,
    previous: [f32; 2],
    transition_from: [f32; 2],
    transition: u32,
    fade_frames: u32,
    held_frames: u64,
    note_duration_frames: Option<u64>,
    release_frames: u64,
}
#[derive(Clone)]
struct OscillatorVoice {
    id: Id,
    routes: Vec<Route>,
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
    fn note_on(&mut self, id: Id, pitch: u8, velocity: u8, duration_frames: u64) {
        self.note_on_with_duration(id, pitch, velocity, Some(duration_frames.max(1)));
    }
    fn note_on_with_duration(
        &mut self,
        id: Id,
        pitch: u8,
        velocity: u8,
        duration_frames: Option<u64>,
    ) {
        self.sleeping = false;
        let idle = self.transition == 0
            && self.layers.iter().all(LayerVoice::is_idle)
            && self.controls.iter().all(|e| e.envelope.is_idle());
        if idle {
            for layer in &mut self.layers {
                layer.gain = layer.gain_target;
                layer.gain_remaining = 0;
                layer.oscillator_level.reset();
                layer.amplitude.reset();
            }
        }
        self.transition_from = self.previous;
        for layer in &mut self.layers {
            layer.transition_from = layer.previous;
            layer.mixed_transition_from = layer.mixed_output;
        }
        self.transition = self.fade_frames;
        self.current = Some(id);
        self.held_frames = 0;
        self.note_duration_frames = duration_frames;
        self.release_frames = 0;
        for e in &mut self.controls {
            e.note_on(self.note_duration_frames);
        }
        for layer in &mut self.layers {
            layer.one_shot = layer.one_shot_target;
            layer.oscillator_level.note_on();
            for route in &mut layer.oscillator_level_routes {
                route.reset();
            }
            layer.gate.note_on();
            if let Some(s) = &mut layer.sampler {
                s.note_on_at(pitch);
                for route in &mut layer.sampler_routes {
                    route.reset();
                }
            }
        }
        self.velocity = f32::from(velocity) / 127.0;
        self.pitch = pitch;
        for oscillator in self.layers.iter_mut().flat_map(|layer| &mut layer.sources) {
            oscillator.source.reset();
            for route in &mut oscillator.routes {
                route.reset();
            }
            oscillator.frequency = note_frequency(pitch, oscillator.transpose);
            oscillator.frequency_target = oscillator.frequency;
            oscillator.frequency_remaining = 0;
            oscillator.source.set_frequency(oscillator.frequency);
        }
    }
    fn retune_pitch(&mut self, pitch: u8, preview: bool) {
        if self.pitch != pitch {
            self.pitch = pitch;
            for source in self.layers.iter_mut().flat_map(|l| &mut l.sources) {
                source.frequency_target = note_frequency(pitch, source.transpose);
                source.frequency_remaining = self.fade_frames;
            }
        }
        for sampler in self.layers.iter_mut().filter_map(|l| l.sampler.as_mut()) {
            sampler.set_live_pitch(pitch, preview);
        }
    }
    fn end_note(&mut self) {
        self.current = None;
        self.release_frames = 0;
        for e in &mut self.controls {
            e.envelope.note_off();
        }
        for layer in &mut self.layers {
            layer.gate.note_off();
        }
    }
    fn reset(&mut self) {
        self.active_last_sample = false;
        self.sleeping = true;
        self.pitch_index = None;
        self.end_note();
        for e in &mut self.controls {
            e.reset();
        }
        for layer in &mut self.layers {
            layer.previous = [0.0; 2];
            layer.output = [0.0; 2];
            layer.mixed_output = [0.0; 2];
            layer.gate.reset();
            if let Some(s) = &mut layer.sampler {
                s.reset();
            }
            for route in &mut layer.sampler_routes {
                route.reset();
            }
            layer.oscillator_level.reset();
            for route in &mut layer.oscillator_level_routes {
                route.reset();
            }
            layer.amplitude.reset();
            for r in &mut layer.routes {
                r.reset();
            }
        }
        self.reconstructed_at = 0;
        self.note_duration_frames = None;
        self.held_frames = 0;
        self.release_frames = 0;
        self.previous = [0.0; 2];
        self.transition = 0;
        for oscillator in self.layers.iter_mut().flat_map(|layer| &mut layer.sources) {
            oscillator.source.reset();
            for route in &mut oscillator.routes {
                route.reset();
            }
        }
    }
    fn advance_envelopes(&mut self, frames: u64) {
        if frames == 0 {
            return;
        }
        if self.current.is_some() {
            self.held_frames = self.held_frames.saturating_add(frames);
        } else {
            self.release_frames = self.release_frames.saturating_add(frames);
        }
        for e in &mut self.controls {
            e.envelope.advance(frames);
        }
        for layer in &mut self.layers {
            layer.gate.advance(frames);
            if let Some(s) = &mut layer.sampler {
                s.advance(frames);
            }
        }
    }
    fn sample(
        &mut self,
        shared: &[ModulationValue],
        shared_sources: &daw_unit_core::ModulationSourceIndex,
        shared_release: Option<u64>,
    ) -> [f32; 2] {
        if self.current.is_some() {
            self.held_frames = self.held_frames.saturating_add(1);
        } else {
            self.release_frames = self.release_frames.saturating_add(1);
        }
        let shared_tail = shared_release == Some(self.release_frames);
        if !shared_tail
            && (self.sleeping
                || (self.controls.iter().all(|e| e.envelope.is_idle())
                    && self.layers.iter().all(LayerVoice::is_idle)
                    && self.transition == 0))
        {
            self.sleeping = true;
            self.active_last_sample = false;
            self.previous = [0.0; 2];
            for layer in &mut self.layers {
                layer.output = [0.0; 2];
                layer.mixed_output = [0.0; 2];
                layer.previous = [0.0; 2];
            }
            return [0.0; 2];
        }
        self.sleeping = false;
        self.active_last_sample = true;
        for &slot in &self.control_order {
            self.values[slot] = self.controls[slot].tick(
                &self.values,
                shared,
                ModulationContext {
                    local_sources: Some(&self.local_sources),
                    shared_sources: Some(shared_sources),
                    ..Default::default()
                },
            );
        }
        let mut sample = [0.0; 2];
        for layer in &mut self.layers {
            let value = layer.sample(
                &self.values,
                shared,
                &self.local_sources,
                shared_sources,
                if self.current.is_none() {
                    Some(self.release_frames)
                } else {
                    None
                },
            );
            layer.mixed_output = std::array::from_fn(|ch| {
                let sample = value[ch] * layer.gain[ch] * self.velocity;
                if self.transition > 0 {
                    let progress = 1.0 - self.transition as f32 / self.fade_frames as f32;
                    layer.mixed_transition_from[ch] * (1.0 - progress) + sample * progress
                } else {
                    sample
                }
            });
            layer.output = value.map(|v| v * self.velocity);
            if self.transition > 0 {
                let progress = 1.0 - self.transition as f32 / self.fade_frames as f32;
                for ch in 0..2 {
                    layer.output[ch] =
                        layer.transition_from[ch] * (1.0 - progress) + layer.output[ch] * progress;
                }
            }
            layer.previous = layer.output;
            for ch in 0..2 {
                sample[ch] += value[ch] * layer.gain[ch];
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
        self.gate.is_idle() && !(self.one_shot && self.sampler.as_ref().is_some_and(|s| s.active()))
    }
    fn sample(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
        local_sources: &daw_unit_core::ModulationSourceIndex,
        shared_sources: &daw_unit_core::ModulationSourceIndex,
        release_age: Option<u64>,
    ) -> [f32; 2] {
        if self.gain_remaining > 0 {
            for ch in 0..2 {
                self.gain[ch] +=
                    (self.gain_target[ch] - self.gain[ch]) / self.gain_remaining as f32;
            }
            self.gain_remaining -= 1;
        }
        let base_gate = self.gate.next_sample();
        let gate = if self.one_shot && self.sampler.as_ref().is_some_and(|s| s.active()) {
            1.0
        } else {
            base_gate
        };
        let context = ModulationContext {
            local_sources: Some(local_sources),
            shared_sources: Some(shared_sources),
            note_gate: gate,
            release_age,
            ..Default::default()
        };
        let output_gain = process_modulation(
            &mut self.oscillator_level,
            &mut self.oscillator_level_routes,
            1.0,
            local,
            shared,
            context,
        );
        let output_pitch_ratio = self.oscillator_level.pitch_ratio();
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
                s.source.set_base_gain(s.gain);
                s.source.set_output_pitch_ratio(output_pitch_ratio);
                process_modulation(&mut s.source, &mut s.routes, 0.0, local, shared, context)
            })
            .sum::<f32>();
        let sources = sources * output_gain;
        let mut stereo = self.sampler.as_mut().map_or([sources; 2], |s| {
            process_modulation(
                s,
                &mut self.sampler_routes,
                [0.0; 2],
                local,
                shared,
                context,
            )
        });
        let shaped = process_modulation(
            &mut self.amplitude,
            &mut self.routes,
            1.0,
            local,
            shared,
            context,
        );
        for value in &mut stereo {
            *value *= shaped;
        }
        stereo
    }
}

/// Center is unity per channel so legacy instruments retain their original gain and track pan.
/// Muted and solo-excluded layers continue advancing all sources/envelopes with zero output.
fn layer_gains(layer: &daw_core::InstrumentLayerSettings, any_solo: bool) -> [f32; 2] {
    if (layer.sampler.is_none() && !layer.oscillator_enabled)
        || !daw_core::mix_audible(layer.muted, layer.soloed, any_solo)
    {
        return [0.0; 2];
    }
    bus_gains(layer.gain_db, layer.pan)
}
fn bus_gains(gain_db: f32, pan: f32) -> [f32; 2] {
    let pan = super::pan_coefficients(pan, true);
    let center = super::pan_coefficients(0.0, true);
    let gain = linear_gain(gain_db);
    std::array::from_fn(|ch| gain * (pan[ch] / center[ch]))
}

fn output_frame(clip: &Clip, frame: u64) -> u64 {
    clip.stretch.map_or(frame, |s| {
        (u128::from(frame) * u128::from(s.output_frames))
            .div_ceil(u128::from(s.source_frames))
            .min(u128::from(u64::MAX)) as u64
    })
}

fn note_frequency(pitch: u8, transpose: i8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(pitch) + f32::from(transpose) - 69.0) / 12.0)
}

fn clip_pitch(pitch: u8, transpose: i8) -> u8 {
    i16::from(pitch)
        .saturating_add(i16::from(transpose))
        .clamp(0, 127) as u8
}

#[derive(Clone)]
pub(crate) struct MidiTrackRuntime {
    // Regular buses follow canonical layer order; Main is always the final bus.
    layer_effects: Vec<super::layer_effects::LayerEffects>,
    layer_samples: Vec<[f32; 2]>,
    effect_latency: u64,
    envelope_history: bool,
    parallel_latency: u64,
    has_bus_effects: bool,
    layer_index: std::collections::HashMap<Id, usize>,
    oscillator_index: std::collections::HashMap<(Id, Id), usize>,
    voices: Vec<Voice>,
    clips: Vec<PatternClip>,
    clip_ranges: super::schedule::Ranges,
    sample_rate_hz: u32,
    fingerprint: u64,
    held_sample: [f32; 2],
    last_clip: Option<Id>,
    last_source: Option<u64>,
    next_event: usize,
    shared: SharedControls,
    lfo_notes: super::lfo::NoteSchedule,
    tempo_bpm: f32,
    main_gain: [f32; 2],
    main_target: [f32; 2],
    main_remaining: u32,
}
impl MidiTrackRuntime {
    pub fn memory_bytes(&self) -> (u64, u64) {
        use daw_unit_core::memory::{map_bytes, vec_bytes};
        let schedules = vec_bytes(&self.clips)
            + self
                .clips
                .iter()
                .map(|c| {
                    c.clip.name.capacity() as u64
                        + vec_bytes(&c.pitches)
                        + vec_bytes(&c.events)
                        + vec_bytes(&c.note_states)
                        + vec_bytes(&c.slot_starts)
                        + c.slot_starts.iter().map(vec_bytes).sum::<u64>()
                })
                .sum::<u64>()
            + self.clip_ranges.heap_bytes()
            + self.lfo_notes.heap_bytes();
        let processors = vec_bytes(&self.layer_effects)
            + self
                .layer_effects
                .iter()
                .map(|b| b.heap_bytes())
                .sum::<u64>()
            + vec_bytes(&self.layer_samples)
            + map_bytes(&self.layer_index)
            + map_bytes(&self.oscillator_index)
            + self.shared.heap_bytes()
            + vec_bytes(&self.voices)
            + self
                .voices
                .iter()
                .map(|v| {
                    vec_bytes(&v.controls)
                        + v.controls.iter().map(|c| vec_bytes(&c.routes)).sum::<u64>()
                        + vec_bytes(&v.control_order)
                        + vec_bytes(&v.values)
                        + v.local_sources.heap_bytes()
                        + vec_bytes(&v.layers)
                        + v.layers
                            .iter()
                            .map(|l| {
                                vec_bytes(&l.routes)
                                    + vec_bytes(&l.sampler_routes)
                                    + vec_bytes(&l.oscillator_level_routes)
                                    + l.sampler.as_ref().map_or(0, |s| s.heap_bytes())
                                    + vec_bytes(&l.sources)
                                    + l.sources
                                        .iter()
                                        .map(|o| vec_bytes(&o.routes) + o.source.heap_bytes())
                                        .sum::<u64>()
                            })
                            .sum::<u64>()
                })
                .sum::<u64>();
        (schedules, processors)
    }

    /// Build schedules and DSP state before publication. No callback allocations.
    #[cfg(test)]
    pub fn new(project: &Project, track: &Track) -> Option<Self> {
        Self::with_audio(
            project,
            track,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
        )
    }
    pub fn with_audio(
        project: &Project,
        track: &Track,
        audio: &std::collections::HashMap<Id, daw_media::AudioData>,
        sampler_audio: &std::collections::HashMap<Id, daw_media::PreparedSampler>,
    ) -> Option<Self> {
        if track.kind != TrackKind::Midi {
            return None;
        }
        let instrument = track.instrument.as_ref()?;
        // UI order and names do not change routing. Keep DSP order stable across row moves.
        let mut layers: Vec<_> = instrument.layers.iter().collect();
        layers.sort_by_key(|layer| layer.id);
        let banks: Vec<_> = layers
            .iter()
            .map(|layer| {
                layer.sampler.as_ref().map(|s| {
                    let mut bank: daw_sampler::SampleBank = std::array::from_fn(|_| None);
                    if let Some(prepared) = sampler_audio.get(&s.id) {
                        for (offset, audio) in prepared.variants() {
                            let note = i16::from(s.root_note) + i16::from(offset);
                            if (0..=127).contains(&note) {
                                bank[note as usize] = Some(audio.samples.clone());
                            }
                        }
                    } else if let Some(audio) = audio.get(&s.id) {
                        // Root-only callers retain their unshifted path. Never substitute it
                        // for a missing non-root variant.
                        bank[usize::from(s.root_note)] = Some(audio.samples.clone());
                    }
                    std::sync::Arc::new(bank)
                })
            })
            .collect();
        let any_solo = layers.iter().any(|layer| layer.soloed);
        let fade_frames = (project.sample_rate_hz / 200).max(1);
        let mut voice_count = 0;
        let clips: Vec<PatternClip> = track
            .clips
            .iter()
            .filter(|clip| !clip.muted)
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
                let pitches = ordered
                    .iter()
                    .map(|n| NotePitch {
                        note: n.id,
                        base: n.pitch,
                        input: Default::default(),
                    })
                    .collect();
                for (pitch_index, note) in ordered.into_iter().enumerate() {
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
                    let release_frames = release_frames.max(
                        layers
                            .iter()
                            .filter_map(|l| l.sampler.as_ref())
                            .filter_map(|s| {
                                audio.get(&s.id).map(|a| {
                                    let tail = if s.granulizer.is_some() {
                                        u64::from(project.sample_rate_hz / 2)
                                    } else {
                                        0
                                    };
                                    (a.samples.len() as u64).saturating_add(tail)
                                })
                            })
                            .max()
                            .unwrap_or(0),
                    );
                    let release_source_frames = clip.stretch.map_or(release_frames, |stretch| {
                        stretch.original_frame(release_frames).saturating_add(1)
                    });
                    let available = end.saturating_add(release_source_frames);
                    if slot == free_at.len() {
                        free_at.push(available);
                    } else {
                        free_at[slot] = available;
                    }
                    let output_frame = |frame| {
                        clip.stretch.map_or(frame, |stretch| {
                            (u128::from(frame) * u128::from(stretch.output_frames))
                                .div_ceil(u128::from(stretch.source_frames))
                                .min(u128::from(u64::MAX)) as u64
                        })
                    };
                    let duration_frames =
                        output_frame(end).saturating_sub(output_frame(start)).max(1);
                    events.push(NoteEvent {
                        duration_frames,
                        pitch_index,
                        slot,
                        frame: start,
                        note: note.id,
                        on: Some((note.pitch, note.velocity)),
                    });
                    events.push(NoteEvent {
                        duration_frames: 0,
                        pitch_index,
                        slot,
                        frame: end,
                        note: note.id,
                        on: None,
                    });
                }
                voice_count = voice_count.max(free_at.len());
                // Ends precede starts at a shared boundary; ID breaks simultaneous-on ties.
                events.sort_by_key(|e| (e.frame, e.on.is_some(), e.note));
                let mut state = super::modulation::NoteState::default();
                let mut slot_starts = vec![Vec::new(); free_at.len()];
                let note_states = events
                    .iter()
                    .enumerate()
                    .map(|(index, e)| {
                        let frame = output_frame(clip, e.frame);
                        state.record(e.on.is_some(), frame, e.duration_frames);
                        if e.on.is_some() {
                            slot_starts[e.slot].push(index);
                        }
                        state
                    })
                    .collect();
                Some(PatternClip {
                    clip: clip.clone(),
                    pitches,
                    events,
                    note_states,
                    slot_starts,
                })
            })
            .collect();
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for clip in &track.clips {
            let mut shape = clip.clone();
            shape.asset_id = Id::nil();
            format!("{shape:?}").hash(&mut hash);
        }
        instrument.version.hash(&mut hash);
        instrument.layers.len().hash(&mut hash);
        // Stable layer IDs preserve independent controls and identify topology changes.
        for layer in &layers {
            layer.id.hash(&mut hash);
            if let Some(s) = &layer.sampler {
                s.id.hash(&mut hash);
                s.root_note.hash(&mut hash);
                s.granulizer.is_some().hash(&mut hash);
                format!("{:?}", s.input).hash(&mut hash);
                s.start_frame.hash(&mut hash);
                s.end_frame.hash(&mut hash);
                s.output_frames(project.sample_rate_hz, project.tempo_bpm)
                    .hash(&mut hash);
                audio
                    .get(&s.id)
                    .map(|a| std::sync::Arc::as_ptr(&a.samples) as usize)
                    .hash(&mut hash);
            }
            let mut sources: Vec<_> = layer.oscillators.iter().collect();
            sources.sort_by_key(|s| s.id);
            for settings in sources {
                settings.id.hash(&mut hash);
                format!("{:?}", settings.waveform).hash(&mut hash);
            }
        }
        project.tempo_bpm.to_bits().hash(&mut hash);
        project.sample_rate_hz.hash(&mut hash);
        for clip in &track.clips {
            clip.muted.hash(&mut hash);
            if let Some(asset) = project.assets.iter().find(|a| a.id == clip.asset_id)
                && let Some(pattern) = &asset.midi_pattern
            {
                pattern.length_ticks.hash(&mut hash);
                let mut notes: Vec<_> = pattern.notes.iter().collect();
                notes.sort_by_key(|n| (n.start_tick, n.id));
                for n in notes {
                    (n.id, n.start_tick, n.length_ticks, n.velocity).hash(&mut hash);
                }
            }
        }
        let lfo_notes = if instrument.main.lfos.is_empty()
            && layers.iter().all(|layer| layer.lfos.is_empty())
        {
            super::lfo::NoteSchedule::new(Vec::new())
        } else {
            super::lfo::NoteSchedule::new(
                clips
                    .iter()
                    .map(|c| {
                        super::lfo::NoteClip::new(
                            c.clip.clone(),
                            c.events.iter().map(|e| (e.frame, e.on.is_some())),
                        )
                    })
                    .collect(),
            )
        };
        let local_nodes: Vec<_> = layers
            .iter()
            .flat_map(|l| {
                l.envelopes
                    .iter()
                    .filter(|e| e.mode == EnvelopeMode::PerNote)
                    .map(move |e| {
                        (
                            ModulationSourceId {
                                layer: l.id,
                                unit: e.id,
                            },
                            e.modulations.as_slice(),
                        )
                    })
            })
            .collect();
        let local_order = daw_unit_core::modulation_dependency_order(&local_nodes)
            .expect("validated local envelope graph");
        let effect_latency = super::layer_effects::latency(instrument, project.sample_rate_hz);
        let parallel_latency =
            super::layer_effects::parallel_latency(instrument, project.sample_rate_hz);
        Some(Self {
            layer_effects: layers
                .iter()
                .map(|l| {
                    super::layer_effects::LayerEffects::new(
                        l.id,
                        &l.effects,
                        layer_gains(l, any_solo),
                        project.sample_rate_hz,
                        project.buffer_size_frames,
                        parallel_latency,
                    )
                })
                .chain(std::iter::once(super::layer_effects::LayerEffects::new(
                    instrument.main.id,
                    &instrument.main.effects,
                    [1.0; 2],
                    project.sample_rate_hz,
                    project.buffer_size_frames,
                    super::latency::chain_frames(&instrument.main.effects, project.sample_rate_hz),
                )))
                .collect(),
            layer_samples: vec![[0.0; 2]; layers.len()],
            effect_latency,
            envelope_history: layers.iter().any(|l| {
                l.sampler.as_ref().is_some_and(|s| {
                    s.granulizer.is_some()
                        && audio.get(&s.id).is_some_and(|a| !a.samples.is_empty())
                })
            }) || instrument.envelope_destinations().any(|(_, e)| {
                e.modulations
                    .iter()
                    .any(|r| r.source.is_some() && r.enabled && r.amount != 0.0)
            }),
            parallel_latency,
            has_bus_effects: layers.iter().any(|l| !l.effects.is_empty())
                || !instrument.main.effects.is_empty(),
            layer_index: layers.iter().enumerate().map(|(i, l)| (l.id, i)).collect(),
            oscillator_index: layers
                .iter()
                .flat_map(|l| {
                    l.oscillators
                        .iter()
                        .enumerate()
                        .map(move |(i, s)| ((l.id, s.id), i))
                })
                .collect(),
            main_gain: bus_gains(instrument.main.gain_db, instrument.main.pan),
            main_target: bus_gains(instrument.main.gain_db, instrument.main.pan),
            main_remaining: 0,
            lfo_notes,
            tempo_bpm: project.tempo_bpm,
            fingerprint: hash.finish(),
            held_sample: [0.0; 2],
            voices: (0..voice_count)
                .map(|_| Voice {
                    active_last_sample: false,
                    sleeping: true,
                    layers: layers
                        .iter()
                        .zip(&banks)
                        .map(|(layer, bank)| LayerVoice {
                            mixed_output: [0.0; 2],
                            mixed_transition_from: [0.0; 2],
                            previous: [0.0; 2],
                            transition_from: [0.0; 2],
                            output: [0.0; 2],
                            sampler: layer.sampler.as_ref().map(|s| {
                                daw_sampler::Sampler::with_bank(
                                    ProcessingContext {
                                        sample_rate_hz: project.sample_rate_hz,
                                        max_block_frames: project.buffer_size_frames,
                                    },
                                    s,
                                    bank.as_ref().unwrap().clone(),
                                )
                            }),
                            sampler_routes: layer
                                .sampler
                                .iter()
                                .flat_map(|s| &s.modulations)
                                .map(|r| Route::new(*r, project.sample_rate_hz))
                                .collect(),
                            one_shot_target: layer
                                .sampler
                                .as_ref()
                                .is_some_and(|s| s.trigger == daw_core::SamplerTrigger::OneShot),
                            one_shot: layer
                                .sampler
                                .as_ref()
                                .is_some_and(|s| s.trigger == daw_core::SamplerTrigger::OneShot),
                            oscillator_level: daw_oscillator::OscillatorOutput::new(
                                layer.oscillator_level_db,
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            )
                            .with_pitch(layer.oscillator_pitch_semitones),
                            oscillator_level_routes: layer
                                .oscillator_modulations
                                .iter()
                                .filter(|r| r.target.is_none())
                                .map(|r| Route::new(*r, project.sample_rate_hz))
                                .collect(),
                            sources: layer
                                .oscillators
                                .iter()
                                .map(|settings| OscillatorVoice {
                                    id: settings.id,
                                    routes: daw_unit_core::prepare_target_routes(
                                        &layer.oscillator_modulations,
                                        settings.id,
                                        project.sample_rate_hz,
                                    ),
                                    source: Oscillator::from_settings(
                                        settings,
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
                            amplitude: daw_amplitude::Amplitude::new(
                                &layer.amplitude,
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            ),
                            routes: layer
                                .amplitude
                                .modulations
                                .iter()
                                .map(|r| Route::new(*r, project.sample_rate_hz))
                                .collect(),
                        })
                        .collect(),
                    control_order: local_order.clone(),
                    controls: layers
                        .iter()
                        .flat_map(|l| {
                            l.envelopes
                                .iter()
                                .filter(|e| e.mode == EnvelopeMode::PerNote)
                                .map(|e| {
                                    Control::new(
                                        ModulationSourceId {
                                            layer: l.id,
                                            unit: e.id,
                                        },
                                        e,
                                        ProcessingContext {
                                            sample_rate_hz: project.sample_rate_hz,
                                            max_block_frames: project.buffer_size_frames,
                                        },
                                    )
                                })
                        })
                        .collect(),
                    values: layers
                        .iter()
                        .flat_map(|l| {
                            l.envelopes
                                .iter()
                                .filter(|e| e.mode == EnvelopeMode::PerNote)
                                .map(|e| ModulationValue {
                                    source: ModulationSourceId {
                                        layer: l.id,
                                        unit: e.id,
                                    },
                                    value: 0.0,
                                    weight: 0.0,
                                    range: daw_unit_core::ModulationRange::Unipolar,
                                    behavior: daw_unit_core::ModulationBehavior::Contour,
                                    release_age: None,
                                })
                        })
                        .collect(),
                    local_sources: daw_unit_core::ModulationSourceIndex::new(
                        layers.iter().flat_map(|l| {
                            l.envelopes
                                .iter()
                                .filter(|e| e.mode == EnvelopeMode::PerNote)
                                .map(|e| ModulationSourceId {
                                    layer: l.id,
                                    unit: e.id,
                                })
                        }),
                    ),
                    current: None,
                    pitch_index: None,
                    velocity: 0.0,
                    pitch: 69,
                    reconstructed_at: 0,
                    previous: [0.0; 2],
                    transition_from: [0.0; 2],
                    transition: 0,
                    fade_frames,
                    held_frames: 0,
                    note_duration_frames: None,
                    release_frames: 0,
                })
                .collect(),
            clip_ranges: super::schedule::Ranges::new(
                clips.iter().map(|p| (p.clip.start_frame, p.clip.end())),
            ),
            clips,
            sample_rate_hz: project.sample_rate_hz,
            last_clip: None,
            last_source: None,
            next_event: 0,
            shared: SharedControls::new(
                layers
                    .iter()
                    .flat_map(|l| {
                        l.envelopes
                            .iter()
                            .filter(|e| e.mode != EnvelopeMode::PerNote)
                            .map(|e| {
                                Control::new(
                                    ModulationSourceId {
                                        layer: l.id,
                                        unit: e.id,
                                    },
                                    e,
                                    ProcessingContext {
                                        sample_rate_hz: project.sample_rate_hz,
                                        max_block_frames: project.buffer_size_frames,
                                    },
                                )
                            })
                    })
                    .chain(instrument.main.envelopes.iter().map(|e| {
                        Control::new(
                            ModulationSourceId {
                                layer: instrument.main.id,
                                unit: e.id,
                            },
                            e,
                            ProcessingContext {
                                sample_rate_hz: project.sample_rate_hz,
                                max_block_frames: project.buffer_size_frames,
                            },
                        )
                    }))
                    .collect(),
            )
            .with_providers(
                layers
                    .iter()
                    .flat_map(|layer| {
                        layer.lfos.iter().map(|lfo| {
                            ProviderControl::lfo(
                                layer.id,
                                lfo,
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            )
                        })
                    })
                    .chain(instrument.main.lfos.iter().map(|lfo| {
                        ProviderControl::lfo(
                            instrument.main.id,
                            lfo,
                            ProcessingContext {
                                sample_rate_hz: project.sample_rate_hz,
                                max_block_frames: project.buffer_size_frames,
                            },
                        )
                    }))
                    .chain(instrument.macro_modules().flat_map(|(layer, module)| {
                        module.controls.iter().map(move |control| {
                            ProviderControl::macro_control(
                                layer,
                                module,
                                control,
                                ProcessingContext {
                                    sample_rate_hz: project.sample_rate_hz,
                                    max_block_frames: project.buffer_size_frames,
                                },
                            )
                        })
                    }))
                    .collect(),
            ),
        })
    }
    pub fn bind_live_pitches(&mut self, track: Id, inputs: &super::LiveMidiPitches) {
        for clip in &mut self.clips {
            for pitch in &mut clip.pitches {
                pitch.input = inputs.bind(track, clip.clip.id, pitch.note);
            }
        }
    }
    pub fn compatible(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
            && self.effect_latency == other.effect_latency
            && self.parallel_latency == other.parallel_latency
            && self
                .layer_effects
                .iter()
                .zip(&other.layer_effects)
                .all(|(a, b)| {
                    super::latency::chain_frames(&a.settings, self.sample_rate_hz)
                        == super::latency::chain_frames(&b.settings, self.sample_rate_hz)
                })
            && self.control_modes().all(|a| {
                other
                    .control_modes()
                    .all(|b| a.source != b.source || a.mode == b.mode)
            })
    }
    fn control_modes(&self) -> impl Iterator<Item = &Control> {
        self.shared
            .controls
            .iter()
            .chain(self.voices.first().into_iter().flat_map(|v| &v.controls))
    }
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    /// Retarget compatible voices before swapping them into the new renderer.
    /// All storage stays owned by the two prepared renderers; no DSP state is dropped here.
    pub fn retarget_controls(&mut self, prepared: &mut Self) {
        let fade_frames = (self.sample_rate_hz / 200).max(1);
        // Pitch-only edits retain the schedule shape and voice slots. Transfer the newly
        // prepared pitches/mailboxes while leaving envelopes, clocks and source phase alive.
        std::mem::swap(&mut self.clips, &mut prepared.clips);
        if let Some(clip) = self
            .clips
            .iter()
            .find(|c| Some(c.clip.id) == self.last_clip)
        {
            for voice in &mut self.voices {
                if let Some(pitch) = voice.pitch_index.and_then(|i| clip.pitches.get(i)) {
                    let (pitch, preview) = pitch.input.read(pitch.base);
                    voice.retune_pitch(clip_pitch(pitch, clip.clip.transpose), preview);
                }
            }
        }
        for next in &mut prepared.layer_effects {
            for old in &mut self.layer_effects {
                next.chain.inherit_chain(&mut old.chain, &next.settings);
            }
        }
        let idle = self.voices.iter().all(|v| !v.active_last_sample);
        for (old, next) in self
            .layer_effects
            .iter_mut()
            .zip(&mut prepared.layer_effects)
        {
            old.retarget(next, fade_frames, idle);
        }
        self.has_bus_effects = prepared.has_bus_effects;
        self.envelope_history = prepared.envelope_history;
        if self.main_target != prepared.main_target {
            self.main_target = prepared.main_target;
            self.main_remaining = fade_frames;
        }
        self.shared.retarget(&mut prepared.shared);
        std::mem::swap(&mut self.layer_index, &mut prepared.layer_index);
        std::mem::swap(&mut self.oscillator_index, &mut prepared.oscillator_index);
        // Adding the first LFO also installs its prepared note-origin schedule.
        std::mem::swap(&mut self.lfo_notes, &mut prepared.lfo_notes);
        for (voice, next) in self.voices.iter_mut().zip(&mut prepared.voices) {
            // New controls can extend a live release; re-evaluate activity once after retarget.
            voice.sleeping = false;
            for n in &mut next.controls {
                if let Some(e) = voice.controls.iter_mut().find(|e| e.source == n.source) {
                    e.retarget(n);
                    std::mem::swap(e, n);
                } else if voice.held_frames > 0 {
                    n.envelope.note_on_with_duration(voice.note_duration_frames);
                    n.envelope.advance(voice.held_frames);
                    if voice.current.is_none() {
                        n.envelope.note_off();
                        n.envelope.advance(voice.release_frames);
                    }
                }
            }
            std::mem::swap(&mut voice.controls, &mut next.controls);
            std::mem::swap(&mut voice.values, &mut next.values);
            std::mem::swap(&mut voice.local_sources, &mut next.local_sources);
            std::mem::swap(&mut voice.control_order, &mut next.control_order);
            let idle = voice.transition == 0
                && voice.layers.iter().all(LayerVoice::is_idle)
                && voice.controls.iter().all(|e| e.envelope.is_idle())
                && !self
                    .shared
                    .values
                    .iter()
                    .any(|v| v.owns_release(Some(voice.release_frames)));
            for (layer, next_layer) in voice.layers.iter_mut().zip(&mut next.layers) {
                daw_unit_core::retarget_modulation_routes(
                    &mut layer.routes,
                    &mut next_layer.routes,
                );
                daw_unit_core::retarget_modulation_routes(
                    &mut layer.sampler_routes,
                    &mut next_layer.sampler_routes,
                );
                layer.amplitude.retarget(&next_layer.amplitude);
                layer
                    .oscillator_level
                    .retarget(&next_layer.oscillator_level);
                daw_unit_core::retarget_modulation_routes(
                    &mut layer.oscillator_level_routes,
                    &mut next_layer.oscillator_level_routes,
                );
                if idle {
                    layer.oscillator_level.reset();
                }
                // Trigger edits apply to new notes; held/released notes keep their gate behavior.
                layer.one_shot_target = next_layer.one_shot_target;
                if let (Some(s), Some(next)) = (&mut layer.sampler, &mut next_layer.sampler) {
                    s.retarget(next);
                    s.inherit_bank(next);
                }
                if idle {
                    layer.amplitude.reset();
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
                for target in &mut next_layer.sources {
                    if let Some(source) = layer.sources.iter_mut().find(|s| s.id == target.id) {
                        daw_unit_core::retarget_modulation_routes(
                            &mut source.routes,
                            &mut target.routes,
                        );
                        if source.transpose != target.transpose {
                            source.transpose = target.transpose;
                            source.frequency_target = note_frequency(voice.pitch, source.transpose);
                            source.frequency_remaining = fade_frames;
                        }
                        if source.gain_target != target.gain_target {
                            source.gain_target = target.gain_target;
                            source.gain_remaining = fade_frames;
                        }
                        std::mem::swap(source, target);
                    } else {
                        target.frequency = note_frequency(voice.pitch, target.transpose);
                        target.frequency_target = target.frequency;
                        target.source.set_frequency(target.frequency);
                        if !idle {
                            target.gain = 0.0;
                            target.gain_remaining = fade_frames;
                        }
                    }
                }
                std::mem::swap(&mut layer.sources, &mut next_layer.sources);
            }
        }
    }
    fn voice_sample(&mut self, frame: u64, loop_region: Option<(u64, u64)>, fade: f32) -> [f32; 2] {
        let mut sum = [0.0; 2];
        self.layer_samples.fill([0.0; 2]);
        for bus in &mut self.layer_effects {
            bus.meter_sample = [0.0; 2];
        }
        self.shared
            .tick(self.lfo_notes.context(frame, self.tempo_bpm));
        let shared_release = self
            .shared
            .values
            .iter()
            .find(|v| v.owns_release(v.release_age))
            .and_then(|v| v.release_age);
        for voice in &mut self.voices {
            let sample = voice.sample(&self.shared.values, &self.shared.index, shared_release);
            for ((sum, bus), layer) in self
                .layer_samples
                .iter_mut()
                .zip(&mut self.layer_effects)
                .zip(&voice.layers)
            {
                for (ch, sum) in sum.iter_mut().enumerate() {
                    *sum += layer.output[ch];
                    bus.meter_sample[ch] += layer.mixed_output[ch];
                }
            }
            for ch in 0..2 {
                sum[ch] += sample[ch];
            }
        }
        if self.has_bus_effects {
            sum = [0.0; 2];
            for (bus, input) in self.layer_effects.iter_mut().zip(&self.layer_samples) {
                let (sample, mono) = bus.chain.sample(
                    input.map(|v| v * fade),
                    Some(false),
                    frame,
                    self.tempo_bpm,
                    loop_region,
                    (&self.shared.values, Some(&self.shared.index)),
                );
                let (sample, _) = bus.delay.tick((sample, mono));
                let sample = bus.mix(sample);
                bus.meter_sample = sample;
                for ch in 0..2 {
                    sum[ch] += sample[ch];
                }
            }
        } else {
            sum = sum.map(|v| v * fade);
            for bus in self.layer_effects.iter_mut().take(self.layer_samples.len()) {
                bus.meter_sample = bus.meter_sample.map(|v| v * fade);
            }
        }
        let main = self.layer_effects.last_mut().expect("Main bus is prepared");
        main.chain
            .sample(
                sum,
                Some(false),
                super::latency::delayed_frame(frame, self.parallel_latency, loop_region),
                self.tempo_bpm,
                loop_region,
                (&self.shared.values, Some(&self.shared.index)),
            )
            .0
    }
    pub fn needs_envelope_history(&self) -> bool {
        self.envelope_history
    }
    pub fn effect_latency_compatible(&self, old: &Self) -> bool {
        self.effect_latency == old.effect_latency
            && self.parallel_latency == old.parallel_latency
            && (self.effect_latency == 0
                || (self.layer_index.len() == old.layer_index.len()
                    && self.layer_index.iter().all(|(id, &index)| {
                        old.layer_index.get(id).is_some_and(|&other| {
                            super::latency::chain_frames(
                                &self.layer_effects[index].settings,
                                self.sample_rate_hz,
                            ) == super::latency::chain_frames(
                                &old.layer_effects[other].settings,
                                old.sample_rate_hz,
                            )
                        })
                    })))
    }
    pub fn effect_latency(&self) -> u64 {
        self.effect_latency
    }
    pub fn inherit_track_effects(&mut self, previous: &mut super::effects::EffectChain) {
        for bus in &mut self.layer_effects {
            bus.chain.inherit_chain(previous, &bus.settings);
        }
    }
    pub fn inherit_into_track_effects(
        &mut self,
        next: &mut super::effects::EffectChain,
        settings: &[daw_core::EffectInstance],
    ) {
        for bus in &mut self.layer_effects {
            next.inherit_chain(&mut bus.chain, settings);
        }
    }
    pub fn envelope_snapshot(&self, id: ModulationSourceId) -> Option<super::ModulationSnapshot> {
        let mut range = [f32::NAN; super::MODULATION_SNAPSHOT_SIZE];
        let mut found = false;
        let readings = self.shared.envelope_values(id).into_iter().chain(
            self.voices
                .iter()
                .filter(|v| v.active_last_sample)
                .filter_map(|v| {
                    v.controls
                        .iter()
                        .find(|e| e.source == id)
                        .map(|e| e.envelope.modulation_values())
                }),
        );
        for values in readings {
            found = true;
            for (index, value) in values.into_iter().enumerate() {
                for edge in 0..2 {
                    let slot = index * 2 + edge;
                    if !range[slot].is_finite() {
                        range[slot] = value;
                    } else if edge == 0 {
                        range[slot] = range[slot].min(value);
                    } else {
                        range[slot] = range[slot].max(value);
                    }
                }
            }
        }
        found.then_some(range)
    }
    pub fn effect_band_snapshot(&self, id: Id, band: Id) -> Option<super::ModulationSnapshot> {
        self.layer_effects
            .iter()
            .find_map(|bus| bus.chain.band_modulation_values(id, band))
    }
    pub fn effect_snapshot(&self, id: Id) -> Option<super::ModulationSnapshot> {
        super::layer_effects::find(&self.layer_effects, id)
    }
    pub fn set_effect_meter(&mut self, id: Id, meter: &std::sync::Arc<daw_unit_core::PeakMeter>) {
        for bus in &mut self.layer_effects {
            bus.chain.set_meter(id, meter);
        }
    }
    pub fn bind_exciter_solo(
        &mut self,
        effect: Id,
        band: Id,
        solo: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        for bus in &mut self.layer_effects {
            bus.chain.bind_exciter_solo(effect, band, solo);
        }
    }
    pub fn set_individual_band_meter(
        &mut self,
        effect: Id,
        band: Id,
        meter: &std::sync::Arc<daw_unit_core::BandMeter<3>>,
    ) {
        for bus in &mut self.layer_effects {
            bus.chain.set_band_level_meter(effect, band, meter);
        }
    }
    pub fn set_effect_output_meter(
        &mut self,
        id: Id,
        meter: &std::sync::Arc<daw_unit_core::BandMeter<3>>,
    ) {
        for bus in &mut self.layer_effects {
            bus.chain.set_output_meter(id, meter);
        }
    }
    pub fn set_effect_band_meter(
        &mut self,
        id: Id,
        meter: &std::sync::Arc<daw_unit_core::BandMeter<3>>,
    ) {
        for bus in &mut self.layer_effects {
            bus.chain.set_band_meter(id, meter);
        }
    }
    pub fn provider_values(&self, id: daw_unit_core::ModulationSourceId) -> Option<[f32; 2]> {
        self.shared.provider_values(id)
    }
    pub fn modulation_sources(&self) -> &daw_unit_core::ModulationSourceIndex {
        &self.shared.index
    }
    pub fn modulation_values(&self) -> &[ModulationValue] {
        &self.shared.values
    }
    pub fn held_sample(&self) -> [f32; 2] {
        self.held_sample
    }
    pub fn reset(&mut self) {
        self.shared.reset();
        for bus in &mut self.layer_effects {
            bus.reset();
        }
        self.main_gain = self.main_target;
        self.main_remaining = 0;
        self.held_sample = [0.0; 2];
        for voice in &mut self.voices {
            voice.reset();
        }
        self.last_clip = None;
        self.last_source = None;
    }
    pub fn begin_note_audition(&mut self, pitch: u8) {
        // The isolated preview keeps prepared voices/banks but bypasses clip scheduling.
        self.clips.clear();
        self.clip_ranges = super::schedule::Ranges::new(std::iter::empty());
        self.last_clip = None;
        self.last_source = None;
        self.shared.event(true, None);
        if let Some(voice) = self.voices.first_mut() {
            voice.note_on_with_duration(Id::new_v4(), pitch, 100, None);
        }
    }

    pub fn end_notes(&mut self) {
        self.shared.end();
        for voice in &mut self.voices {
            voice.end_note();
        }
        self.last_clip = None;
        self.last_source = None;
    }
    /// Highest active voice gain. Voice summing is not part of the parameter value.
    pub fn amplitude_gain(&self, layer: Id) -> f32 {
        self.voices
            .iter()
            .filter(|voice| voice.active_last_sample)
            .filter_map(|voice| {
                self.layer_index
                    .get(&layer)
                    .and_then(|&i| voice.layers.get(i))
            })
            .map(|l| l.amplitude.effective_gain())
            .fold(0.0, f32::max)
    }
    /// Active-voice output Level dB and common Pitch semitone extrema.
    pub fn oscillator_level_values(&self, layer: Id) -> [f32; 4] {
        let mut range = [
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ];
        for voice in self.voices.iter().filter(|v| v.active_last_sample) {
            if let Some(l) = self
                .layer_index
                .get(&layer)
                .and_then(|&i| voice.layers.get(i))
            {
                let gain = l.oscillator_level.effective_gain();
                let db = if gain > 0.0 {
                    20.0 * gain.log10()
                } else {
                    daw_core::SILENT_GAIN_DB
                };
                range[0] = range[0].min(db);
                range[1] = range[1].max(db);
                let pitch = l.oscillator_level.effective_pitch();
                range[2] = range[2].min(pitch);
                range[3] = range[3].max(pitch);
            }
        }
        range.map(|v| if v.is_finite() { v } else { f32::NAN })
    }
    /// Active-voice transpose and gain (dB) extrema. Exclude MIDI pitch, velocity and layer mixing.
    pub fn oscillator_values(&self, layer: Id, oscillator: Id) -> [f32; 4] {
        let mut range = [
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ];
        for voice in self.voices.iter().filter(|v| v.active_last_sample) {
            if let Some(source) = self
                .layer_index
                .get(&layer)
                .and_then(|&i| voice.layers.get(i))
                .and_then(|l| {
                    self.oscillator_index
                        .get(&(layer, oscillator))
                        .and_then(|&i| l.sources.get(i))
                })
            {
                let pitch = 12.0 * (source.frequency / note_frequency(voice.pitch, 0)).log2()
                    + source.source.pitch_offset();
                range[0] = range[0].min(pitch);
                range[1] = range[1].max(pitch);
                let gain = source.source.effective_gain();
                let db = if gain > 0.0 {
                    20.0 * gain.log10()
                } else {
                    daw_core::SILENT_GAIN_DB
                };
                range[2] = range[2].min(db);
                range[3] = range[3].max(db);
            }
        }
        range.map(|v| if v.is_finite() { v } else { f32::NAN })
    }
    /// Active Sampler gain extrema in dB. Exclude velocity, boundary fades and Amplitude.
    pub fn sampler_gain(&self, layer: Id) -> [f32; 2] {
        let mut range = [f32::INFINITY, f32::NEG_INFINITY];
        for sampler in self
            .voices
            .iter()
            .filter(|v| v.active_last_sample)
            .filter_map(|v| self.layer_index.get(&layer).and_then(|&i| v.layers.get(i)))
            .filter_map(|l| l.sampler.as_ref())
            .filter(|s| s.active())
        {
            let gain = sampler.effective_gain();
            let db = if gain > 0.0 {
                20.0 * gain.log10()
            } else {
                daw_core::SILENT_GAIN_DB
            };
            range[0] = range[0].min(db);
            range[1] = range[1].max(db);
        }
        range.map(|v| if v.is_finite() { v } else { f32::NAN })
    }
    pub fn granular_snapshot(&self, layer: Id) -> Option<super::ModulationSnapshot> {
        let mut result = [f32::NAN; super::MODULATION_SNAPSHOT_SIZE];
        let sampler = self
            .voices
            .iter()
            .filter(|v| v.active_last_sample)
            .filter_map(|v| self.layer_index.get(&layer).and_then(|&i| v.layers.get(i)))
            .filter_map(|l| l.sampler.as_ref())
            .find(|s| s.active() && s.granular_snapshot().is_some())?;
        let (values, positions) = sampler.granular_snapshot()?;
        result[0] = sampler.position().map_or(f32::NAN, |p| p as f32);
        result[1..3].copy_from_slice(&self.sampler_gain(layer));
        result[3..11].copy_from_slice(&values);
        result[11..27].copy_from_slice(&positions);
        Some(result)
    }
    pub fn sampler_position(&self, layer: Id) -> f32 {
        self.voices
            .iter()
            .filter(|v| v.active_last_sample)
            .filter_map(|v| self.layer_index.get(&layer).and_then(|&i| v.layers.get(i)))
            .filter_map(|l| l.sampler.as_ref())
            .filter_map(|s| s.position())
            .max()
            .map_or(f32::NAN, |p| p as f32)
    }
    #[cfg(test)]
    pub fn sample(&mut self, frame: u64) -> [f32; 2] {
        self.sample_in_region(frame, None)
    }
    pub fn sample_in_region(&mut self, frame: u64, loop_region: Option<(u64, u64)>) -> [f32; 2] {
        let mut clip_gain = 1.0;
        let sample = if let Some(index) = self.clip_ranges.at(frame) {
            let prepared = &self.clips[index];
            let clip = &prepared.clip;
            clip_gain = linear_gain(clip.gain_db);
            let position = clip.source_frame(frame - clip.start_frame);
            let source = clip
                .stretch
                .map_or(position, |s| s.original_frame(position));
            let restarted = self.last_clip != Some(clip.id)
                || self.last_source.is_none_or(|last| source < last);
            if restarted {
                self.next_event = prepared.events.partition_point(|e| e.frame <= source);
                let state = self
                    .next_event
                    .checked_sub(1)
                    .map_or_else(Default::default, |i| prepared.note_states[i]);
                self.shared.reconstruct(state, position);
                for (slot, voice) in self.voices.iter_mut().enumerate() {
                    voice.reset();
                    let event = prepared.slot_starts.get(slot).and_then(|starts| {
                        let next = starts.partition_point(|&i| prepared.events[i].frame <= source);
                        next.checked_sub(1).map(|i| &prepared.events[starts[i]])
                    });
                    if let Some(event) = event {
                        let (pitch, velocity) = event.on.unwrap();
                        let pitch = clip_pitch(pitch, clip.transpose);
                        let start = output_frame(clip, event.frame);
                        let end = start.saturating_add(event.duration_frames);
                        voice.note_on(event.note, pitch, velocity, event.duration_frames);
                        voice.pitch_index = Some(event.pitch_index);
                        voice.advance_envelopes(position.min(end).saturating_sub(start));
                        voice.reconstructed_at = start;
                        if position >= end {
                            voice.end_note();
                            voice.advance_envelopes(position - end);
                            voice.reconstructed_at = end;
                        }
                    } else {
                        voice.advance_envelopes(position);
                    }
                }
            } else {
                while let Some(event) = prepared
                    .events
                    .get(self.next_event)
                    .filter(|e| e.frame <= source)
                {
                    self.shared
                        .event(event.on.is_some(), event.on.map(|_| event.duration_frames));
                    let voice = &mut self.voices[event.slot];
                    if let Some((pitch, velocity)) = event.on {
                        voice.note_on(
                            event.note,
                            clip_pitch(pitch, clip.transpose),
                            velocity,
                            event.duration_frames,
                        );
                        voice.pitch_index = Some(event.pitch_index);
                    } else if voice.current == Some(event.note) {
                        voice.end_note();
                    }
                    self.next_event += 1;
                }
            }
            for voice in &mut self.voices {
                if (voice.current.is_some() || voice.active_last_sample)
                    && let Some(pitch) = voice.pitch_index.and_then(|i| prepared.pitches.get(i))
                {
                    let (pitch, preview) = pitch.input.read(pitch.base);
                    voice.retune_pitch(clip_pitch(pitch, clip.transpose), preview);
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
            self.voice_sample(frame, loop_region, fade)
        } else {
            if self.last_clip.take().is_some() {
                self.shared.reset_notes();
                // Clip fades have already reached zero; do not resurrect a release tail in a gap.
                for voice in &mut self.voices {
                    voice.reset();
                }
            }
            self.last_source = None;
            self.voice_sample(frame, loop_region, 1.0)
        };
        if self.main_remaining > 0 {
            for ch in 0..2 {
                self.main_gain[ch] +=
                    (self.main_target[ch] - self.main_gain[ch]) / self.main_remaining as f32;
            }
            self.main_remaining -= 1;
        }
        let sample = std::array::from_fn(|ch| sample[ch] * self.main_gain[ch] * clip_gain);
        // Read-only pre-track monitoring: regular buses include clip gain; Output
        // includes its own gain/pan and clip gain. No monitor changes the audio sum.
        let regular = self.layer_samples.len();
        for (index, bus) in self.layer_effects.iter_mut().enumerate() {
            let reading = if index == regular {
                sample
            } else {
                bus.meter_sample.map(|v| v * clip_gain)
            };
            for (peak, value) in bus.peak.iter_mut().zip(reading) {
                *peak = peak.max(value.abs());
            }
        }
        self.held_sample = sample;
        sample
    }
    pub fn reset_layer_peaks(&mut self) {
        for bus in &mut self.layer_effects {
            bus.peak = [0.0; 2];
        }
    }
    pub fn layer_peaks(&self) -> impl Iterator<Item = (Id, [f32; 2])> + '_ {
        self.layer_effects.iter().map(|bus| (bus.id, bus.peak))
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
    fn disabled_midi_clips_remove_note_events_and_invalidate_history() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
        let clip = project.add_midi_pattern(track, 0).unwrap();
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 1920,
                        pitch: 60,
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        let active = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        assert_eq!(active.clips.len(), 1);
        project
            .edit(Edit::SetClipMuted {
                clip_id: clip,
                muted: true,
            })
            .unwrap();
        let mut disabled = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        assert!(disabled.clips.is_empty());
        assert_ne!(active.fingerprint(), disabled.fingerprint());
        for frame in 0..2000 {
            assert_eq!(disabled.sample(frame), [0.0; 2]);
        }
    }

    #[test]
    fn clip_transpose_shifts_playback_pitch_and_gain_scales_output() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
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
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        project.tracks[0].clips[0].length_frames = 96_000;
        project.tracks[0].clips[0].transpose = 12;
        let mut runtime = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        runtime.sample(2_000);
        assert_eq!(runtime.voices[0].pitch, 72);
        let asset = project.tracks[0].clips[0].asset_id;
        assert_eq!(
            project
                .assets
                .iter()
                .find(|item| item.id == asset)
                .unwrap()
                .midi_pattern
                .as_ref()
                .unwrap()
                .notes[0]
                .pitch,
            60
        );
        project.tracks[0].clips[0].transpose = 0;
        let mut unity = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        let mut unity_sample = [0.0; 2];
        for frame in 0..=2_000 {
            unity_sample = unity.sample(frame);
        }
        project.tracks[0].clips[0].gain_db = 6.0;
        let mut louder = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        let mut louder_sample = [0.0; 2];
        for frame in 0..=2_000 {
            louder_sample = louder.sample(frame);
        }
        let ratio = 10.0_f32.powf(6.0 / 20.0);
        assert!(unity_sample.iter().any(|sample| sample.abs() > 1e-4));
        for ch in 0..2 {
            if unity_sample[ch].abs() > 1e-4 {
                assert!((louder_sample[ch] / unity_sample[ch] - ratio).abs() < 1e-3);
            }
        }
        project.tracks[0].clips[0].gain_db = daw_core::SILENT_GAIN_DB;
        let mut silent = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        for frame in 0..=2_000 {
            assert_eq!(silent.sample(frame), [0.0, 0.0]);
        }
        project.tracks[0].clips[0].gain_db = 0.0;
        project.tracks[0].clips[0].transpose = 48;
        project
            .assets
            .iter_mut()
            .find(|item| item.id == asset)
            .unwrap()
            .midi_pattern
            .as_mut()
            .unwrap()
            .notes[0]
            .pitch = 100;
        let mut high = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        high.sample(2_000);
        assert_eq!(high.voices[0].pitch, 127);
        project.tracks[0].clips[0].transpose = -48;
        project
            .assets
            .iter_mut()
            .find(|item| item.id == asset)
            .unwrap()
            .midi_pattern
            .as_mut()
            .unwrap()
            .notes[0]
            .pitch = 10;
        let mut low = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
        low.sample(2_000);
        assert_eq!(low.voices[0].pitch, 0);
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
            instrument.layers[0].envelopes[0].settings.attack.time_ms = 200.0;
            instrument.layers[0].envelopes[0].settings.release.time_ms = 500.0;
            let mut layer = daw_core::InstrumentLayerSettings::default();
            layer.oscillators.push(Default::default());
            layer.install_amplitude_envelope();
            layer.envelopes[0].settings.attack.time_ms = 350.0;
            layer.envelopes[0].settings.release.time_ms = 800.0;
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
                for (sought, played) in sought.controls.iter().zip(&played.controls) {
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
            assert!(voice.controls[0].envelope.level() > 0.0);
        }
    }
}

#[cfg(test)]
mod note_timing_tests {
    use super::*;
    use daw_core::{
        Edit, EnvelopeSettings, EnvelopeTiming, MidiNote, MidiPattern, OscillatorSettings,
    };

    #[test]
    fn note_lengths_use_the_stretched_clock_and_reconstruct_through_trims_and_repeats() {
        for rate in daw_core::SAMPLE_RATES {
            for (numerator, denominator) in [(1, 1), (2, 1), (2, 3)] {
                let mut project = Project {
                    sample_rate_hz: rate,
                    ..Default::default()
                };
                let track = project.add_midi_track().unwrap();
                let clip_id = project.add_midi_pattern(track, 0).unwrap();
                project.tracks[0].instrument = Some(OscillatorSettings::default().into());
                let layer = &mut project.tracks[0].instrument.as_mut().unwrap().layers[0];
                layer.envelopes[0].mode = EnvelopeMode::PerNote;
                layer.envelopes[0].settings = EnvelopeSettings {
                    timing: EnvelopeTiming::Note,
                    attack_note: 1.0,
                    decay_note: 0.0,
                    ..Default::default()
                };
                project
                    .edit(Edit::ReplaceMidiPattern {
                        clip_id,
                        pattern: MidiPattern {
                            length_ticks: 3840,
                            notes: vec![MidiNote {
                                id: Id::from_u128(1),
                                start_tick: 0,
                                length_ticks: 960,
                                pitch: 69,
                                velocity: 127,
                            }],
                        },
                    })
                    .unwrap();
                let length = project.midi_frame(960);
                let stretch = daw_core::ClipStretch::new(denominator, numerator).unwrap();
                let duration = (u128::from(length) * u128::from(numerator))
                    .div_ceil(u128::from(denominator)) as u64;
                let repeat_length = duration * 3;
                let offset = duration / 4;
                let clip = &mut project.tracks[0].clips[0];
                clip.stretch = Some(stretch);
                clip.source_offset_frame = offset;
                clip.repeat = Some(daw_core::ClipLoop {
                    length_frames: repeat_length,
                    phase_frame: 0,
                });
                clip.length_frames = repeat_length * 2;
                project.validate().unwrap();
                let mut played = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                assert_eq!(played.clips[0].events[0].duration_frames, duration);
                let age = duration / 4;
                for frame in 0..=age {
                    played.sample(frame);
                }
                let expected = (offset + age + 1) as f32 / duration as f32;
                assert!((played.voices[0].controls[0].envelope.level() - expected).abs() < 1e-6);
                let mut sought = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                sought.sample(age);
                assert_eq!(sought.voices[0].note_duration_frames, Some(duration));
                assert!((sought.voices[0].controls[0].envelope.level() - expected).abs() < 1e-6);
                // Cross the source-clock wrap, as sequential rendering does.
                sought.sample(repeat_length - 1);
                sought.sample(repeat_length + age);
                assert!((sought.voices[0].controls[0].envelope.level() - expected).abs() < 1e-6);
            }
        }
    }
}

#[cfg(test)]
mod restart_index_tests {
    use super::*;
    #[test]
    fn indexed_voice_restart_matches_full_prefix_replay_after_slot_reuse() {
        for stretch in [None, daw_core::ClipStretch::new(2, 3)] {
            let mut project = Project::default();
            let track = project.add_midi_track().unwrap();
            let clip_id = project.add_midi_pattern(track, 0).unwrap();
            project
                .edit(daw_core::Edit::ReplaceMidiPattern {
                    clip_id,
                    pattern: daw_core::MidiPattern {
                        length_ticks: 128 * 480,
                        notes: (0..128)
                            .map(|i| daw_core::MidiNote {
                                id: Id::new_v4(),
                                start_tick: i * 480,
                                length_ticks: if i % 3 == 0 { 960 } else { 240 },
                                pitch: 50 + (i % 24) as u8,
                                velocity: 80,
                            })
                            .collect(),
                    },
                })
                .unwrap();
            project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
            project.tracks[0].clips[0].stretch = stretch;
            project.tracks[0].clips[0].length_frames = 3_000_000;
            for frame in [0, 6000, 24_000, 100_000, 1_000_000] {
                let mut indexed = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                let mut reference = MidiTrackRuntime::new(&project, &project.tracks[0]).unwrap();
                let prepared = &reference.clips[0];
                let clip = &prepared.clip;
                let position = clip.source_frame(frame);
                let source = clip
                    .stretch
                    .map_or(position, |s| s.original_frame(position));
                reference.next_event = prepared.events.partition_point(|e| e.frame <= source);
                reference.shared.reset_notes();
                for v in &mut reference.voices {
                    v.reset();
                }
                for event in &prepared.events[..reference.next_event] {
                    let at = output_frame(clip, event.frame);
                    reference.shared.advance_to(at);
                    reference
                        .shared
                        .event(event.on.is_some(), event.on.map(|_| event.duration_frames));
                    let voice = &mut reference.voices[event.slot];
                    voice.advance_envelopes(at.saturating_sub(voice.reconstructed_at));
                    if let Some((pitch, velocity)) = event.on {
                        voice.note_on(
                            event.note,
                            clip_pitch(pitch, clip.transpose),
                            velocity,
                            event.duration_frames,
                        );
                    } else if voice.current == Some(event.note) {
                        voice.end_note();
                    }
                    voice.reconstructed_at = at;
                }
                reference.shared.advance_to(position);
                for voice in &mut reference.voices {
                    voice.advance_envelopes(position.saturating_sub(voice.reconstructed_at));
                }
                reference.last_clip = Some(clip.id);
                reference.last_source = Some(source);
                for at in frame..frame + 300 {
                    assert_eq!(indexed.sample(at), reference.sample(at));
                }
            }
        }
    }
}

#[cfg(test)]
mod live_pitch_tests {
    use super::*;
    use daw_core::{Edit, MidiNote, MidiPattern};
    #[test]
    fn live_pitch_keeps_voice_age_contours_modulation_and_source_phase_through_commit() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let mut p = Project {
                sample_rate_hz: rate,
                ..Default::default()
            };
            let track = p.add_midi_track().unwrap();
            p.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
            let layer = &mut p.tracks[0].instrument.as_mut().unwrap().layers[0];
            layer.envelopes[0].present = true;
            layer.envelopes[0].settings.attack.time_ms = 500.0;
            let clip = p.add_midi_pattern(track, 0).unwrap();
            let note = Id::new_v4();
            let pattern = |pitch| MidiPattern {
                length_ticks: 3840,
                notes: vec![MidiNote {
                    id: note,
                    start_tick: 0,
                    length_ticks: 3840,
                    pitch,
                    velocity: 100,
                }],
            };
            p.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(60),
            })
            .unwrap();
            p.tracks[0].clips[0].length_frames = u64::from(rate) * 2;
            let inputs = super::super::LiveMidiPitches::prepare(&p, &Default::default());
            let mut runtime = MidiTrackRuntime::new(&p, &p.tracks[0]).unwrap();
            runtime.bind_live_pitches(track, &inputs);
            for frame in 0..1000 {
                runtime.sample(frame);
            }
            let held = runtime.voices[0].held_frames;
            let gate = runtime.voices[0].current;
            inputs.set(track, clip, note, 72, true);
            runtime.sample(1000);
            assert_eq!(runtime.voices[0].held_frames, held + 1);
            assert_eq!(runtime.voices[0].current, gate);
            assert_eq!(runtime.voices[0].pitch, 72);
            assert_eq!(
                runtime.voices[0].layers[0].sources[0].frequency_target,
                note_frequency(72, 0)
            );
            for frame in 1001..2000 {
                runtime.sample(frame);
            }
            let mut reference = runtime.clone();
            inputs.set(track, clip, note, 72, false);
            p.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(72),
            })
            .unwrap();
            let committed = super::super::LiveMidiPitches::prepare(&p, &inputs);
            let mut prepared = MidiTrackRuntime::new(&p, &p.tracks[0]).unwrap();
            prepared.bind_live_pitches(track, &committed);
            assert!(runtime.compatible(&prepared));
            let age = runtime.voices[0].held_frames;
            runtime.retarget_controls(&mut prepared);
            assert_eq!(runtime.voices[0].held_frames, age);
            // Bind the same committed schedule to the uninterrupted reference. Source phase
            // and contour timing must match the compatible publication sample for sample.
            reference.clips = runtime.clips.clone();
            reference.bind_live_pitches(track, &committed);
            for frame in 2000..2500 {
                assert_eq!(runtime.sample(frame), reference.sample(frame));
            }
            let mut undo = p.clone();
            undo.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(60),
            })
            .unwrap();
            let undo_inputs = super::super::LiveMidiPitches::prepare(&undo, &committed);
            let mut next = MidiTrackRuntime::new(&undo, &undo.tracks[0]).unwrap();
            next.bind_live_pitches(track, &undo_inputs);
            runtime.retarget_controls(&mut next);
            runtime.sample(2500);
            assert_eq!(runtime.voices[0].pitch, 60);
            assert!(runtime.voices[0].held_frames > age);
        }
    }
}
