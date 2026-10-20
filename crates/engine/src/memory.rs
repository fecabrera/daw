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
        let mut a = self.project.clone();
        let mut b = other.project.clone();
        a.transport.playhead_frame = 0;
        b.transport.playhead_frame = 0;
        for track in &mut a.tracks {
            track.group_collapsed = false;
        }
        for track in &mut b.tracks {
            track.group_collapsed = false;
        }
        a == b
            && self.audio.len() == other.audio.len()
            && self.audio.iter().all(|(id, a)| {
                other.audio.get(id).is_some_and(|b| {
                    a.sample_rate_hz == b.sample_rate_hz
                        && std::sync::Arc::ptr_eq(&a.samples, &b.samples)
                })
            })
            && self.stretched_audio.len() == other.stretched_audio.len()
            && self.stretched_audio.iter().all(|(id, a)| {
                other.stretched_audio.get(id).is_some_and(|b| {
                    a.audio.sample_rate_hz == b.audio.sample_rate_hz
                        && std::sync::Arc::ptr_eq(&a.audio.samples, &b.audio.samples)
                })
            })
            && self.sampler_audio.len() == other.sampler_audio.len()
            && self.sampler_audio.iter().all(|(id, a)| {
                other.sampler_audio.get(id).is_some_and(|b| {
                    a.variants().count() == b.variants().count()
                        && a.variants().zip(b.variants()).all(|((ka, a), (kb, b))| {
                            ka == kb
                                && a.sample_rate_hz == b.sample_rate_hz
                                && std::sync::Arc::ptr_eq(&a.samples, &b.samples)
                        })
                })
            })
    }

    pub fn model_bytes(&self) -> u64 {
        prepared_model_bytes(
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
        let mut usage = RendererMemory {
            model_bytes: self.plan.model_bytes(),
            schedule_bytes: vec_bytes(&self.audio_tracks)
                + self
                    .audio_tracks
                    .iter()
                    .map(|t| t.heap_bytes())
                    .sum::<u64>(),
            processor_bytes: std::mem::size_of::<Self>() as u64
                + map_bytes(&self.track_index)
                + map_bytes(&self.effect_tracks)
                + vec_bytes(&self.gains)
                + vec_bytes(&self.effects)
                + self.effects.iter().map(|e| e.heap_bytes()).sum::<u64>()
                + self.master_effects.heap_bytes()
                + self.alignment.heap_bytes()
                + self.buses.heap_bytes()
                + vec_bytes(&self.midi),
            shared_audio_bytes: shared.bytes,
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
            project: Project::default(),
            audio: Default::default(),
            stretched_audio: Default::default(),
            sampler_audio: Default::default(),
        };
        let id = Id::new_v4();
        a.audio.insert(
            id,
            AudioData {
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
