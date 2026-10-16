//! Control-thread dependency checks for units that both consume and provide modulation.
use super::{Modulation, ModulationSourceId};

/// Provider-first order. Retained, disabled and zero-amount connections still count as edges,
/// so enabling a saved connection cannot introduce a feedback cycle.
pub fn modulation_dependency_order(
    nodes: &[(ModulationSourceId, &[Modulation])],
) -> Option<Vec<usize>> {
    let mut order = Vec::with_capacity(nodes.len());
    while order.len() < nodes.len() {
        let next = nodes.iter().enumerate().position(|(index, (_, routes))| {
            !order.contains(&index)
                && routes.iter().all(|route| {
                    route.source.is_none_or(|source| {
                        nodes
                            .iter()
                            .position(|(id, _)| *id == source)
                            .is_none_or(|provider| order.contains(&provider))
                    })
                })
        })?;
        order.push(next);
    }
    Some(order)
}

/// Whether adding source → destination would close a cycle. Used by source pickers.
pub fn modulation_would_cycle(
    nodes: &[(ModulationSourceId, &[Modulation])],
    destination: ModulationSourceId,
    source: ModulationSourceId,
) -> bool {
    let mut pending = vec![source];
    let mut visited = Vec::new();
    while let Some(id) = pending.pop() {
        if id == destination {
            return true;
        }
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        if let Some((_, routes)) = nodes.iter().find(|(node, _)| *node == id) {
            pending.extend(routes.iter().filter_map(|route| route.source));
        }
    }
    false
}
