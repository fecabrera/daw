mod cache_policy;
pub use cache_policy::{CachePolicy, CacheProfile};
mod memory;
pub use memory::{RendererMemory, prepared_model_bytes};
mod continuous_modulation;
mod effects;
mod latency;
mod layer_effects;
mod lfo;
mod midi;
mod modulation;
mod parallel_tracks;
mod schedule;
mod track_processing;
pub use parallel_tracks::{PreparationMode, PreparationStats};
mod transport_cache;
use daw_core::{Clip, Id, Project, linear_gain};
use daw_media::AudioData;
use std::collections::HashMap;
pub use transport_cache::{TransportCache, TransportCacheMemory};

#[derive(Clone)]
pub struct RenderPlan {
    pub project: Project,
    pub audio: HashMap<Id, AudioData>,
    pub sampler_audio: HashMap<Id, daw_media::PreparedSampler>,
    pub stretched_audio: daw_media::ClipAudioCache,
}
impl RenderPlan {
    pub fn audio_for_clip(&self, clip: &Clip) -> Option<&AudioData> {
        daw_media::clip_audio(
            clip,
            &self.audio,
            &self.stretched_audio,
            self.project.stretch_engine,
        )
    }
    fn needs_stateful_render(&self) -> bool {
        self.project.all_effects().next().is_some()
            || self
                .project
                .tracks
                .iter()
                .any(|t| t.kind == daw_core::TrackKind::Midi)
    }
    /// Exact isolated sample. Effects/MIDI require replay from zero: use a persistent
    /// `Renderer::render_offline_at` for sequential or forward-range consumers.
    pub fn sample_at(&self, frame: u64) -> [f32; 2] {
        if self.needs_stateful_render() {
            let mut output = [[0.0; 2]];
            self.render(frame, &mut output);
            return output[0];
        }
        let mut mix = [0.0; 2];
        for t in &self.project.tracks {
            if !self.project.audible(t) {
                continue;
            }
            let gain = linear_gain(t.gain_db);
            if let Some(c) = t
                .clips
                .iter()
                .find(|c| frame >= c.start_frame && frame < c.end())
            {
                let Some(audio) = self.audio_for_clip(c) else {
                    continue;
                };
                let local = frame - c.start_frame;
                let source_frame = c.source_frame(local);
                let Some(source) = audio.samples.get(source_frame as usize) else {
                    continue;
                };
                let fade = sample_fade(c, local, source_frame, self.project.sample_rate_hz);
                let coefficients = pan_coefficients(t.pan, audio.metadata.channels == 1);
                let clip_gain = linear_gain(c.gain_db);
                for ch in 0..2 {
                    mix[ch] += source[ch] * gain * fade * coefficients[ch] * clip_gain;
                }
            }
        }
        let gain = linear_gain(self.project.master.gain_db);
        [mix[0] * gain, mix[1] * gain]
    }
    pub fn render(&self, start: u64, out: &mut [[f32; 2]]) {
        if self.needs_stateful_render() {
            let mut renderer = Renderer::new(self.clone());
            renderer.seek(0);
            renderer.render_offline_at(start, out);
            return;
        }
        for (i, s) in out.iter_mut().enumerate() {
            *s = self.sample_at(start + i as u64);
        }
    }
}
fn sample_fade(clip: &daw_core::Clip, local: u64, source_frame: u64, sample_rate_hz: u32) -> f32 {
    let outer = clip_fade_at_rate(local, clip.length_frames, sample_rate_hz);
    clip.repeat.map_or(outer, |repeat| {
        outer.min(clip_fade_at_rate(
            source_frame - clip.source_offset_frame,
            repeat.length_frames,
            sample_rate_hz,
        ))
    })
}

pub fn clip_fade(position: u64, length: u64) -> f32 {
    clip_fade_at_rate(position, length, daw_core::SAMPLE_RATE)
}

pub fn clip_fade_at_rate(position: u64, length: u64, sample_rate_hz: u32) -> f32 {
    let fade = (u64::from(sample_rate_hz) / 200).max(1).min(length / 2);
    if fade == 0 {
        return 0.0;
    }
    ((position as f32 / fade as f32).min((length - 1 - position) as f32 / fade as f32))
        .clamp(0.0, 1.0)
}
pub fn pan_coefficients(pan: f32, mono: bool) -> [f32; 2] {
    if mono {
        let theta = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        [theta.cos().max(0.0), theta.sin().max(0.0)]
    } else {
        [1.0 - pan.max(0.0), 1.0 + pan.min(0.0)]
    }
}

/// Capacity for the largest current set of processed destination controls.
pub const MODULATION_SNAPSHOT_SIZE: usize = daw_unit_core::ModulationParameter::OTT.len();
pub type ModulationSnapshot = [f32; MODULATION_SNAPSHOT_SIZE];
fn parameter_snapshot<const N: usize>(values: [f32; N]) -> ModulationSnapshot {
    std::array::from_fn(|i| values.get(i).copied().unwrap_or(f32::NAN))
}

/// Stable identity for UI parameter telemetry. Layer IDs are scoped to their track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModulationTarget {
    Envelope {
        track: Id,
        layer: Id,
        unit: Id,
    },
    Effect(Id),
    /// Playback position in prepared sample frames; shares the atomic telemetry transport.
    Sampler {
        track: Id,
        layer: Id,
    },
    Macro {
        track: Id,
        layer: Id,
        unit: Id,
    },
    Lfo {
        track: Id,
        layer: Id,
        unit: Id,
    },
    Oscillator {
        track: Id,
        layer: Id,
        oscillator: Id,
    },
    Amplitude {
        track: Id,
        layer: Id,
    },
}

/// Clone only on control/workers: mutable DSP state is independent and meter handles detach.
/// Immutable decoded/prepared audio remains shared.
#[derive(Clone)]
pub struct Renderer {
    /// Immutable prepared model. Replace it through a new renderer and `inherit`.
    pub plan: RenderPlan,
    end_frame: u64,
    audio_tracks: Vec<schedule::AudioTrack>,
    track_index: HashMap<Id, usize>,
    effect_tracks: HashMap<Id, Option<usize>>,
    pub playhead: u64,
    pub playing: bool,
    paused: bool,
    playback_start: Option<u64>,
    pub ramp: f32,
    gains: Vec<TrackGains>,
    effects: Vec<effects::EffectChain>,
    master_effects: effects::EffectChain,
    alignment: latency::Alignment,
    midi: Vec<Option<midi::MidiTrackRuntime>>,
    master: f32,
    master_target: f32,
    master_remaining: u32,
    transition: u32,
    transition_from: [f32; 2],
    previous: [f32; 2],
    latent_held_mix: [f32; 2],
    source_fingerprint: u64,
}
/// Coherent transport metadata for worker preparation; no DSP ownership crosses threads here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransportState {
    pub frame: u64,
    pub playing: bool,
    pub paused: bool,
    pub playback_start: Option<u64>,
}
#[derive(Clone)]
struct TrackGains {
    id: Id,
    value: [f32; 4],
    target: [f32; 4],
    remaining: u32,
    peak: [f32; 2],
}
impl Renderer {
    pub fn new(plan: RenderPlan) -> Self {
        let end_frame = plan.project.end();
        let track_index = plan
            .project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let effect_tracks = plan
            .project
            .tracks
            .iter()
            .enumerate()
            .flat_map(|(i, t)| {
                t.effects
                    .iter()
                    .chain(t.instrument.iter().flat_map(|i| {
                        i.layers
                            .iter()
                            .flat_map(|l| &l.effects)
                            .chain(&i.main.effects)
                    }))
                    .map(move |e| (e.id, Some(i)))
            })
            .chain(plan.project.master.effects.iter().map(|e| (e.id, None)))
            .collect();
        let audio_tracks = plan
            .project
            .tracks
            .iter()
            .map(|t| schedule::AudioTrack::new(&plan, t))
            .collect();
        let any_solo = plan.project.tracks.iter().any(|t| t.soloed);
        let gains = plan
            .project
            .tracks
            .iter()
            .map(|t| {
                let gate = if daw_core::mix_audible(t.muted, t.soloed, any_solo) {
                    linear_gain(t.gain_db)
                } else {
                    0.0
                };
                let mono = pan_coefficients(t.pan, true);
                let stereo = pan_coefficients(t.pan, false);
                let target = [
                    mono[0] * gate,
                    mono[1] * gate,
                    stereo[0] * gate,
                    stereo[1] * gate,
                ];
                TrackGains {
                    id: t.id,
                    value: target,
                    target,
                    remaining: 0,
                    peak: [0.0; 2],
                }
            })
            .collect();
        let effects = plan
            .project
            .tracks
            .iter()
            .map(|track| {
                effects::EffectChain::new(
                    track,
                    plan.project.sample_rate_hz,
                    plan.project.buffer_size_frames,
                )
            })
            .collect();
        let master_effects = effects::EffectChain::chain(
            None,
            &plan.project.master.effects,
            plan.project.sample_rate_hz,
            plan.project.buffer_size_frames,
        );
        let master = linear_gain(plan.project.master.gain_db);
        let playhead = plan.project.transport.playhead_frame;
        let midi: Vec<_> = plan
            .project
            .tracks
            .iter()
            .map(|track| {
                midi::MidiTrackRuntime::with_audio(
                    &plan.project,
                    track,
                    &plan.audio,
                    &plan.sampler_audio,
                )
            })
            .collect();
        // Prepare source identity off the callback. Mixer/source control changes need only
        // parameter ramps; restarting the output transition would modulate a held sample.
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        plan.project.sample_rate_hz.hash(&mut hash);
        for (track, runtime) in plan.project.tracks.iter().zip(&midi) {
            track.id.hash(&mut hash);
            track.effects.len().hash(&mut hash);
            if let Some(runtime) = runtime {
                Some(runtime.fingerprint()).hash(&mut hash);
            } else {
                None::<u64>.hash(&mut hash);
                format!("{:?}", track.clips).hash(&mut hash);
                for clip in &track.clips {
                    if let Some(audio) = plan.audio_for_clip(clip) {
                        std::sync::Arc::as_ptr(&audio.samples).hash(&mut hash);
                        audio.sample_rate_hz.hash(&mut hash);
                        audio.metadata.channels.hash(&mut hash);
                    }
                }
            }
        }
        plan.project.master.effects.len().hash(&mut hash);
        // Layer row order is presentation only; hash each chain under its stable owner.
        let mut effect_groups = Vec::new();
        for track in &plan.project.tracks {
            effect_groups.push(((Some(track.id), None), track.effects.as_slice()));
            if let Some(instrument) = &track.instrument {
                effect_groups.push((
                    (Some(track.id), Some(instrument.main.id)),
                    instrument.main.effects.as_slice(),
                ));
                let mut layers: Vec<_> = instrument.layers.iter().collect();
                layers.sort_by_key(|l| l.id);
                for layer in layers {
                    effect_groups
                        .push(((Some(track.id), Some(layer.id)), layer.effects.as_slice()));
                }
            }
        }
        effect_groups.push(((None, None), plan.project.master.effects.as_slice()));
        for (owner, effects) in effect_groups {
            owner.hash(&mut hash);
            effects.len().hash(&mut hash);
            for effect in effects {
                effect.id.hash(&mut hash);
                match &effect.unit {
                    daw_core::EffectSettings::Eq3(_) => daw_unit_core::EQ_TYPE_ID.hash(&mut hash),
                    daw_core::EffectSettings::Drive(_) => {
                        daw_unit_core::DRIVE_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::AutoFilter(_) => {
                        daw_unit_core::AUTO_FILTER_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::Compressor(_) => {
                        daw_unit_core::COMPRESSOR_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::Ott(_) => daw_unit_core::OTT_TYPE_ID.hash(&mut hash),
                    daw_core::EffectSettings::Limiter(_) => {
                        daw_unit_core::LIMITER_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::StereoSpread(_) => {
                        daw_unit_core::STEREO_SPREAD_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::Saturator(_) => {
                        daw_unit_core::SATURATOR_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::Bitcrusher(_) => {
                        daw_unit_core::BITCRUSHER_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::DirectionMixer(_) => {
                        daw_unit_core::DIRECTION_MIXER_TYPE_ID.hash(&mut hash)
                    }
                    daw_core::EffectSettings::Missing { type_id, .. } => type_id.hash(&mut hash),
                }
            }
        }
        let source_fingerprint = hash.finish();
        let alignment = latency::Alignment::new(&plan.project);
        let mut renderer = Self {
            alignment,
            end_frame,
            audio_tracks,
            track_index,
            effect_tracks,
            midi,
            effects,
            master_effects,
            plan,
            playhead,
            playing: false,
            paused: false,
            playback_start: None,
            ramp: 0.0,
            gains,
            master,
            master_target: master,
            master_remaining: 0,
            transition: 0,
            transition_from: [0.0; 2],
            previous: [0.0; 2],
            latent_held_mix: [0.0; 2],
            source_fingerprint,
        };
        renderer.prime_latency(false);
        renderer
    }
    /// Read the last processed parameters without advancing DSP or allocating.
    /// Auto-filter returns cutoff L/R Hz, Q, level dB, wet fraction.
    /// Drive returns Volume dB, Gain, Tone, Color strength, Color frequency Hz. Amplitude returns
    /// the highest voice gain in dB in slot 0. Sampler keeps position in slot 0 and
    /// lowest/highest active gain dB in slots 1/2; unavailable readings are NaN.
    pub fn modulation_values(&self, target: ModulationTarget) -> Option<[f32; 5]> {
        self.modulation_snapshot(target)
            .map(|v| std::array::from_fn(|i| v[i]))
    }
    /// Full readings. EQ/Compressor use their parameter-list order; added unused slots contain NaN.
    pub fn modulation_snapshot(&self, target: ModulationTarget) -> Option<ModulationSnapshot> {
        match target {
            ModulationTarget::Envelope { track, layer, unit } => {
                let index = *self.track_index.get(&track)?;
                self.midi[index]
                    .as_ref()?
                    .envelope_snapshot(daw_core::ModulationSourceId { layer, unit })
            }

            ModulationTarget::Sampler { track, layer } => {
                let index = *self.track_index.get(&track)?;
                let midi = self.midi[index].as_ref()?;
                if !self.plan.project.tracks[index]
                    .instrument
                    .as_ref()?
                    .layers
                    .iter()
                    .any(|l| l.id == layer && l.sampler.is_some())
                {
                    return None;
                }
                let gain = midi.sampler_gain(layer);
                Some(parameter_snapshot([
                    midi.sampler_position(layer),
                    gain[0],
                    gain[1],
                ]))
            }
            ModulationTarget::Effect(id) => match self.effect_tracks.get(&id)? {
                Some(index) => self.effects[*index]
                    .modulation_values(id)
                    .or_else(|| self.midi[*index].as_ref()?.effect_snapshot(id)),
                None => self.master_effects.modulation_values(id),
            },
            ModulationTarget::Macro { track, layer, unit } => {
                let index = *self.track_index.get(&track)?;
                let value = self.midi[index]
                    .as_ref()?
                    .provider_values(daw_core::ModulationSourceId { layer, unit })?[0];
                Some(parameter_snapshot([value]))
            }
            ModulationTarget::Lfo { track, layer, unit } => {
                let index = *self.track_index.get(&track)?;
                let [depth, hz] = self.midi[index]
                    .as_ref()?
                    .provider_values(daw_core::ModulationSourceId { layer, unit })?;
                Some(parameter_snapshot([depth, hz]))
            }
            ModulationTarget::Oscillator {
                track,
                layer,
                oscillator,
            } => {
                let index = *self.track_index.get(&track)?;
                if !self.plan.project.tracks[index]
                    .instrument
                    .as_ref()?
                    .layers
                    .iter()
                    .any(|l| l.id == layer && l.oscillators.iter().any(|s| s.id == oscillator))
                {
                    return None;
                }
                Some(parameter_snapshot(
                    self.midi[index]
                        .as_ref()?
                        .oscillator_values(layer, oscillator),
                ))
            }
            ModulationTarget::Amplitude { track, layer } => {
                let index = *self.track_index.get(&track)?;
                if !self.plan.project.tracks[index]
                    .instrument
                    .as_ref()?
                    .layers
                    .iter()
                    .any(|l| l.id == layer && l.amplitude.present)
                {
                    return None;
                }
                let gain = self.midi[index].as_ref()?.amplitude_gain(layer);
                Some(parameter_snapshot([
                    if gain > 0.0 {
                        20.0 * gain.log10()
                    } else {
                        daw_core::SILENT_GAIN_DB
                    },
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ]))
            }
        }
    }
    /// Attach prepared meter handles before publishing a renderer to the callback.
    pub fn set_effect_meter(
        &mut self,
        id: Id,
        meter: std::sync::Arc<std::sync::atomic::AtomicU32>,
    ) {
        self.set_effect_peak_meter(
            id,
            std::sync::Arc::new(daw_unit_core::PeakMeter::new(meter)),
        );
    }
    /// Attach current and held readings using the same effect and layer traversal.
    pub fn set_effect_peak_meter(
        &mut self,
        id: Id,
        meter: std::sync::Arc<daw_unit_core::PeakMeter>,
    ) {
        self.master_effects.set_meter(id, &meter);
        for midi in self.midi.iter_mut().flatten() {
            midi.set_effect_meter(id, &meter);
        }
        for effects in &mut self.effects {
            effects.set_meter(id, &meter);
        }
    }
    pub fn set_ott_meter(
        &mut self,
        id: Id,
        meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>,
    ) {
        self.set_band_meter(id, meter);
    }
    /// Attach prepared EQ level or OTT gain-change handles by stable effect identity.
    pub fn set_band_meter(
        &mut self,
        id: Id,
        meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>,
    ) {
        self.set_band_peak_meter(
            id,
            std::sync::Arc::new(daw_unit_core::BandMeter::new(meter)),
        );
    }
    /// Attach signed band readings and reduction holds through the shared traversal.
    pub fn set_band_peak_meter(
        &mut self,
        id: Id,
        meter: std::sync::Arc<daw_unit_core::BandMeter<3>>,
    ) {
        self.master_effects.set_band_meter(id, &meter);
        for midi in self.midi.iter_mut().flatten() {
            midi.set_effect_band_meter(id, &meter);
        }
        for effects in &mut self.effects {
            effects.set_band_meter(id, &meter);
        }
    }
    /// The native host checks this before callback inheritance. Incompatible latency
    /// needs worker pre-roll; direct/offline users retain synchronous inheritance.
    pub fn can_inherit_without_preroll(&self, old: &Self) -> bool {
        let new_loop = &self.plan.project.transport.r#loop;
        let old_loop = &old.plan.project.transport.r#loop;
        self.alignment.compatible(&old.alignment)
            && self.midi.iter().enumerate().all(|(index, runtime)| {
                runtime.as_ref().is_none_or(|next| {
                    old.track_index
                        .get(&self.plan.project.tracks[index].id)
                        .and_then(|&i| old.midi[i].as_ref())
                        .is_none_or(|previous| {
                            next.effect_latency_compatible(previous)
                                && (!next.needs_envelope_history() || next.compatible(previous))
                        })
                })
            })
            && (self.alignment.total == 0
                || (new_loop.enabled == old_loop.enabled
                    && new_loop.start_frame == old_loop.start_frame
                    && new_loop.end_frame == old_loop.end_frame))
    }
    pub fn transport_state(&self) -> TransportState {
        TransportState {
            frame: self.playhead,
            playing: self.playing,
            paused: self.paused,
            playback_start: self.playback_start,
        }
    }
    /// Control/worker thread only: reconstruct sources and alignment at the requested clock.
    pub fn prepare_transport(&mut self, state: TransportState) {
        self.prepare_transport_cancellable(state, || false);
    }
    /// Worker-only history replay. Cancellation is checked once per prepared block.
    pub fn prepare_transport_cancellable(
        &mut self,
        state: TransportState,
        mut cancelled: impl FnMut() -> bool,
    ) -> bool {
        self.playhead = state.frame;
        self.playing = state.playing && self.end_frame > 0;
        self.paused = state.paused;
        self.playback_start = state.playback_start;
        self.reset_processing();
        let history = self
            .midi
            .iter()
            .flatten()
            .any(|m| m.needs_envelope_history());
        if history {
            let block = u64::from(self.plan.project.buffer_size_frames.max(1));
            for frame in 0..state.frame {
                if frame % block == 0 && cancelled() {
                    return false;
                }
                self.offline_frame(frame, self.playing || self.paused);
            }
        }
        if cancelled() {
            return false;
        }
        self.prime_latency(self.playing || self.paused);
        true
    }
    /// Bounded activation of a worker-prepared transport, retaining the output fade.
    pub fn activate_transport(&mut self, old: &Self) {
        self.ramp = old.ramp;
        self.previous = old.previous;
        self.transition_from = old.previous;
        self.transition = (self.plan.project.sample_rate_hz / 200).max(1);
    }
    /// Resume an already prepared paused transport without resetting its history.
    pub fn resume(&mut self) {
        if self.end_frame > 0 {
            self.playing = true;
            self.paused = false;
        }
    }
    pub fn inherit(&mut self, old: &mut Self) {
        let fade_frames = (self.plan.project.sample_rate_hz / 200).max(1);
        let aligned = self.can_inherit_without_preroll(old);
        self.playhead = old.playhead;
        self.playing = old.playing;
        self.paused = old.paused;
        self.playback_start = old.playback_start;
        self.ramp = old.ramp;
        self.master = old.master;
        self.master_remaining = fade_frames;
        self.previous = old.previous;
        self.latent_held_mix = old.latent_held_mix;
        if self.source_fingerprint == old.source_fingerprint {
            self.transition_from = old.transition_from;
            self.transition = old.transition;
        } else {
            self.transition_from = old.previous;
            self.transition = fade_frames;
        }
        if aligned && self.plan.project.sample_rate_hz == old.plan.project.sample_rate_hz {
            for (index, track) in self.plan.project.tracks.iter().enumerate() {
                if let Some(&previous) = old.track_index.get(&track.id) {
                    if let Some(next) = &mut self.midi[index] {
                        next.inherit_track_effects(&mut old.effects[previous]);
                    }
                    if let Some(previous) = &mut old.midi[previous] {
                        previous
                            .inherit_into_track_effects(&mut self.effects[index], &track.effects);
                    }
                }
            }
        }
        for (index, runtime) in self.midi.iter_mut().enumerate() {
            let id = self.plan.project.tracks[index].id;
            if let Some(&previous) = old.track_index.get(&id)
                && runtime
                    .as_ref()
                    .zip(old.midi[previous].as_ref())
                    .is_some_and(|(a, b)| a.compatible(b))
            {
                if let Some((next, previous)) = runtime.as_mut().zip(old.midi[previous].as_mut()) {
                    previous.retarget_controls(next);
                }
                std::mem::swap(runtime, &mut old.midi[previous]);
            }
        }
        if aligned && self.plan.project.sample_rate_hz == old.plan.project.sample_rate_hz {
            self.alignment.inherit(&mut old.alignment);
            self.master_effects
                .inherit_chain(&mut old.master_effects, &self.plan.project.master.effects);
            for (effects, track) in self.effects.iter_mut().zip(&self.plan.project.tracks) {
                if let Some(&index) = old.track_index.get(&track.id) {
                    effects.inherit_chain(&mut old.effects[index], &track.effects);
                }
            }
        }
        for g in &mut self.gains {
            if let Some(&index) = old.track_index.get(&g.id) {
                g.value = old.gains[index].value;
            } else {
                g.value = [0.0; 4];
            }
            g.remaining = fade_frames;
        }
        if !aligned {
            self.reset_processing();
            self.prime_latency(self.playing || self.paused);
        }
    }
    pub fn latency_frames(&self) -> u64 {
        self.alignment.total
    }
    fn loop_region(&self, live: bool) -> Option<(u64, u64)> {
        let region = &self.plan.project.transport.r#loop;
        (live && region.enabled && region.end_frame > region.start_frame)
            .then_some((region.start_frame, region.end_frame))
    }
    fn feed_frame(&mut self, frame: u64, live: bool) -> u64 {
        let region = self.loop_region(live);
        let frame = latency::wrapped_frame(frame, region);
        if live
            && self.alignment.total > 0
            && self.alignment.previous_feed.is_some_and(|old| frame < old)
        {
            for runtime in self.midi.iter_mut().flatten() {
                runtime.end_notes();
            }
        }
        self.alignment.previous_feed = Some(frame);
        frame
    }
    fn reset_processing(&mut self) {
        self.alignment.reset();
        self.master_effects.reset();
        for effects in &mut self.effects {
            effects.reset();
        }
        for runtime in self.midi.iter_mut().flatten() {
            runtime.reset();
        }
    }
    fn prime_latency(&mut self, live: bool) {
        self.alignment.live = live;
        if self.alignment.total == 0 {
            return;
        }
        let mut start = self.playhead;
        if let Some((first, end)) = self.loop_region(live)
            && (start < first || start >= end)
        {
            start = first;
        }
        for offset in 0..self.alignment.total {
            let future = latency::advanced_frame(start, offset, self.loop_region(live));
            let frame = self.feed_frame(future, live);
            self.offline_frame(frame, live);
        }
    }
    pub fn seek(&mut self, frame: u64) {
        self.reset_processing();
        self.playhead = frame;
        self.prime_latency(self.playing || self.paused);
        self.transition_from = self.previous;
        self.transition = (self.plan.project.sample_rate_hz / 200).max(1);
    }
    pub fn play(&mut self) {
        if self.end_frame > 0 {
            if !self.playing && !self.paused {
                self.playback_start = Some(self.playhead);
            }
            if !self.paused && self.alignment.total > 0 {
                self.reset_processing();
                self.prime_latency(true);
            }
            self.paused = false;
            self.playing = true;
        }
    }
    pub fn pause(&mut self) {
        if self.playing {
            self.paused = true;
        }
        self.playing = false;
    }
    /// A playback run still owns its original start until Stop consumes it, including while paused.
    pub fn can_return_to_playback_start(&self) -> bool {
        self.playback_start.is_some()
    }
    pub fn stop(&mut self) {
        self.master_effects.reset();
        for effects in &mut self.effects {
            effects.reset();
        }
        for runtime in self.midi.iter_mut().flatten() {
            runtime.reset();
        }
        self.playing = false;
        self.paused = false;
        if let Some(start) = self.playback_start.take() {
            self.seek(start);
        }
    }
    /// Exact range from the current offline history. Forward requests reuse DSP state;
    /// backwards requests reset and replay. Construct a dedicated renderer starting at zero.
    pub fn render_offline_at(&mut self, start: u64, out: &mut [[f32; 2]]) {
        if start < self.playhead {
            self.seek(0);
        }
        let mut scratch = [[0.0; 2]; 512];
        while self.playhead < start {
            let count = (start - self.playhead).min(scratch.len() as u64) as usize;
            self.render_offline(&mut scratch[..count]);
        }
        self.render_offline(out);
    }
    pub fn render_offline(&mut self, out: &mut [[f32; 2]]) {
        if self.alignment.live && self.alignment.total > 0 {
            self.reset_processing();
            self.prime_latency(false);
        }
        for sample in out {
            let frame = self.feed_frame(self.playhead.saturating_add(self.alignment.total), false);
            *sample = self.offline_frame(frame, false);
            self.playhead = self.playhead.saturating_add(1);
        }
    }
    fn offline_frame(&mut self, frame: u64, live: bool) -> [f32; 2] {
        let mut mix = [0.0; 2];
        let loop_region = self.loop_region(live);
        for (index, ((track, midi), effects)) in self
            .plan
            .project
            .tracks
            .iter()
            .zip(&mut self.midi)
            .zip(&mut self.effects)
            .enumerate()
        {
            let (source, mono) = track_processing::TrackProcessor {
                audio: &mut self.audio_tracks[index],
                midi,
                effects,
                delay: &mut self.alignment.tracks[index],
                clips: &track.clips,
            }
            .sample(frame, self.plan.project.tempo_bpm, loop_region, true);
            let gains = self.gains[index].target;
            let offset = if mono { 0 } else { 2 };
            for ch in 0..2 {
                mix[ch] += source[ch] * gains[offset + ch];
            }
        }
        self.sample_master(mix, frame, loop_region)
            .map(|value| value * self.master_target)
    }
    fn sample_master(
        &mut self,
        mix: [f32; 2],
        frame: u64,
        loop_region: Option<(u64, u64)>,
    ) -> [f32; 2] {
        self.master_effects
            .sample(
                mix,
                Some(false),
                latency::delayed_frame(frame, self.alignment.track_max, loop_region),
                self.plan.project.tempo_bpm,
                loop_region,
                (&[], None),
            )
            .0
    }
    /// Reset once per output buffer, outside the per-sample loop.
    pub fn reset_track_peaks(&mut self) {
        for g in &mut self.gains {
            g.peak = [0.0; 2];
        }
    }
    /// Post-fader stereo peaks before master gain, in project track order.
    pub fn track_peaks(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        self.gains.iter().map(|g| g.peak)
    }
    pub fn next_sample(&mut self) -> [f32; 2] {
        let fade_frames = (self.plan.project.sample_rate_hz / 200).max(1) as f32;
        let target = if self.playing { 1.0 } else { 0.0 };
        self.ramp = if self.ramp < target {
            (self.ramp + 1.0 / fade_frames).min(target)
        } else {
            (self.ramp - 1.0 / fade_frames).max(target)
        };
        let mut mix = [0.0; 2];
        if self.playing || self.ramp > 0.0 {
            let l = &self.plan.project.transport.r#loop;
            if self.playing
                && l.enabled
                && (self.playhead < l.start_frame || self.playhead >= l.end_frame)
            {
                if self.alignment.total == 0 {
                    for runtime in self.midi.iter_mut().flatten() {
                        runtime.end_notes();
                    }
                }
                self.playhead = l.start_frame;
                self.transition_from = self.previous;
                self.transition = (self.plan.project.sample_rate_hz / 200).max(1);
            }
            if self.playing && !l.enabled && self.playhead >= self.end_frame {
                self.playing = false;
            }
            if !self.playing && self.alignment.total > 0 {
                mix = self.latent_held_mix;
            } else {
                let frame = if self.playing {
                    let future = latency::advanced_frame(
                        self.playhead,
                        self.alignment.total,
                        self.loop_region(true),
                    );
                    self.feed_frame(future, true)
                } else {
                    self.playhead
                };
                let loop_region = self.loop_region(true);
                for (index, (((track, g), midi), effects)) in self
                    .plan
                    .project
                    .tracks
                    .iter()
                    .zip(&mut self.gains)
                    .zip(&mut self.midi)
                    .zip(&mut self.effects)
                    .enumerate()
                {
                    if g.remaining > 0 {
                        for ch in 0..4 {
                            g.value[ch] += (g.target[ch] - g.value[ch]) / g.remaining as f32;
                        }
                        g.remaining -= 1;
                    }
                    let (source, mono) = track_processing::TrackProcessor {
                        audio: &mut self.audio_tracks[index],
                        midi,
                        effects,
                        delay: &mut self.alignment.tracks[index],
                        clips: &track.clips,
                    }
                    .sample(
                        frame,
                        self.plan.project.tempo_bpm,
                        loop_region,
                        self.playing,
                    );
                    let base = if mono { 0 } else { 2 };
                    for ch in 0..2 {
                        let sample = source[ch] * g.value[base + ch];
                        mix[ch] += sample;
                        g.peak[ch] = g.peak[ch].max((sample * self.ramp).abs());
                    }
                }
                mix = self.sample_master(mix, frame, loop_region);
                self.latent_held_mix = mix;
            }
            if self.playing {
                self.playhead = self.playhead.saturating_add(1);
            }
        }
        if self.master_remaining > 0 {
            self.master += (self.master_target - self.master) / self.master_remaining as f32;
            self.master_remaining -= 1;
        }
        for x in &mut mix {
            *x *= self.master * self.ramp;
        }
        if self.transition > 0 {
            let progress = 1.0 - self.transition as f32 / fade_frames;
            for (ch, value) in mix.iter_mut().enumerate() {
                *value = self.transition_from[ch] * (1.0 - progress) + *value * progress;
            }
            self.transition -= 1;
        }
        self.previous = mix;
        mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mono_power_and_stereo_balance() {
        let center = pan_coefficients(0.0, true);
        assert!((center[0] * center[0] + center[1] * center[1] - 1.0).abs() < 1e-6);
        assert_eq!(pan_coefficients(0.0, false), [1.0, 1.0]);
        assert_eq!(pan_coefficients(1.0, false), [0.0, 1.0]);
    }
    #[test]
    fn clip_fades_cover_short_clips() {
        assert_eq!(clip_fade(0, 1000), 0.0);
        assert_eq!(clip_fade(999, 1000), 0.0);
        assert_eq!(clip_fade(240, 1000), 1.0);
        assert_eq!(clip_fade(50, 100), 0.98);
    }
    #[test]
    fn clip_gain_scales_audio_before_track_gain() {
        let mut plan = plan();
        let unity = plan.sample_at(1000);
        assert!((unity[0] - 0.25).abs() < 1e-5);
        plan.project.tracks[0].clips[0].gain_db = 6.0;
        let louder = plan.sample_at(1000);
        assert!((louder[0] / unity[0] - 10.0_f32.powf(6.0 / 20.0)).abs() < 1e-4);
        let mut track = schedule::AudioTrack::new(&plan, &plan.project.tracks[0]);
        let (scheduled, _) = track.sample(&plan.project.tracks[0].clips, 1000);
        assert!((scheduled[0] - louder[0]).abs() < 1e-5);
        plan.project.tracks[0].clips[0].gain_db = daw_core::SILENT_GAIN_DB;
        assert_eq!(plan.sample_at(1000), [0.0, 0.0]);
    }
    fn plan() -> RenderPlan {
        use daw_core::{Asset, Clip, Source, SourceMetadata};
        use std::sync::Arc;
        let mut project = Project::default();
        project.add_track().unwrap();
        let id = Id::new_v4();
        let metadata = SourceMetadata {
            sample_rate_hz: 48_000,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        };
        project.assets.push(Asset {
            midi_pattern: None,
            id,
            name: "Test".into(),
            source: Source {
                kind: "external".into(),
                path: "/test.wav".into(),
                path_kind: "absolute".into(),
            },
            source_metadata: metadata.clone(),
            decoded_frame_count: 2000,
        });
        project.tracks[0].clips.push(Clip {
            stretch: None,
            id: Id::new_v4(),
            asset_id: id,
            name: "Test".into(),
            color: None,
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 2000,
            repeat: None,
            gain_db: 0.0,
            transpose: 0,
        });
        RenderPlan {
            sampler_audio: Default::default(),
            stretched_audio: HashMap::new(),
            project,
            audio: HashMap::from([(
                id,
                AudioData {
                    sample_rate_hz: daw_core::SAMPLE_RATE,
                    samples: Arc::new(vec![[0.25, -0.25]; 2000]),
                    metadata,
                    peaks: Arc::new(vec![]),
                },
            )]),
        }
    }
    #[test]
    fn processing_rate_keeps_fades_and_live_offline_samples_consistent() {
        for rate in daw_core::SAMPLE_RATES {
            let mut plan = plan();
            plan.project = plan.project.with_audio_settings(rate, 64).unwrap();
            let id = plan.project.assets[0].id;
            plan.audio
                .insert(id, plan.audio[&id].at_sample_rate(rate).unwrap());
            let mut renderer = Renderer::new(plan.clone());
            renderer.play();
            let nominal_fade = u64::from(rate / 200);
            assert_eq!(clip_fade_at_rate(nominal_fade, u64::from(rate), rate), 1.0);
            assert!(
                (clip_fade_at_rate(nominal_fade / 2, u64::from(rate), rate) - 0.5).abs() < 0.01
            );
            for frame in 0..plan.project.end() {
                let sample = renderer.next_sample();
                if frame >= nominal_fade {
                    assert_eq!(sample, plan.sample_at(frame));
                }
            }
        }
    }

    #[test]
    fn repeated_clip_playback_and_render_share_the_trimmed_source_and_fades() {
        let mut plan = plan();
        let id = plan.project.tracks[0].clips[0].asset_id;
        let samples = (0..2000)
            .map(|frame| [frame as f32 / 2000.0, -(frame as f32) / 4000.0])
            .collect::<Vec<_>>();
        plan.audio.get_mut(&id).unwrap().samples = std::sync::Arc::new(samples);
        let original = plan.audio[&id].samples.clone();
        for phase in [0, 375] {
            let clip = &mut plan.project.tracks[0].clips[0];
            clip.source_offset_frame = 400;
            clip.length_frames = 4250;
            clip.repeat = Some(daw_core::ClipLoop {
                length_frames: 1000,
                phase_frame: phase,
            });
            plan.project.validate().unwrap();
            let mut renderer = Renderer::new(plan.clone());
            renderer.play();
            renderer.ramp = 1.0;
            let mut exported = vec![[0.0; 2]; 4250];
            plan.render(0, &mut exported);
            for (local, actual) in exported.iter().enumerate() {
                let position = (local + phase as usize) % 1000;
                let fade = clip_fade(position as u64, 1000).min(clip_fade(local as u64, 4250));
                let expected = original[400 + position].map(|sample| sample * fade);
                assert_eq!(*actual, expected);
                assert_eq!(renderer.next_sample(), expected);
            }
            assert_eq!(renderer.next_sample(), [0.0; 2]);
            assert!(!renderer.playing);
            assert!(std::sync::Arc::ptr_eq(&plan.audio[&id].samples, &original));
            plan.audio.clear();
            assert_eq!(plan.sample_at(1000), [0.0; 2]);
            plan.audio.insert(id, renderer.plan.audio[&id].clone());
        }
    }

    #[test]
    fn transport_pauses_at_position_wraps_and_stops_at_end() {
        let mut renderer = Renderer::new(plan());
        renderer.play();
        for _ in 0..600 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 600);
        let expected = renderer.plan.sample_at(600);
        assert_eq!(renderer.next_sample(), expected);
        renderer.pause();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 601);
        assert_eq!(renderer.next_sample(), [0.0; 2]);
        renderer.seek(900);
        renderer.play();
        renderer.plan.project.transport.r#loop = daw_core::Loop {
            enabled: true,
            start_frame: 500,
            end_frame: 1000,
        };
        for _ in 0..101 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 501);
        renderer.stop();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 0);
        renderer.plan.project.transport.r#loop.enabled = false;
        renderer.seek(1999);
        renderer.play();
        renderer.next_sample();
        renderer.next_sample();
        assert!(!renderer.playing);
        assert_eq!(renderer.playhead, 2000);
        renderer.stop();
        assert_eq!(renderer.playhead, 1999);
        renderer.play();
        renderer.next_sample();
        renderer.next_sample();
        renderer.seek(700);
        renderer.play();
        renderer.next_sample();
        renderer.stop();
        assert_eq!(renderer.playhead, 700);
    }
    #[test]
    fn stop_returns_to_original_start_after_pause_seek_and_live_edit() {
        let mut renderer = Renderer::new(plan());
        renderer.seek(400);
        renderer.play();
        for _ in 0..260 {
            renderer.next_sample();
        }
        renderer.pause();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 660);
        assert_eq!(renderer.next_sample(), [0.0; 2]);
        renderer.play();
        renderer.seek(850);
        for _ in 0..20 {
            renderer.next_sample();
        }
        let mut next = Renderer::new(plan());
        next.inherit(&mut renderer);
        assert!(next.playing);
        next.pause();
        next.stop();
        assert!(!next.playing);
        assert_eq!(next.playhead, 400);
        for _ in 0..300 {
            next.next_sample();
        }
        assert_eq!(next.next_sample(), [0.0; 2]);

        next.seek(1200);
        next.play();
        for _ in 0..20 {
            next.next_sample();
        }
        next.stop();
        assert_eq!(next.playhead, 1200);
        next.seek(1500);
        next.stop();
        assert_eq!(next.playhead, 1500);
    }
    #[test]
    fn track_peaks_measure_separate_post_fader_channels_before_master() {
        let mut p = plan();
        let mut second = p.project.tracks[0].clone();
        second.id = Id::new_v4();
        second.pan = -1.0;
        p.project.tracks.push(second);
        p.project.tracks[0].gain_db = 20.0;
        p.project.tracks[0].pan = 0.5;
        p.project.master.gain_db = -20.0;
        let mut renderer = Renderer::new(p.clone());
        renderer.playing = true;
        for _ in 0..500 {
            renderer.next_sample();
        }
        assert_eq!(
            renderer.track_peaks().collect::<Vec<_>>(),
            vec![[1.25, 2.5], [0.25, 0.0]]
        );
        assert!(renderer.next_sample().iter().all(|v| v.abs() < 1.0));

        p.project.tracks[0].muted = true;
        let mut muted = Renderer::new(p.clone());
        muted.playing = true;
        for _ in 0..500 {
            muted.next_sample();
        }
        assert_eq!(
            muted.track_peaks().collect::<Vec<_>>(),
            vec![[0.0, 0.0], [0.25, 0.0]]
        );

        p.project.tracks[0].soloed = true;
        let mut solo = Renderer::new(p);
        solo.playing = true;
        for _ in 0..500 {
            solo.next_sample();
        }
        assert_eq!(
            solo.track_peaks().collect::<Vec<_>>(),
            vec![[1.25, 2.5], [0.0, 0.0]]
        );
        solo.playing = false;
        for _ in 0..300 {
            solo.next_sample();
        }
        solo.reset_track_peaks();
        solo.next_sample();
        assert!(solo.track_peaks().all(|peak| peak == [0.0; 2]));
    }
    #[test]
    fn live_controls_reach_offline_targets_after_five_ms() {
        let mut old = Renderer::new(plan());
        old.playing = true;
        for _ in 0..500 {
            old.next_sample();
        }
        let mut next_plan = old.plan.clone();
        next_plan.project.tracks[0].gain_db = -6.0;
        next_plan.project.tracks[0].pan = 0.5;
        next_plan.project.master.gain_db = -6.0;
        let mut next = Renderer::new(next_plan);
        next.inherit(&mut old);
        for _ in 0..240 {
            next.next_sample();
        }
        let target = next.plan.sample_at(next.playhead);
        assert_eq!(next.next_sample(), target);
        let previous = next.previous;
        next.seek(1000);
        assert_eq!(next.next_sample(), previous);
    }
}
