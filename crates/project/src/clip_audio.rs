//! Worker-side preparation and shared lookup of complete clip audio variants.
use crate::{Result, Session, error};
use daw_core::{Clip, Project};
use daw_media::ClipAudioKey;
use std::collections::HashMap;

pub(super) fn requests(project: &Project) -> impl Iterator<Item = ClipAudioKey> + '_ {
    let key = |clip: &Clip| {
        ClipAudioKey::for_clip(clip).or_else(|| {
            project
                .compound_window_requires_preparation(clip)
                .then(|| ClipAudioKey::unity(clip.asset_id))
        })
    };
    project
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .filter_map(key)
        .chain(project.compound_members().filter_map(key))
        .chain(project.tracks.iter().flat_map(move |t| {
            t.clips.iter().flat_map(move |c| {
                let mut variants = Vec::new();
                if let Some(r) = project
                    .audio_asset(c.asset_id)
                    .and_then(|a| a.rendered_audio.as_ref())
                {
                    for id in r.dependencies() {
                        let mut leaf = c.clone();
                        leaf.asset_id = id;
                        if let Some(k) = key(&leaf) {
                            variants.push(k);
                        }
                    }
                }
                variants
            })
        }))
        .chain(
            project
                .samplers()
                .filter_map(|sampler| sampler.input.as_ref())
                .filter_map(move |input| key(&Clip::from(input))),
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
        if project.assets.iter().any(|asset| {
            asset.compound_audio.is_some()
                && (!self.project.assets.iter().any(|old| old == asset)
                    || !self.compound_views.contains_key(&asset.id))
        }) {
            return true;
        }
        if project.assets.iter().any(|asset| {
            asset.is_file_audio()
                && !self.audio.contains_key(&asset.id)
                && !self.missing_audio.contains(&asset.id)
        }) {
            return true;
        }
        if requests(project).any(|key| {
            project
                .audio_asset(key.asset_id)
                .is_some_and(|a| a.compound_audio.is_some())
                && self.audio.get(&key.asset_id).is_none_or(|source| {
                    self.stretched_audio.get(&key).is_none_or(|prepared| {
                        !key.matches(prepared, source, project.stretch_engine)
                    })
                })
        }) {
            return true;
        }
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
    /// Replace prepared maps and source availability for the matching editor revision.
    /// The caller retains its model, save location, warnings and history.
    pub fn refresh_prepared_audio(&mut self, prepared: &Self) {
        self.audio = prepared.audio.clone();
        self.missing_audio = prepared.missing_audio.clone();
        self.stretched_audio = prepared.stretched_audio.clone();
        self.sampler_audio = prepared.sampler_audio.clone();
        self.compound_views = prepared.compound_views.clone();
    }

    /// MIDI and metadata changes can publish before their audio is ready.
    /// Source imports, audio transforms, Sampler settings and rate changes retain their transactions.
    pub fn background_preparation_compatible(&self, project: &Project) -> bool {
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
            .filter(|a| a.is_audio())
            .eq(project.assets.iter().filter(|a| a.is_audio()))
        {
            return false;
        }
        before.tracks.iter().zip(&project.tracks).all(|(old, new)| {
            old.id == new.id
                && track_samplers(old).eq(track_samplers(new))
                && old
                    .clips
                    .iter()
                    .filter(|clip| {
                        before
                            .assets
                            .iter()
                            .any(|a| a.id == clip.asset_id && a.is_audio())
                    })
                    .map(audio_mapping)
                    .eq(new
                        .clips
                        .iter()
                        .filter(|clip| {
                            project
                                .assets
                                .iter()
                                .any(|a| a.id == clip.asset_id && a.is_audio())
                        })
                        .map(audio_mapping))
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
        if cancelled() {
            return Err(error("Audio preparation cancelled"));
        }
        let mut candidate = self.clone();
        candidate.prepare_stretches_in_place(cancelled, samplers)?;
        self.audio = candidate.audio;
        self.stretched_audio = candidate.stretched_audio;
        self.sampler_audio = candidate.sampler_audio;
        self.compound_views = candidate.compound_views;
        Ok(())
    }
    fn prepare_stretches_in_place(
        &mut self,
        cancelled: &impl Fn() -> bool,
        samplers: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        self.project.validate().map_err(error)?;
        self.prepare_compound_sources(cancelled)?;
        for track in &self.project.tracks {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            self.project
                .compile_audio_track(track, |id| self.audio.contains_key(&id))
                .map_err(error)?;
        }
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
        self.stretched_audio = prepared;
        samplers(self)?;
        self.plan()
            .compound_schedule_bytes(cancelled)
            .map_err(error)?;
        Ok(())
    }
}

/// Names, colors, level and mute do not change prepared PCM or its source mapping.
fn audio_mapping(clip: &Clip) -> impl PartialEq + use<> {
    (
        clip.id,
        clip.asset_id,
        clip.start_frame,
        clip.source_offset_frame,
        clip.length_frames,
        clip.repeat,
        clip.stretch,
        clip.transpose,
    )
}

fn track_samplers(track: &daw_core::Track) -> impl Iterator<Item = &daw_core::SamplerSettings> {
    track
        .instrument
        .iter()
        .chain(
            track
                .beat
                .iter()
                .flat_map(|b| b.subtracks.iter().map(|r| &r.instrument)),
        )
        .flat_map(|instrument| &instrument.layers)
        .filter_map(|layer| layer.sampler.as_ref())
}
