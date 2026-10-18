//! Worker-side sample preparation. The original asset and its clip mapping stay immutable.
use super::*;
use daw_core::{Edit, SamplerInput, SamplerSettings};

impl Session {
    pub fn sampler_preparation_needed(&self, project: &Project) -> bool {
        let key = |s: &SamplerSettings, p: &Project| {
            (
                s.id,
                s.input.clone(),
                s.start_frame,
                s.end_frame,
                s.output_frames(p.sample_rate_hz, p.tempo_bpm),
            )
        };
        let mut old: Vec<_> = self
            .project
            .samplers()
            .filter(|s| s.input.is_some())
            .map(|s| key(s, &self.project))
            .collect();
        let mut new: Vec<_> = project
            .samplers()
            .filter(|s| s.input.is_some())
            .map(|s| key(s, project))
            .collect();
        old.sort_by_key(|k| k.0);
        new.sort_by_key(|k| k.0);
        old != new
            || project
                .samplers()
                .any(|s| s.input.is_some() && !self.audio.contains_key(&s.id))
    }
    pub fn with_edit(&self, edit: Edit) -> Result<Self> {
        let mut next = self.clone();
        next.project.edit(edit).map_err(error)?;
        next.prepare_stretches()?;
        Ok(next)
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
        let data = daw_media::decode_wav_cached(&path, self.project.sample_rate_hz, &|| false)
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
        if cancelled() {
            return Err(error("Sample preparation cancelled"));
        }
        let path = absolute(path)?;
        let data = data
            .at_sample_rate(self.project.sample_rate_hz)
            .map_err(error)?;
        if data.samples.is_empty() {
            return Err(error("The sample is empty"));
        }
        let mut next = self.clone();
        let id = Id::new_v4();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let length_frames = data.samples.len() as u64;
        next.project.assets.push(Asset {
            id,
            name: name.clone(),
            midi_pattern: None,
            source: Source {
                kind: "external".into(),
                path: path.to_string_lossy().into_owned(),
                path_kind: "absolute".into(),
            },
            decoded_frame_count: length_frames,
            source_metadata: data.metadata.clone(),
        });
        next.audio.insert(id, data);
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
        let sampler = self
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .and_then(|t| t.instrument.as_mut())
            .and_then(|i| i.layers.iter_mut().find(|l| l.id == layer))
            .and_then(|l| l.sampler.as_mut())
            .ok_or_else(|| error("Sampler no longer exists"))?;
        let first = sampler.input.is_none();
        sampler.start_frame = 0;
        sampler.end_frame = input.length_frames;
        if first {
            sampler.set_duration_from_frames(
                input.length_frames,
                self.project.sample_rate_hz,
                self.project.tempo_bpm,
            );
        }
        sampler.input = Some(input);
        self.project.validate().map_err(error)
    }
    pub(super) fn prepare_sampler_audio(&mut self, cancelled: &impl Fn() -> bool) -> Result<()> {
        let mut outputs = Vec::new();
        for s in self.project.samplers() {
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
            let count = usize::try_from(s.end_frame - s.start_frame).map_err(error)?;
            daw_media::register_audio(source);
            let reservation = daw_media::AudioReservation::new(count as u64, 0).map_err(error)?;
            let mut samples = Vec::new();
            samples.try_reserve_exact(count).map_err(error)?;
            for local in s.start_frame..s.end_frame {
                if local % 4096 == 0 && cancelled() {
                    return Err(error("Sample preparation cancelled"));
                }
                samples.push(
                    source
                        .samples
                        .get(clip.source_frame(local) as usize)
                        .copied()
                        .unwrap_or_default(),
                );
            }
            let selected = daw_media::prepared_audio(source, samples);
            drop(reservation); // selected is now registered as a retained PCM owner.
            let length = s
                .output_frames(self.project.sample_rate_hz, self.project.tempo_bpm)
                .ok_or_else(|| error("Unsupported sampler duration"))?;
            let ratio = ClipStretch::new(count as u64, length)
                .ok_or_else(|| error("Invalid sampler region"))?;
            let prepared = daw_media::PreparedStretch::new_cancellable(
                &selected,
                ratio,
                self.project.stretch_engine,
                cancelled,
            )
            .map_err(error)?;
            outputs.push((s.id, prepared.audio));
        }
        // Synthetic IDs cannot alias assets. Publish only after every buffer succeeds.
        self.audio
            .retain(|id, _| self.project.assets.iter().any(|a| a.id == *id));
        self.audio.extend(outputs);
        Ok(())
    }
}
