//! Prepared source-only input for optional tempo analysis.
use crate::Session;
use daw_core::{Id, Project};
use daw_engine::{CompoundAudioSource, RenderPlan};
use daw_media::{AudioData, ClipAudioKey, Tempo};
use std::collections::HashSet;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

const CATALOG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_INSPECTIONS: usize = 65_536;
static CATALOG_MEMORY: AtomicU64 = AtomicU64::new(0);

struct Admission(u64);

impl Admission {
    fn new(bytes: u64) -> Option<Self> {
        CATALOG_MEMORY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= CATALOG_BYTES)
            })
            .ok()?;
        Some(Self(bytes))
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        CATALOG_MEMORY.fetch_sub(self.0, Ordering::Relaxed);
    }
}

/// Shared immutable owner for compound source preparation and history.
pub struct TempoCatalog {
    plan: RenderPlan,
    _memory: Admission,
}

impl TempoCatalog {
    /// Called on the preparation worker, after the final transform map exists.
    fn build(session: &Session, cancelled: &impl Fn() -> bool) -> Option<Self> {
        let assets = &session.project.assets;
        if assets.len() > MAX_INSPECTIONS || session.compound_views.is_empty() {
            return None;
        }
        let mut inspected = assets.len();
        let mut sources = HashSet::new();
        let mut keys = HashSet::new();
        for asset in assets {
            if cancelled() {
                return None;
            }
            let Some(recipe) = &asset.compound_audio else {
                continue;
            };
            sources.insert(asset.id);
            inspected = inspected.checked_add(recipe.members.len())?;
            if inspected > MAX_INSPECTIONS {
                return None;
            }
            for clip in &recipe.members {
                sources.insert(clip.asset_id);
                keys.insert(
                    ClipAudioKey::for_clip(clip)
                        .unwrap_or_else(|| ClipAudioKey::unity(clip.asset_id)),
                );
            }
        }

        // The scheduler tests captured-loop ownership in reverse. Keep metadata
        // for owners outside the recipe without adding their audio to the input.
        let mut included = Vec::new();
        let mut bytes = 4096;
        for asset in assets {
            if cancelled() {
                return None;
            }
            let mut include = sources.contains(&asset.id);
            if let Some(rendered) = &asset.rendered_audio {
                inspected = inspected
                    .checked_add(1 + 2 * rendered.feeds.len() + rendered.return_feeds.len())?;
                if inspected > MAX_INSPECTIONS {
                    return None;
                }
                include |= rendered.dependencies().any(|id| sources.contains(&id));
            }
            if include {
                // Upper-bound capacity, owned strings, member clips and routing metadata
                // before cloning. Prepared PCM and peaks keep their existing owners.
                bytes += 1024
                    + asset.name.len() as u64
                    + asset.source.path.len() as u64
                    + asset.source.path_kind.len() as u64
                    + asset.source.kind.len() as u64
                    + asset.source_metadata.sample_format.len() as u64;
                if let Some(recipe) = &asset.compound_audio {
                    bytes += recipe
                        .members
                        .iter()
                        .map(|c| 512 + c.name.len() as u64)
                        .sum::<u64>();
                }
                if let Some(r) = &asset.rendered_audio {
                    bytes += 128 * (r.feeds.len() + r.return_feeds.len()) as u64;
                }
                included.push(asset);
            }
        }
        for id in &sources {
            if let Some(audio) = session.audio.get(id) {
                bytes += 512 + audio.metadata.sample_format.len() as u64;
            }
        }
        for key in &keys {
            if let Some(prepared) = session.stretched_audio.get(key) {
                bytes += 512 + prepared.heap_bytes();
            }
        }
        let memory = Admission::new(bytes)?;
        let plan = RenderPlan {
            project: Project {
                sample_rate_hz: session.project.sample_rate_hz,
                stretch_engine: session.project.stretch_engine,
                assets: included.into_iter().cloned().collect(),
                ..Project::default()
            },
            audio: sources
                .into_iter()
                .filter_map(|id| session.audio.get(&id).map(|audio| (id, audio.clone())))
                .collect(),
            stretched_audio: keys
                .into_iter()
                .filter_map(|key| {
                    session
                        .stretched_audio
                        .get(&key)
                        .map(|prepared| (key, prepared.clone()))
                })
                .collect(),
            sampler_audio: Default::default(),
            external: Default::default(),
            beat_tracks: Default::default(),
        };
        Some(Self {
            plan,
            _memory: memory,
        })
    }

    /// Analyze the compound recipe body, with no outer clip or track processing.
    pub fn analyze(
        &self,
        id: Id,
        cancelled: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Tempo, String> {
        let mut source = CompoundAudioSource::for_analysis(&self.plan, id, cancelled.clone())?;
        daw_media::source_tempo(
            self.plan.project.sample_rate_hz,
            source.length_frames,
            &|| cancelled.load(Ordering::Relaxed),
            |start, out| {
                source
                    .read_analysis_frames(start, out)
                    .map_err(daw_media::Error)
            },
        )
        .map_err(|e| e.to_string())
    }
}

pub enum TempoSource<'a> {
    File(&'a AudioData),
    Compound(&'a Arc<TempoCatalog>),
    Pending,
    Missing,
    Unavailable,
}

impl Session {
    pub(crate) fn prepare_tempo_catalog(&mut self, cancelled: &impl Fn() -> bool) {
        self.tempo_catalog = None;
        self.tempo_catalog = TempoCatalog::build(self, cancelled).map(Arc::new);
    }

    pub fn tempo_source(&self, id: Id) -> TempoSource<'_> {
        let Some(asset) = self.project.audio_asset(id) else {
            return TempoSource::Missing;
        };
        if asset.compound_audio.is_some() {
            return self
                .tempo_catalog
                .as_ref()
                .filter(|catalog| catalog.plan.project.audio_asset(id).is_some())
                .map_or(TempoSource::Unavailable, TempoSource::Compound);
        }
        if self.missing_audio.contains(&id) {
            return TempoSource::Missing;
        }
        self.audio
            .get(&id)
            .map_or(TempoSource::Pending, TempoSource::File)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, sync::atomic::AtomicBool};

    fn click_audio() -> AudioData {
        let rate = 48_000;
        let mut samples = vec![[0.01, -0.01]; rate as usize * 2];
        for beat in 0..4 {
            samples[600 + beat * 24_000..680 + beat * 24_000].fill([0.6, -0.6]);
        }
        AudioData {
            sample_rate_hz: rate,
            samples: Arc::new(samples),
            disk: None,
            metadata: daw_core::SourceMetadata {
                sample_rate_hz: rate,
                channels: 2,
                sample_format: "ieee_float".into(),
                bits_per_sample: 32,
            },
            peaks: Arc::default(),
        }
    }

    #[test]
    fn compound_tempo_reads_its_recipe_body_and_rejects_missing_leaves() {
        let mut session = Session::default();
        let original = click_audio();
        let first = session
            .import_decoded(Path::new("first.wav"), original.clone(), None, 0)
            .unwrap();
        let track = session.project.tracks[0].id;
        let second = session
            .import_decoded(Path::new("second.wav"), original, Some(track), 96_000)
            .unwrap();
        let leaf = session.project.tracks[0].clips[0].asset_id;
        let mut reverse_owner = session.project.audio_asset(leaf).unwrap().clone();
        reverse_owner.id = Id::new_v4();
        reverse_owner.source.kind = "rendered_audio".into();
        reverse_owner.rendered_audio = Some(daw_core::RenderedAudio {
            version: 1,
            pre_send_asset: leaf,
            feeds: vec![daw_core::RenderedFeed {
                provider: track,
                raw_asset: leaf,
                post_asset: reverse_owner.id,
            }],
            return_feeds: Vec::new(),
        });
        let owner_id = reverse_owner.id;
        session.project.assets.push(reverse_owner);
        let clips = &mut session.project.tracks[0].clips;
        clips[0].length_frames = 192_000;
        clips[0].repeat = Some(daw_core::ClipLoop {
            length_frames: 96_000,
            phase_frame: 0,
        });
        clips[1].start_frame = 192_000;
        session.project.tracks[0].clips[1].channels.inverted = true;
        session.project.tracks[0].clips[1].gain_db = -3.0;
        let (mut session, merged) = session
            .merged_audio_clips(&[first, second], &|| false)
            .unwrap();
        let root = session.project.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == merged)
            .unwrap()
            .asset_id;
        let TempoSource::Compound(catalog) = session.tempo_source(root) else {
            panic!("Missing prepared catalog")
        };
        assert!(catalog.plan.project.audio_asset(owner_id).is_some());
        assert!(!catalog.plan.audio.contains_key(&owner_id));
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut bounded =
            CompoundAudioSource::for_analysis(&catalog.plan, root, cancelled.clone()).unwrap();
        let mut original = CompoundAudioSource::new(&session.plan(), root).unwrap();

        for start in [0, 512, 95_980, 96_550, 191_950, 192_550, 287_900] {
            let mut block = [[0.0; 2]; 100];
            bounded.read_analysis_frames(start, &mut block).unwrap();
            for (offset, output) in block.iter().enumerate() {
                assert_eq!(*output, original.sample(start + offset as u64).unwrap().0);
            }
        }
        assert!(
            matches!(catalog.analyze(root, &cancelled).unwrap(), Tempo::Estimate { bpm, .. } if (bpm - 120.0).abs() < 1.0)
        );

        let leaf = session
            .project
            .audio_asset(root)
            .unwrap()
            .compound_audio
            .as_ref()
            .unwrap()
            .members[1]
            .asset_id;
        session.audio.remove(&leaf);
        session.prepare_tempo_catalog(&|| false);
        let TempoSource::Compound(catalog) = session.tempo_source(root) else {
            panic!("Missing prepared catalog")
        };
        assert!(
            catalog
                .analyze(root, &cancelled)
                .unwrap_err()
                .starts_with("Missing")
        );
    }
}
