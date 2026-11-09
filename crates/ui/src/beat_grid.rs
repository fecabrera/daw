//! Presentation lookup for immutable beat sources; hidden hits remain indexed.
use daw_core::{BeatPattern, BeatStep, Id};
use std::collections::HashMap;

/// Find intersecting cells from monotonic project-frame boundaries, including partial cells.
pub(super) fn visible_slots(
    count: u32,
    visible: std::ops::Range<i128>,
    boundary: impl Fn(u32) -> i128,
) -> std::ops::Range<u32> {
    let partition = |after: &dyn Fn(u32) -> bool| {
        let (mut low, mut high) = (0, count);
        while low < high {
            let middle = low + (high - low) / 2;
            if after(middle) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    };
    let first = partition(&|slot| boundary(slot + 1) <= visible.start);
    let last = partition(&|slot| boundary(slot) < visible.end);
    first..last.max(first)
}

/// A visible source cell. Repetitions retain content but cannot edit the source.
pub(super) struct Cell {
    pub clip_index: Option<usize>,
    pub tick: u32,
    pub start_frame: u64,
    pub end_frame: u64,
    pub step_ticks: u32,
    pub editable: bool,
}

#[derive(Default)]
pub(super) struct StepIndex {
    sources: HashMap<Id, HashMap<(Id, u32), BeatStep>>,
    #[cfg(test)]
    pub builds: usize,
}

impl StepIndex {
    pub fn retain(&mut self, assets: impl Iterator<Item = Id>) {
        let assets: std::collections::HashSet<_> = assets.collect();
        self.sources.retain(|id, _| assets.contains(id));
    }

    pub fn prepare(&mut self, asset: Id, pattern: &BeatPattern) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.sources.entry(asset) {
            entry.insert(
                pattern
                    .steps
                    .iter()
                    .map(|step| ((step.subtrack_id, step.start_tick), step.clone()))
                    .collect(),
            );
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
    }

    pub fn get(&self, asset: Id, row: Id, tick: u32) -> Option<&BeatStep> {
        self.sources.get(&asset)?.get(&(row, tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_slots_match_intersections_with_trimmed_stretched_and_repeated_clocks() {
        for origin in [-1000, 0, 3000] {
            for scale in [0.125, 1.0, 1.333, 8.0] {
                let boundary = |slot| origin + (f64::from(slot) * 120.5 * scale).round() as i128;
                for visible in [
                    -2000..-1000,
                    -10..1,
                    0..300,
                    500..1500,
                    1_000_000..1_000_010,
                ] {
                    let expected: Vec<_> = (0..512)
                        .filter(|slot| {
                            boundary(slot + 1) > visible.start && boundary(*slot) < visible.end
                        })
                        .collect();
                    assert_eq!(
                        visible_slots(512, visible, boundary).collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }
}
