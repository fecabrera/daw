//! Worker orchestration for immutable compound sources and sparse waveform clocks.
use crate::{Result, Session, error};
use daw_core::{Clip, Id};
use daw_media::{AudioData, ClipAudioKey};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

impl Session {
    /// Render only the immutable source. Clip and track controls remain outside it.
    pub fn render_compound_source(
        &self,
        id: Id,
        cancelled: &impl Fn() -> bool,
    ) -> Result<AudioData> {
        self.render_compound_window(id, 0, 0, cancelled)
    }
    fn render_compound_window(
        &self,
        id: Id,
        before: u64,
        after: u64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<AudioData> {
        if cancelled() {
            return Err(error("Audio preparation cancelled"));
        }
        let asset = self
            .project
            .audio_asset(id)
            .ok_or_else(|| error("Audio source is missing"))?;
        let frames = asset
            .decoded_frame_count
            .checked_add(before)
            .and_then(|n| n.checked_add(after))
            .ok_or_else(|| error("Compound context length overflow"))?;
        let _reservation = daw_media::AudioReservation::new(frames, 0).map_err(error)?;
        let mut source = daw_engine::CompoundAudioSource::new(&self.plan(), id).map_err(error)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(usize::try_from(frames).map_err(error)?)
            .map_err(error)?;
        for frame in 0..frames {
            if frame.is_multiple_of(1024) && cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            samples.push(
                source
                    .sample_signed(i128::from(frame) - i128::from(before))
                    .map_err(error)?
                    .0,
            );
        }
        let peaks = daw_media::prepare_peaks(&samples, cancelled).map_err(error)?;
        let audio = AudioData {
            disk: None,
            samples: Arc::new(samples),
            peaks: Arc::new(peaks),
            sample_rate_hz: self.project.sample_rate_hz,
            metadata: asset.source_metadata.clone(),
        };
        daw_media::register_audio(&audio);
        Ok(audio)
    }
    /// Borrow PCM or prepare an admitted compound extraction on the caller's worker.
    pub fn render_clip_source(
        &self,
        clip: &Clip,
        cancelled: &impl Fn() -> bool,
    ) -> Result<AudioData> {
        if let Some(audio) = self.audio_for_clip(clip) {
            return Ok(audio.clone());
        }
        if self
            .project
            .audio_asset(clip.asset_id)
            .is_some_and(|a| a.compound_audio.is_some())
        {
            let source = self.render_compound_source(clip.asset_id, cancelled)?;
            return match ClipAudioKey::for_clip(clip) {
                Some(key) => Ok(key
                    .prepare(&source, self.project.stretch_engine, cancelled)
                    .map_err(error)?
                    .audio),
                None => Ok(source),
            };
        }
        Err(error("Prepared clip audio is missing"))
    }
    /// Topological preparation: child transforms exist before a parent needs their samples.
    pub(super) fn prepare_compound_sources(&mut self, cancelled: &impl Fn() -> bool) -> Result<()> {
        if !self
            .project
            .assets
            .iter()
            .any(|a| a.compound_audio.is_some())
        {
            self.compound_views.clear();
            return Ok(());
        }
        let model_bytes = self
            .project
            .assets
            .iter()
            .filter_map(|a| a.compound_audio.as_ref())
            .try_fold(0u64, |sum, s| sum.checked_add(s.metadata_bytes()?))
            .ok_or_else(|| error("Compound metadata overflow"))?;
        let old_bytes = self.compound_view_bytes();
        if model_bytes.saturating_mul(2).saturating_add(old_bytes)
            > daw_core::COMPOUND_METADATA_LIMIT
        {
            return Err(error(
                "Compound preparation exceeds the 128 MiB metadata budget",
            ));
        }
        let mut reachable = HashSet::new();
        let mut pending: Vec<_> = self
            .project
            .assets
            .iter()
            .filter(|a| a.compound_audio.is_some())
            .map(|a| a.id)
            .collect();
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            let asset = self
                .project
                .audio_asset(id)
                .ok_or_else(|| error("Compound dependency is missing"))?;
            if let Some(recipe) = &asset.compound_audio {
                pending.extend(recipe.members.iter().map(|m| m.asset_id));
            } else if !self.audio.contains_key(&id) && !self.missing_audio.contains(&id) {
                return Err(error("Wait for compound source audio preparation"));
            }
        }
        let keys: Vec<_> = super::clip_audio::requests(&self.project).collect();
        let mut needed: HashSet<_> = keys.iter().map(|key| key.asset_id).collect();
        needed.extend(
            self.project
                .samplers()
                .filter_map(|s| s.input.as_ref())
                .map(|i| i.asset_id),
        );
        let mut order = Vec::new();
        let mut done = HashSet::new();
        for asset in &self.project.assets {
            let mut stack = vec![(asset.id, false)];
            while let Some((id, leaving)) = stack.pop() {
                if done.contains(&id) {
                    continue;
                }
                if cancelled() {
                    return Err(error("Audio preparation cancelled"));
                }
                if leaving {
                    done.insert(id);
                    order.push(id);
                    continue;
                }
                stack.push((id, true));
                if let Some(source) = self
                    .project
                    .audio_asset(id)
                    .and_then(|a| a.compound_audio.as_ref())
                {
                    stack.extend(source.members.iter().map(|m| (m.asset_id, false)));
                }
            }
        }
        // Compiled recipes are immutable, but their transformed leaves can change with the engine/rate.
        // Rebuild disposable source PCM on a source job; never reuse stale parent samples.
        self.audio.retain(|id, _| {
            !self
                .project
                .audio_asset(*id)
                .is_some_and(|a| a.compound_audio.is_some())
        });
        for id in order {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            let compound = self
                .project
                .audio_asset(id)
                .is_some_and(|a| a.compound_audio.is_some());
            if compound && needed.contains(&id) {
                let audio = self.render_compound_source(id, cancelled)?;
                self.audio.insert(id, audio);
            }
            for key in keys.iter().filter(|key| key.asset_id == id) {
                let Some(source) = self.audio.get(&id) else {
                    continue;
                };
                if self
                    .stretched_audio
                    .get(key)
                    .is_some_and(|p| key.matches(p, source, self.project.stretch_engine))
                {
                    continue;
                }
                let prepared = if compound {
                    let left = self.project.compound_edge_extension(id, true);
                    let right = self.project.compound_edge_extension(id, false);
                    if left > 0 || right > 0 {
                        let context = self.render_compound_window(id, left, right, cancelled)?;
                        key.prepare(&context, self.project.stretch_engine, cancelled)
                            .map_err(error)?
                            .with_source_context(source, left, cancelled)
                            .map_err(error)?
                    } else {
                        key.prepare(source, self.project.stretch_engine, cancelled)
                            .map_err(error)?
                    }
                } else {
                    key.prepare(source, self.project.stretch_engine, cancelled)
                        .map_err(error)?
                };
                self.stretched_audio.insert(*key, prepared);
            }
        }

        let mut views = HashMap::new();
        let mut bytes = 0u64;
        for asset in self
            .project
            .assets
            .iter()
            .filter(|a| a.compound_audio.is_some())
        {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            let mut local = self.project.tracks.first().cloned().unwrap_or_else(|| {
                let mut p = daw_core::Project::default();
                p.add_track().unwrap();
                p.tracks.remove(0)
            });
            local.clips = asset.compound_audio.as_ref().unwrap().members.clone();
            local.effects.clear();
            local.sends = Vec::new();
            local.name.clear();
            local.instrument = None;
            local.beat = None;
            let view = self
                .project
                .compile_audio_track_view_cancellable(
                    &local,
                    |id| self.audio.contains_key(&id),
                    cancelled,
                )
                .map_err(error)?;
            bytes = bytes
                .saturating_add(view.metadata_bytes())
                .saturating_add(std::mem::size_of::<daw_core::CompiledAudioTrack>() as u64 + 64);
            if bytes > daw_core::COMPOUND_METADATA_LIMIT / 4 {
                return Err(error("Compound waveform metadata admission exceeded"));
            }
            views.insert(asset.id, Arc::new(view));
        }
        let mut compiled_bytes = 0u64;
        for track in &self.project.tracks {
            if !track.clips.iter().any(|c| {
                self.project
                    .audio_asset(c.asset_id)
                    .is_some_and(|a| a.compound_audio.is_some())
            }) {
                continue;
            }
            let view = self
                .project
                .compile_audio_track_view_cancellable(
                    track,
                    |id| self.audio.contains_key(&id),
                    cancelled,
                )
                .map_err(error)?;
            compiled_bytes = compiled_bytes.saturating_add(view.metadata_bytes());
        }
        if model_bytes
            .saturating_mul(2)
            .saturating_add(old_bytes)
            .saturating_add(bytes)
            .saturating_add(compiled_bytes.saturating_mul(2))
            > daw_core::COMPOUND_METADATA_LIMIT
        {
            return Err(error(
                "Compound preparation exceeds the 128 MiB metadata budget",
            ));
        }
        self.compound_views = views;
        Ok(())
    }
    pub(crate) fn compound_view_bytes(&self) -> u64 {
        (self.compound_views.capacity()
            * (std::mem::size_of::<Id>()
                + std::mem::size_of::<Arc<daw_core::CompiledAudioTrack>>()
                + 1)) as u64
            + self
                .compound_views
                .values()
                .map(|v| {
                    v.metadata_bytes()
                        + std::mem::size_of::<daw_core::CompiledAudioTrack>() as u64
                        + 16
                })
                .sum::<u64>()
    }
}
