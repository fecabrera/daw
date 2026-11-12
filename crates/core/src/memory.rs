//! Owned project metadata estimates; immutable media is accounted separately.
use crate::Project;
use daw_unit_core::memory::{effect_settings_bytes, vec_bytes};
impl Project {
    pub fn memory_bytes(&self) -> u64 {
        std::mem::size_of::<Self>() as u64
            + self.name.capacity() as u64
            + effect_settings_bytes(&self.master.effects)
            + vec_bytes(&self.assets)
            + self
                .assets
                .iter()
                .map(|a| {
                    a.name.capacity() as u64
                        + a.source.kind.capacity() as u64
                        + a.source.path.capacity() as u64
                        + a.source.path_kind.capacity() as u64
                        + a.source_metadata.sample_format.capacity() as u64
                        + a.compound_audio
                            .as_ref()
                            .map_or(0, |s| s.metadata_bytes().unwrap_or(u64::MAX))
                        + a.midi_pattern.as_ref().map_or(0, |p| vec_bytes(&p.notes))
                        + a.beat_pattern
                            .as_ref()
                            .map_or(0, |s| s.pattern.heap_bytes())
                })
                .sum::<u64>()
            + vec_bytes(&self.tracks)
            + self
                .tracks
                .iter()
                .map(|t| {
                    t.name.capacity() as u64
                        + vec_bytes(&t.sends)
                        + vec_bytes(&t.clips)
                        + t.clips
                            .iter()
                            .map(|c| c.name.capacity() as u64)
                            .sum::<u64>()
                        + effect_settings_bytes(&t.effects)
                        + t.instrument.as_ref().map_or(0, |i| i.heap_bytes())
                        + t.beat.as_ref().map_or(0, |b| {
                            std::mem::size_of::<crate::BeatInstrument>() as u64 + b.heap_bytes()
                        })
                })
                .sum::<u64>()
    }
}
