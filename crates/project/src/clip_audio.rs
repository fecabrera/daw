//! Worker-side preparation and shared lookup of complete clip audio variants.
use crate::{Result, Session, error};
use daw_core::{Clip, Project};
use daw_media::ClipAudioKey;
use std::collections::HashMap;

fn requests(project: &Project) -> impl Iterator<Item = ClipAudioKey> + '_ {
    project
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .filter_map(ClipAudioKey::for_clip)
        .chain(
            project
                .samplers()
                .filter_map(|sampler| sampler.input.as_ref())
                .filter_map(|input| ClipAudioKey::for_clip(&Clip::from(input))),
        )
}
impl Session {
    pub fn with_edit(&self, edit: daw_core::Edit) -> Result<Self> {
        self.with_edit_cancellable(edit, &|| false)
    }
    /// Publish settings and all required clip/Sampler variants as one transaction.
    pub fn with_edit_cancellable(
        &self,
        edit: daw_core::Edit,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        let mut next = self.clone();
        next.project.edit(edit).map_err(error)?;
        next.prepare_stretches_cancellable(cancelled)?;
        Ok(next)
    }
    /// Decide on the control thread; all missing DSP is built by the existing job.
    pub fn audio_preparation_needed(&self, project: &Project) -> bool {
        let expected: std::collections::HashSet<_> = requests(project)
            .filter(|key| self.audio.contains_key(&key.asset_id))
            .collect();
        self.stretched_audio.len() != expected.len()
            || expected.iter().any(|key| {
                !self.stretched_audio.get(key).is_some_and(|prepared| {
                    key.matches(prepared, &self.audio[&key.asset_id], project.stretch_engine)
                })
            })
            || self.sampler_preparation_needed(project)
    }
    /// Only MIDI content/placement changes can enter the first nonblocking edit path.
    /// Source imports, audio transforms, instrument changes and rate changes retain their transactions.
    pub fn midi_only_preparation_change(&self, project: &Project) -> bool {
        let before = &self.project;
        if before.sample_rate_hz != project.sample_rate_hz
            || before.tempo_bpm != project.tempo_bpm
            || before.stretch_engine != project.stretch_engine
            || before.tracks.len() != project.tracks.len()
        {
            return false;
        }
        if !before
            .assets
            .iter()
            .filter(|a| a.midi_pattern.is_none())
            .eq(project.assets.iter().filter(|a| a.midi_pattern.is_none()))
        {
            return false;
        }
        before.tracks.iter().zip(&project.tracks).all(|(old, new)| {
            old.id == new.id
                && old.instrument == new.instrument
                && old
                    .clips
                    .iter()
                    .filter(|clip| {
                        before
                            .assets
                            .iter()
                            .any(|a| a.id == clip.asset_id && a.midi_pattern.is_none())
                    })
                    .eq(new.clips.iter().filter(|clip| {
                        project
                            .assets
                            .iter()
                            .any(|a| a.id == clip.asset_id && a.midi_pattern.is_none())
                    }))
        })
    }
    pub fn audio_for_clip(&self, clip: &Clip) -> Option<&daw_media::AudioData> {
        daw_media::clip_audio(
            clip,
            &self.audio,
            &self.stretched_audio,
            self.project.stretch_engine,
        )
    }

    /// Build missing stretch buffers on a worker. Original audio remains untouched.
    pub fn prepare_stretches(&mut self) -> Result<()> {
        self.prepare_stretches_cancellable(&|| false)
    }
    pub fn prepare_stretches_cancellable(&mut self, cancelled: &impl Fn() -> bool) -> Result<()> {
        self.prepare_stretches_using(cancelled, |next| next.prepare_sampler_audio(cancelled))
    }
    pub(super) fn prepare_stretches_parallel(
        &mut self,
        cancelled: &(impl Fn() -> bool + Sync),
    ) -> Result<()> {
        self.prepare_stretches_using(cancelled, |next| {
            next.prepare_sampler_audio_parallel(cancelled)
        })
    }
    fn prepare_stretches_using(
        &mut self,
        cancelled: &impl Fn() -> bool,
        samplers: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        self.project.validate().map_err(error)?;
        let mut prepared = HashMap::new();
        for key in requests(&self.project) {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            if prepared.contains_key(&key) {
                continue;
            }
            if let Some(audio) = self.stretched_audio.get(&key).filter(|prepared| {
                self.audio.get(&key.asset_id).is_some_and(|source| {
                    key.matches(prepared, source, self.project.stretch_engine)
                })
            }) {
                prepared.insert(key, audio.clone());
            } else if let Some(audio) = self.audio.get(&key.asset_id) {
                prepared.insert(
                    key,
                    key.prepare(audio, self.project.stretch_engine, cancelled)
                        .map_err(error)?,
                );
            }
        }
        if cancelled() {
            return Err(error("Audio preparation cancelled"));
        }
        let mut next = self.clone();
        next.stretched_audio = prepared;
        samplers(&mut next)?;
        self.sampler_audio = next.sampler_audio;
        self.stretched_audio = next.stretched_audio;
        self.audio = next.audio;
        Ok(())
    }
}
