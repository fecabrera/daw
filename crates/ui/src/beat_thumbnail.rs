//! Immutable beat overview levels. Conversion is shared by clips using one source.
use daw_core::{BeatPattern, Id};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy)]
pub(super) struct Note {
    pub start_tick: u32,
    pub length_ticks: u32,
    pub row: usize,
}
pub(super) struct Level {
    pub notes: Vec<Note>,
    pub max_length: u32,
    pub max_row: usize,
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
                .map(|(row, id)| (*id, row))
                .collect();
            let notes: Vec<_> = pattern
                .visible_steps()
                .filter_map(|step| {
                    Some(Note {
                        start_tick: step.start_tick,
                        length_ticks: pattern.resolution.step_ticks(),
                        row: *index.get(&step.subtrack_id)?,
                    })
                })
                .collect();
            let mut levels = Vec::new();
            let mut bin = pattern.resolution.step_ticks();
            let mut current = notes;
            loop {
                current.sort_by_key(|n| (n.start_tick, n.row));
                levels.push(Arc::new(Level {
                    notes: current.clone(),
                    max_length: bin,
                    max_row: rows.len().saturating_sub(1).max(12),
                }));
                if bin >= pattern.length_ticks() {
                    break;
                }
                bin = bin.saturating_mul(2);
                let mut bins: HashMap<_, Note> = HashMap::new();
                for note in current {
                    bins.entry((note.row, note.start_tick / bin))
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
        assert_eq!(full.notes.len(), 2);
        assert_eq!(full.notes[0].row, 0);
        assert_eq!(full.notes[1].row, 1);
        assert!(Arc::ptr_eq(
            &full,
            &cache.level((project, 0), source, &[row, other], &pattern, 2.0)
        ));
        assert_eq!(cache.builds, 1);
        let swapped = cache.level((project, 0), source, &[other, row], &pattern, 2.0);
        assert_eq!(swapped.notes[0].row, 1);
        cache.level((project, 1), source, &[other, row], &pattern, 0.01);
        cache.level((Id::new_v4(), 1), source, &[other, row], &pattern, 0.01);
        assert_eq!(cache.builds, 4);
    }
    #[test]
    fn full_row_indices_remain_distinct_at_every_detail_level() {
        let rows: Vec<_> = (0..200).map(|_| Id::new_v4()).collect();
        let pattern = BeatPattern::default()
            .with_step(rows[0], 0, Some(100))
            .unwrap()
            .with_step(rows[60], 0, Some(100))
            .unwrap()
            .with_step(rows[199], 0, Some(100))
            .unwrap();
        let mut cache = Cache::default();
        let generation = (Id::new_v4(), 0);
        let source = Id::new_v4();
        for pixels in [2.0, 0.01] {
            let level = cache.level(generation, source, &rows, &pattern, pixels);
            assert_eq!(
                level.notes.iter().map(|note| note.row).collect::<Vec<_>>(),
                vec![0, 60, 199]
            );
            assert_eq!(level.max_row, 199);
        }
    }
}
