use super::{ModulationSourceId, ModulationValue};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

/// Index of a prepared publication buffer. Rebuild when source identities/order change;
/// sample values, ranges and contribution weights may change without rebuilding.
#[derive(Debug)]
pub struct ModulationSourceIndex {
    pub(super) generation: u64,
    positions: HashMap<ModulationSourceId, usize>,
}

impl ModulationSourceIndex {
    pub fn new(sources: impl IntoIterator<Item = ModulationSourceId>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut positions = HashMap::new();
        for (i, id) in sources.into_iter().enumerate() {
            positions.entry(id).or_insert(i);
        }
        Self {
            generation: NEXT.fetch_add(1, Ordering::Relaxed),
            positions,
        }
    }
    pub fn from_values(values: &[ModulationValue]) -> Self {
        Self::new(values.iter().map(|v| v.source))
    }
    pub(super) fn position(&self, source: ModulationSourceId) -> Option<usize> {
        self.positions.get(&source).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    #[test]
    fn prepared_sources_follow_reorders_local_precedence_and_changed_assignments() {
        let a = ModulationSourceId {
            layer: uuid::Uuid::new_v4(),
            unit: uuid::Uuid::new_v4(),
        };
        let b = ModulationSourceId {
            unit: uuid::Uuid::new_v4(),
            ..a
        };
        let value = |source, value| ModulationValue {
            source,
            value,
            weight: 1.0,
            release_age: None,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
        };
        let mut routes = [ModulationRoute::new(
            Modulation {
                source: Some(a),
                amount: 1.0,
                ..Modulation::new(ModulationParameter::Level)
            },
            48000,
        )];
        let shared = [value(a, 0.2), value(b, 0.7)];
        let local = [value(a, 0.8)];
        let shared_index = ModulationSourceIndex::from_values(&shared);
        let local_index = ModulationSourceIndex::from_values(&local);
        let read = |routes: &mut [ModulationRoute],
                    local: &[ModulationValue],
                    shared: &[ModulationValue],
                    context| {
            ModulationInputs::resolve(
                routes,
                &[ModulationParameter::Level],
                local,
                shared,
                context,
            )
            .offset(ModulationParameter::Level)
        };
        assert_eq!(
            read(
                &mut routes,
                &local,
                &shared,
                ModulationContext {
                    local_sources: Some(&local_index),
                    shared_sources: Some(&shared_index),
                    ..Default::default()
                }
            ),
            0.8
        );
        let reordered = [shared[1], shared[0]];
        let index = ModulationSourceIndex::from_values(&reordered);
        assert_eq!(
            read(
                &mut routes,
                &[],
                &reordered,
                ModulationContext {
                    shared_sources: Some(&index),
                    ..Default::default()
                }
            ),
            0.2
        );
        routes[0].settings.source = Some(b);
        assert_eq!(
            read(
                &mut routes,
                &[],
                &reordered,
                ModulationContext {
                    shared_sources: Some(&index),
                    ..Default::default()
                }
            ),
            0.7
        );
        // Slice-only clients may change topology between calls too.
        routes[0].settings.source = Some(a);
        assert_eq!(read(&mut routes, &[], &shared, Default::default()), 0.2);
        assert_eq!(
            read(&mut routes, &local, &reordered, Default::default()),
            0.8
        );
    }
}
