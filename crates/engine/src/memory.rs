//! Worker-side renderer accounting. Shared media is counted separately from clone cost.
use super::*;
use daw_unit_core::memory::{map_bytes, vec_bytes};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RendererMemory {
    pub model_bytes: u64,
    pub schedule_bytes: u64,
    pub processor_bytes: u64,
    pub shared_audio_bytes: u64,
}
impl RendererMemory {
    pub fn owned_bytes(self) -> u64 {
        self.model_bytes
            .saturating_add(self.schedule_bytes)
            .saturating_add(self.processor_bytes)
    }
}
impl RenderPlan {
    /// Strict checkpoint compatibility. Only the requested playhead may differ;
    /// every processing setting and prepared media allocation must match.
    pub fn same_processing_plan(&self, other: &Self) -> bool {
        self.same_processing_plan_with(other, |_| {})
    }

    pub(crate) fn same_processing_plan_with(
        &self,
        other: &Self,
        normalize_other: impl FnOnce(&mut Project),
    ) -> bool {
        self.compare_processing_plan(other, normalize_other, false)
    }

    pub(super) fn same_checkpoint_plan(
        &self,
        other: &Self,
        normalize_other: impl FnOnce(&mut Project),
    ) -> bool {
        self.compare_processing_plan(other, normalize_other, true)
    }

    fn compare_processing_plan(
        &self,
        other: &Self,
        normalize_other: impl FnOnce(&mut Project),
        exact: bool,
    ) -> bool {
        let mut a = self.project.clone();
        let mut b = other.project.clone();
        normalize_other(&mut b);
        a.markers.clear();
        b.markers.clear();
        a.transport.playhead_frame = 0;
        b.transport.playhead_frame = 0;
        for track in &mut a.tracks {
            track.group_collapsed = false;
        }
        for track in &mut b.tracks {
            track.group_collapsed = false;
        }
        let same_audio = |a: &AudioData, b: &AudioData| {
            a.sample_rate_hz == b.sample_rate_hz
                && Arc::ptr_eq(&a.samples, &b.samples)
                && (!exact
                    || a.metadata == b.metadata
                        && match (&a.disk, &b.disk) {
                            (None, None) => true,
                            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                            _ => false,
                        })
        };
        a == b
            // Rust float equality treats signed zero as equal. A borrowed DSP
            // checkpoint requires bit-compatible settings across the append.
            && (!exact || serde_json::to_vec(&a).is_ok_and(|a| serde_json::to_vec(&b).is_ok_and(|b| a == b)))
            && self.beat_tracks == other.beat_tracks
            && self.audio.len() == other.audio.len()
            && self.audio.iter().all(|(id, a)| {
                other.audio.get(id).is_some_and(|b| same_audio(a, b))
            })
            && self.stretched_audio.len() == other.stretched_audio.len()
            && self.stretched_audio.iter().all(|(id, a)| {
                other.stretched_audio.get(id).is_some_and(|b| same_audio(&a.audio, &b.audio))
            })
            && self.sampler_audio.len() == other.sampler_audio.len()
            && self.sampler_audio.iter().all(|(id, a)| {
                other.sampler_audio.get(id).is_some_and(|b| {
                    a.duration_variants().count() == b.duration_variants().count()
                        && a.duration_variants().zip(b.duration_variants()).all(
                            |((la, a), (lb, b))| {
                                la == lb
                                    && (!exact || a.variants().count() == b.variants().count())
                                    && a.variants().zip(b.variants()).all(|((ka, a), (kb, b))| {
                                        ka == kb && if exact { same_audio(a, b) } else { Arc::ptr_eq(&a.samples, &b.samples) }
                                    })
                            },
                        )
                        && a.variants().count() == b.variants().count()
                        && a.variants().zip(b.variants()).all(|((ka, a), (kb, b))| {
                            ka == kb && same_audio(a, b)
                        })
                })
            })
    }

    pub fn model_bytes(&self) -> u64 {
        map_bytes(&self.beat_tracks)
            + self
                .beat_tracks
                .values()
                .map(|b| {
                    vec_bytes(&b.clip_gains)
                        + map_bytes(&b.stops)
                        + b.stops.values().map(vec_bytes).sum::<u64>()
                })
                .sum::<u64>()
            + prepared_model_bytes(
                &self.project,
                &self.audio,
                &self.stretched_audio,
                &self.sampler_audio,
            )
    }

    pub fn visit_audio_memory(&self, usage: &mut daw_media::AudioMemoryUsage) {
        for a in self.audio.values() {
            usage.audio(a);
        }
        for a in self.stretched_audio.values() {
            a.visit_memory(usage);
        }
        for a in self.sampler_audio.values() {
            a.visit_memory(usage);
        }
    }
}
impl Renderer {
    pub fn memory_usage(&self) -> RendererMemory {
        let mut shared = daw_media::AudioMemoryUsage::default();
        self.plan.visit_audio_memory(&mut shared);
        let mut usage = self.owned_memory();
        usage.shared_audio_bytes = shared.bytes + self.plan.external.states.retained_bytes();
        usage
    }
    /// Capacity accounting without a shared-media owner registry or allocation.
    pub fn owned_memory_bytes(&self) -> u64 {
        self.owned_memory().owned_bytes()
    }
    fn owned_memory(&self) -> RendererMemory {
        let mut usage = RendererMemory {
            model_bytes: self.plan.model_bytes(),
            schedule_bytes: vec_bytes(&self.audio_tracks)
                + self
                    .audio_tracks
                    .iter()
                    .map(|t| t.heap_bytes())
                    .sum::<u64>(),
            processor_bytes: self.hosted.as_ref().map_or(0, |g| g.heap_bytes())
                + self.clock.memory_bytes()
                + self.capture_controls.as_ref().map_or(0, |controls| {
                    vec_bytes(controls) + controls.iter().map(vec_bytes).sum::<u64>()
                })
                + std::mem::size_of::<Self>() as u64
                + map_bytes(&self.track_index)
                + map_bytes(&self.effect_tracks)
                + vec_bytes(&self.gains)
                + vec_bytes(&self.effects)
                + self.effects.iter().map(|e| e.heap_bytes()).sum::<u64>()
                + self.master_effects.heap_bytes()
                + self.alignment.heap_bytes()
                + self.buses.heap_bytes()
                + self.sidechains.heap_bytes()
                + vec_bytes(&self.sidechain_post)
                + vec_bytes(&self.midi),
            shared_audio_bytes: 0,
        };
        for m in self.midi.iter().flatten() {
            let (schedules, processors) = m.memory_bytes();
            usage.schedule_bytes += schedules;
            usage.processor_bytes += processors;
        }
        usage
    }
}

pub fn prepared_model_bytes(
    project: &Project,
    audio: &HashMap<Id, AudioData>,
    stretched_audio: &daw_media::ClipAudioCache,
    sampler_audio: &HashMap<Id, daw_media::PreparedSampler>,
) -> u64 {
    project.memory_bytes()
        + map_bytes(audio)
        + audio
            .values()
            .map(|a| a.metadata.sample_format.capacity() as u64)
            .sum::<u64>()
        + map_bytes(stretched_audio)
        + stretched_audio
            .values()
            .map(|s| s.heap_bytes())
            .sum::<u64>()
        + map_bytes(sampler_audio)
        + sampler_audio.values().map(|s| s.heap_bytes()).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_reuse_requires_equal_processing_and_media_identity() {
        let mut a = RenderPlan {
            external: Default::default(),
            beat_tracks: Default::default(),
            project: Project::default(),
            audio: Default::default(),
            stretched_audio: Default::default(),
            sampler_audio: Default::default(),
        };
        let id = Id::new_v4();
        a.audio.insert(
            id,
            AudioData {
                disk: None,
                sample_rate_hz: 48_000,
                samples: std::sync::Arc::new(vec![[0.0; 2]; 4]),
                peaks: Default::default(),
                metadata: daw_core::SourceMetadata {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    bits_per_sample: 32,
                    sample_format: "float".into(),
                },
            },
        );
        let mut b = a.clone();
        b.project.transport.playhead_frame = 123;
        assert!(a.same_processing_plan(&b));
        b.project.master.gain_db = -3.0;
        assert!(!a.same_processing_plan(&b));
        b = a.clone();
        b.audio.get_mut(&id).unwrap().sample_rate_hz = 96_000;
        assert!(!a.same_processing_plan(&b));
        b = a.clone();
        // Equal samples in a new allocation do not prove equal prepared history.
        b.audio.get_mut(&id).unwrap().samples = std::sync::Arc::new(vec![[0.0; 2]; 4]);
        assert!(!a.same_processing_plan(&b));
    }
}
