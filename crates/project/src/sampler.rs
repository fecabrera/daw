//! Worker-side sample preparation. The original asset and its clip mapping stay immutable.
use super::*;
use daw_core::{SamplerInput, SamplerSettings};

enum LayerSampleTarget {
    Layer(Id, Id, bool),
    BeatSubtrack(Id),
}

impl Session {
    pub fn sampler_preparation_needed(&self, project: &Project) -> bool {
        let requests = sampler_requests(project, &self.live_midi_pitches);
        self.sampler_audio.keys().any(|id| {
            !requests
                .iter()
                .any(|(s, _)| s.id == *id && s.input.is_some())
        }) || requests.iter().any(|(s, notes)| {
            let Some(input) = &s.input else {
                return false;
            };
            let Some(source) = self.audio_for_clip(&Clip::from(input)) else {
                return self.audio.contains_key(&input.asset_id)
                    || project
                        .audio_asset(input.asset_id)
                        .is_some_and(|a| a.compound_audio.is_some());
            };
            !self.sampler_audio.get(&s.id).is_some_and(|prepared| {
                sampler_lengths(project, s).is_ok_and(|lengths| {
                    lengths.into_iter().all(|length| {
                        prepared.duration(length).is_some_and(|variant| {
                            variant.matches(source, s, length, project.stretch_engine, notes)
                        })
                    })
                })
            })
        })
    }
    /// Assign the visible source range of a clip. No track effects or mixing are captured.
    pub fn with_sampler_clip(&self, track: Id, layer: Id, clip: &Clip) -> Result<Self> {
        self.with_sampler_clip_cancellable(track, layer, clip, &|| false)
    }
    pub fn with_sampler_clip_cancellable(
        &self,
        track: Id,
        layer: Id,
        clip: &Clip,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        self.with_clip_sample(
            LayerSampleTarget::Layer(track, layer, false),
            clip,
            cancelled,
        )
    }

    pub fn with_beat_sampler_clip(
        &self,
        subtrack: Id,
        clip: &Clip,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        self.with_clip_sample(LayerSampleTarget::BeatSubtrack(subtrack), clip, cancelled)
    }

    fn with_clip_sample(
        &self,
        target: LayerSampleTarget,
        clip: &Clip,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        if self.project.audio_asset(clip.asset_id).is_none() {
            return Err(error("The clip has no audio source"));
        }
        let mut next = self.clone();
        next.assign_sampler_input(target, SamplerInput::from(clip))?;
        next.prepare_stretches_cancellable(cancelled)?;
        Ok(next)
    }
    pub fn with_sampler_file(&self, track: Id, layer: Id, path: &Path) -> Result<Self> {
        let path = absolute(path)?;
        let data = daw_media::decode_audio_cached(&path, self.project.sample_rate_hz, &|| false)
            .map_err(error)?;
        self.with_sampler_decoded(track, layer, &path, data, &|| false)
    }
    /// Consume audio already decoded by the shared file-hover worker.
    pub fn with_sampler_decoded(
        &self,
        track: Id,
        layer: Id,
        path: &Path,
        data: AudioData,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        self.with_decoded_layer_audio(
            LayerSampleTarget::Layer(track, layer, false),
            path,
            data,
            cancelled,
        )
    }
    /// File drop on a regular layer installs Sampler atomically with prepared audio.
    pub fn with_layer_sampler_decoded(
        &self,
        track: Id,
        layer: Id,
        path: &Path,
        data: AudioData,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        self.with_decoded_layer_audio(
            LayerSampleTarget::Layer(track, layer, true),
            path,
            data,
            cancelled,
        )
    }

    pub fn with_beat_sampler_decoded(
        &self,
        subtrack: Id,
        path: &Path,
        data: AudioData,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        self.with_decoded_layer_audio(
            LayerSampleTarget::BeatSubtrack(subtrack),
            path,
            data,
            cancelled,
        )
    }
    fn with_decoded_layer_audio(
        &self,
        target: LayerSampleTarget,
        path: &Path,
        data: AudioData,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        if cancelled() {
            return Err(error("Sample preparation cancelled"));
        }
        let path = absolute(path)?;
        let data = data
            .at_sample_rate(self.project.sample_rate_hz)
            .map_err(error)?;
        if data.frame_count() == 0 {
            return Err(error("The sample is empty"));
        }
        let mut next = self.clone();
        let id = Id::new_v4();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let length_frames = data.frame_count();
        next.project.assets.push(Asset {
            id,
            name: name.clone(),
            midi_pattern: None,
            rendered_audio: None,
            compound_audio: None,
            beat_pattern: None,
            source: Source {
                kind: "external".into(),
                path: path.to_string_lossy().into_owned(),
                storage: None,
                path_kind: "absolute".into(),
            },
            decoded_frame_count: length_frames,
            source_metadata: data.metadata.clone(),
        });
        next.audio.insert(id, data);
        next.assign_sampler_input(
            target,
            SamplerInput {
                reversed: false,
                asset_id: id,
                name,
                source_offset_frame: 0,
                length_frames,
                repeat: None,
                stretch: None,
            },
        )?;
        next.prepare_stretches_cancellable(cancelled)?;
        Ok(next)
    }
    fn assign_sampler_input(
        &mut self,
        target: LayerSampleTarget,
        input: SamplerInput,
    ) -> Result<()> {
        let rate = self.project.sample_rate_hz;
        let tempo = self.project.tempo_bpm;
        let (layer, install) = match target {
            LayerSampleTarget::Layer(owner, id, install) => (
                self.project
                    .instrument_settings_mut(owner)
                    .and_then(|i| i.layers.iter_mut().find(|l| l.id == id))
                    .ok_or_else(|| error("Instrument layer no longer exists"))?,
                install,
            ),
            LayerSampleTarget::BeatSubtrack(owner) => {
                if self.project.beat_subtrack(owner).is_none() {
                    return Err(error("Beat subtrack no longer exists"));
                }
                let instrument = self
                    .project
                    .instrument_settings_mut(owner)
                    .ok_or_else(|| error("Beat subtrack instrument no longer exists"))?;
                let layer = instrument
                    .layers
                    .first_mut()
                    .ok_or_else(|| error("Beat subtrack has no instrument layer"))?;
                (layer, true)
            }
        };
        if install
            && layer
                .sampler
                .as_ref()
                .is_none_or(|s| s.granulizer.is_some())
        {
            layer.clear_sources();
            layer.sampler = Some(Default::default());
        }
        let sampler = layer
            .sampler
            .as_mut()
            .ok_or_else(|| error("Sampler no longer exists"))?;
        let first = sampler.input.is_none();
        sampler.start_frame = 0;
        sampler.end_frame = input.length_frames;
        if first {
            sampler.set_duration_from_frames(input.length_frames, rate, tempo);
        }
        sampler.input = Some(input);
        self.project.validate().map_err(error)
    }
    pub(super) fn prepare_sampler_audio(&mut self, cancelled: &impl Fn() -> bool) -> Result<()> {
        let engine = self.project.stretch_engine;
        self.prepare_sampler_audio_using(cancelled, |source, settings, length, notes, previous| {
            daw_media::PreparedSampler::new_cancellable(
                source, settings, length, engine, notes, previous, cancelled,
            )
            .map_err(error)
        })
    }
    pub(super) fn prepare_sampler_audio_parallel(
        &mut self,
        cancelled: &(impl Fn() -> bool + Sync),
    ) -> Result<()> {
        let engine = self.project.stretch_engine;
        self.prepare_sampler_audio_using(cancelled, |source, settings, length, notes, previous| {
            daw_media::PreparedSampler::new_parallel(
                source, settings, length, engine, notes, previous, cancelled,
            )
            .map_err(error)
        })
    }
    fn prepare_sampler_audio_using(
        &mut self,
        cancelled: &impl Fn() -> bool,
        prepare: impl Fn(
            &AudioData,
            &SamplerSettings,
            u64,
            &[u8],
            Option<&daw_media::PreparedSampler>,
        ) -> Result<daw_media::PreparedSampler>,
    ) -> Result<()> {
        let mut outputs = HashMap::new();
        for (s, notes) in sampler_requests(&self.project, &self.live_midi_pitches) {
            if cancelled() {
                return Err(error("Sample preparation cancelled"));
            }
            let Some(input) = &s.input else {
                continue;
            };
            let clip = Clip::from(input);
            let Some(source) = self.audio_for_clip(&clip) else {
                continue;
            };
            let length = s
                .output_frames(self.project.sample_rate_hz, self.project.tempo_bpm)
                .ok_or_else(|| error("Unsupported sampler duration"))?;
            let previous = self.sampler_audio.get(&s.id);
            let mut prepared = prepare(
                source,
                s,
                length,
                &notes,
                previous.and_then(|p| p.duration(length)),
            )?;
            for variant_length in sampler_lengths(&self.project, s)? {
                if variant_length == length {
                    continue;
                }
                let variant = prepare(
                    source,
                    s,
                    variant_length,
                    &notes,
                    previous.and_then(|p| p.duration(variant_length)),
                )?;
                prepared.add_duration(variant_length, variant);
            }
            outputs.insert(s.id, prepared);
        }
        if cancelled() {
            return Err(error("Sample preparation cancelled"));
        }
        // Publish every pitch together. Root buffers retain the existing preview lookup.
        self.audio
            .retain(|id, _| self.project.assets.iter().any(|a| a.id == *id));
        self.audio.extend(
            outputs
                .iter()
                .map(|(id, prepared)| (*id, prepared.root_audio().clone())),
        );
        self.sampler_audio = outputs;
        Ok(())
    }
}

/// Prepare the unique pitches referenced by each track, including hidden pattern notes.
/// Pattern copies and repeated clips share these buffers. Empty tracks need only the root.
fn sampler_requests<'a>(
    project: &'a Project,
    live: &HashMap<Id, Vec<u8>>,
) -> Vec<(&'a SamplerSettings, Vec<u8>)> {
    let mut requests = Vec::new();
    for track in &project.tracks {
        let Some(instrument) = &track.instrument else {
            continue;
        };
        if !instrument.layers.iter().any(|l| l.sampler.is_some()) {
            continue;
        }
        let mut notes: Vec<_> = track
            .clips
            .iter()
            .filter_map(|clip| {
                let pattern = project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)?
                    .midi_pattern
                    .as_ref()?;
                Some((clip, pattern))
            })
            .flat_map(|(clip, p)| {
                p.notes
                    .iter()
                    .map(move |n| daw_core::transposed_midi_pitch(n.pitch, clip.transpose))
            })
            .collect();
        notes.extend(live.get(&track.id).into_iter().flatten().copied());
        notes.sort_unstable();
        notes.dedup();
        for sampler in instrument.layers.iter().filter_map(|l| l.sampler.as_ref()) {
            requests.push((sampler, notes.clone()));
        }
    }
    for track in &project.tracks {
        if let Some(beat) = &track.beat {
            for row in &beat.subtracks {
                let mut notes: Vec<_> = track
                    .clips
                    .iter()
                    .map(|clip| daw_core::transposed_midi_pitch(row.trigger_pitch, clip.transpose))
                    .chain(std::iter::once(row.trigger_pitch))
                    .collect();
                notes.sort_unstable();
                notes.dedup();
                for sampler in row
                    .instrument
                    .layers
                    .iter()
                    .filter_map(|l| l.sampler.as_ref())
                {
                    requests.push((sampler, notes.clone()));
                }
            }
        }
    }
    requests
}

fn sampler_lengths(project: &Project, settings: &SamplerSettings) -> Result<Vec<u64>> {
    let mut lengths: Vec<_> = project
        .tempo_events()
        .iter()
        .map(|e| {
            settings
                .output_frames(project.sample_rate_hz, e.bpm)
                .ok_or_else(|| error("Unsupported sampler duration in tempo map"))
        })
        .collect::<Result<_>>()?;
    lengths.sort_unstable();
    lengths.dedup();
    Ok(lengths)
}
