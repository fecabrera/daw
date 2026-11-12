//! Worker-side sample preparation. The original asset and its clip mapping stay immutable.
use super::*;
use daw_core::{SamplerInput, SamplerSettings};

impl Session {
    pub fn sampler_preparation_needed(&self, project: &Project) -> bool {
        let requests = sampler_requests(project);
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
            let Some(length) = s.output_frames(project.sample_rate_hz, project.tempo_bpm) else {
                return true;
            };
            !self.sampler_audio.get(&s.id).is_some_and(|prepared| {
                prepared.matches(source, s, length, project.stretch_engine, notes)
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
        let mut next = self.clone();
        next.assign_sampler_input(track, layer, SamplerInput::from(clip))?;
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
        self.with_decoded_layer_audio(track, layer, path, data, cancelled, false)
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
        self.with_decoded_layer_audio(track, layer, path, data, cancelled, true)
    }
    fn with_decoded_layer_audio(
        &self,
        track: Id,
        layer: Id,
        path: &Path,
        data: AudioData,
        cancelled: &impl Fn() -> bool,
        replace_source: bool,
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
        let settings = next
            .project
            .instrument_settings_mut(track)
            .and_then(|i| i.layers.iter_mut().find(|l| l.id == layer))
            .ok_or_else(|| error("Instrument layer no longer exists"))?;
        if replace_source
            && settings
                .sampler
                .as_ref()
                .is_none_or(|s| s.granulizer.is_some())
        {
            settings.clear_sources();
            settings.sampler = Some(Default::default());
        }
        next.assign_sampler_input(
            track,
            layer,
            SamplerInput {
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
    fn assign_sampler_input(&mut self, track: Id, layer: Id, input: SamplerInput) -> Result<()> {
        let rate = self.project.sample_rate_hz;
        let tempo = self.project.tempo_bpm;
        let sampler = self
            .project
            .instrument_settings_mut(track)
            .and_then(|i| i.layers.iter_mut().find(|l| l.id == layer))
            .and_then(|l| l.sampler.as_mut())
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
        for (s, notes) in sampler_requests(&self.project) {
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
            let prepared = prepare(source, s, length, &notes, self.sampler_audio.get(&s.id))?;
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
fn sampler_requests(project: &Project) -> Vec<(&SamplerSettings, Vec<u8>)> {
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
