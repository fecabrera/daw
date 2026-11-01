//! Immutable beat overview levels. Conversion is shared by clips using one source.
use daw_core::{BeatPattern, Id, MidiNote, MidiPattern};
use std::{collections::HashMap, sync::Arc};

pub(super) struct Level {
    pub pattern: MidiPattern,
    pub max_length: u32,
    pub min_pitch: u8,
    pub max_pitch: u8,
}
struct Overview {
    rows: Vec<Id>,
    levels: Vec<Arc<Level>>,
}
#[derive(Default)]
pub(super) struct Cache {
    generation: Option<(Id, u64)>,
    sources: HashMap<Id, Overview>,
    #[cfg(test)]
    pub builds: usize,
}
impl Cache {
    pub fn level(
        &mut self,
        generation: (Id, u64),
        asset: Id,
        rows: &[Id],
        pattern: &BeatPattern,
        pixels_per_step: f32,
    ) -> Arc<Level> {
        if self.generation != Some(generation) {
            self.sources.clear();
            self.generation = Some(generation);
        }
        if self
            .sources
            .get(&asset)
            .is_none_or(|overview| overview.rows != rows)
        {
            let index: HashMap<_, _> = rows
                .iter()
                .enumerate()
                .map(|(row, id)| (*id, 60 + (row % 60) as u8))
                .collect();
            let notes: Vec<_> = pattern
                .visible_steps()
                .filter_map(|step| {
                    Some(MidiNote {
                        id: step.id,
                        start_tick: step.start_tick,
                        length_ticks: pattern.resolution.step_ticks(),
                        pitch: *index.get(&step.subtrack_id)?,
                        velocity: step.velocity,
                    })
                })
                .collect();
            let min = notes.iter().map(|n| n.pitch).min().unwrap_or(60);
            let max = notes
                .iter()
                .map(|n| n.pitch)
                .max()
                .unwrap_or(72)
                .max(min.saturating_add(12));
            let mut levels = Vec::new();
            let mut bin = pattern.resolution.step_ticks();
            let mut current = notes;
            loop {
                current.sort_by_key(|n| (n.start_tick, n.pitch));
                levels.push(Arc::new(Level {
                    pattern: MidiPattern {
                        length_ticks: pattern.length_ticks(),
                        notes: current.clone(),
                    },
                    max_length: bin,
                    min_pitch: min,
                    max_pitch: max,
                }));
                if bin >= pattern.length_ticks() {
                    break;
                }
                bin = bin.saturating_mul(2);
                let mut bins: HashMap<_, MidiNote> = HashMap::new();
                for note in current {
                    bins.entry((note.pitch, note.start_tick / bin))
                        .and_modify(|merged| {
                            let end = (merged.start_tick + merged.length_ticks)
                                .max(note.start_tick + note.length_ticks);
                            merged.start_tick = merged.start_tick.min(note.start_tick);
                            merged.length_ticks = end - merged.start_tick;
                        })
                        .or_insert(note);
                }
                current = bins.into_values().collect();
            }
            self.sources.insert(
                asset,
                Overview {
                    rows: rows.to_vec(),
                    levels,
                },
            );
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        let overview = &self.sources[&asset];
        let mut level = 0;
        let mut pixels = pixels_per_step;
        while pixels < 1.0 && level + 1 < overview.levels.len() {
            pixels *= 2.0;
            level += 1;
        }
        overview.levels[level].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_reuse_sources_and_invalidate_for_order_revision_and_project() {
        let row = Id::new_v4();
        let other = Id::new_v4();
        let source = Id::new_v4();
        let project = Id::new_v4();
        let pattern = BeatPattern::default()
            .with_step(row, 0, Some(100))
            .unwrap()
            .with_step(other, 240, Some(100))
            .unwrap();
        let mut cache = Cache::default();
        let full = cache.level((project, 0), source, &[row, other], &pattern, 2.0);
        assert_eq!(full.pattern.notes.len(), 2);
        assert_eq!(full.pattern.notes[0].pitch, 60);
        assert_eq!(full.pattern.notes[1].pitch, 61);
        assert!(Arc::ptr_eq(
            &full,
            &cache.level((project, 0), source, &[row, other], &pattern, 2.0)
        ));
        assert_eq!(cache.builds, 1);
        let swapped = cache.level((project, 0), source, &[other, row], &pattern, 2.0);
        assert_eq!(swapped.pattern.notes[0].pitch, 61);
        cache.level((project, 1), source, &[other, row], &pattern, 0.01);
        cache.level((Id::new_v4(), 1), source, &[other, row], &pattern, 0.01);
        assert_eq!(cache.builds, 4);
    }
}
