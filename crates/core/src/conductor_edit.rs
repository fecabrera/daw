//! Frame-range editing for conductor blocks. No clip, source or UI ownership.
use crate::{ArrangementMarker, ClipEdge, Edit, Error, Id, Project, Result, SignatureEvent};
use std::collections::HashSet;

use crate::frame_ranges::uncovered_ranges;
#[derive(Clone)]
struct Range<T> {
    id: Id,
    start: u64,
    end: u64,
    value: T,
}

fn edited<T: Clone>(
    mut ranges: Vec<Range<T>>,
    primary: Id,
    selected: &[Id],
    edge: Option<ClipEdge>,
    delta: i128,
    linked: bool,
    origin: Option<Id>,
) -> Result<Vec<Range<T>>> {
    let ids: HashSet<_> = selected.iter().copied().chain([primary]).collect();
    if selected.iter().collect::<HashSet<_>>().len() != selected.len()
        || ids.iter().any(|id| !ranges.iter().any(|r| r.id == *id))
    {
        return Err(Error("Invalid conductor selection".into()));
    }
    if delta == 0 {
        return Ok(ranges);
    }
    let primary_range = ranges
        .iter()
        .find(|r| r.id == primary)
        .cloned()
        .ok_or_else(|| Error("Conductor block not found".into()))?;
    let neighbor = if linked {
        edge.and_then(|edge| {
            ranges
                .iter()
                .find(|r| {
                    r.id != primary
                        && match edge {
                            ClipEdge::Left => r.end == primary_range.start,
                            ClipEdge::Right => r.start == primary_range.end,
                        }
                })
                .cloned()
        })
    } else {
        None
    };
    let mut candidates = Vec::new();
    let mut removed = HashSet::new();
    let mut pair = Vec::new();
    if let Some(neighbor) = neighbor {
        let (mut left, mut right) = if edge == Some(ClipEdge::Right) {
            (primary_range.clone(), neighbor)
        } else {
            (neighbor, primary_range.clone())
        };
        let boundary = (i128::from(left.end) + delta).clamp(
            i128::from(left.start + u64::from(origin == Some(left.id))),
            i128::from(right.end),
        ) as u64;
        left.end = boundary;
        right.start = boundary;
        for range in [left, right] {
            removed.insert(range.id);
            if range.start < range.end {
                pair.push(range);
            }
        }
    }
    for original in &ranges {
        if !ids.contains(&original.id) || removed.contains(&original.id) {
            continue;
        }
        let mut candidate = original.clone();
        let shifted = |frame: u64| {
            u64::try_from(i128::from(frame) + delta)
                .map_err(|_| Error("Conductor edit exceeds timeline bounds".into()))
        };
        match edge {
            Some(ClipEdge::Left) => candidate.start = shifted(candidate.start)?,
            Some(ClipEdge::Right) => candidate.end = shifted(candidate.end)?,
            None => {
                candidate.start = shifted(candidate.start)?;
                candidate.end = shifted(candidate.end)?;
            }
        }
        if candidate.start >= candidate.end || origin == Some(candidate.id) && candidate.start != 0
        {
            return Err(Error(
                "Conductor edit would empty a selected block or move the origin".into(),
            ));
        }
        removed.insert(candidate.id);
        candidates.push(candidate);
    }
    candidates.sort_by_key(|r| {
        let original = ranges.iter().find(|old| old.id == r.id).unwrap();
        (original.start, original.id)
    });
    if edge == Some(ClipEdge::Left) {
        candidates.reverse();
    }
    ranges.retain(|r| !removed.contains(&r.id));
    ranges.extend(pair);
    // Match clip boundary trims: apply ordinary edits on other selected blocks
    // after updating the contiguous pair.
    let mut winners: Vec<Range<T>> = Vec::new();
    for candidate in candidates {
        // Earlier right-edge resizes and later left-edge resizes take priority.
        let mut pieces = vec![candidate];
        for winner in &winners {
            pieces = subtract(pieces, winner.start, winner.end);
        }
        for piece in pieces {
            ranges = subtract(ranges, piece.start, piece.end);
            winners.push(piece);
        }
    }
    ranges.extend(winners);
    ranges.sort_by_key(|r| (r.start, r.id));
    if let Some(id) = origin
        && ranges.first().is_none_or(|r| r.id != id || r.start != 0)
    {
        return Err(Error("The signature origin must remain at zero".into()));
    }
    Ok(ranges)
}
fn subtract<T: Clone>(ranges: Vec<Range<T>>, start: u64, end: u64) -> Vec<Range<T>> {
    ranges
        .into_iter()
        .flat_map(|r| {
            uncovered_ranges(r.start, r.end, start, end)
                .into_iter()
                .enumerate()
                .map(move |(index, (start, end))| {
                    let mut piece = r.clone();
                    piece.start = start;
                    piece.end = end;
                    if index > 0 {
                        piece.id = Id::new_v4();
                    }
                    piece
                })
        })
        .collect()
}
impl Project {
    pub fn edited_sections(
        &self,
        primary: Id,
        selected: &[Id],
        edge: Option<ClipEdge>,
        delta: i128,
        linked: bool,
    ) -> Result<Vec<ArrangementMarker>> {
        if delta == 0 {
            return Ok(self.markers.clone());
        }
        let clock = self.musical_clock()?;
        let ranges = self
            .markers
            .iter()
            .map(|m| {
                Ok(Range {
                    id: m.id,
                    start: m.start_frame,
                    end: clock.section_end_frame(m)?,
                    value: m.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let markers: Vec<_> = edited(ranges, primary, selected, edge, delta, linked, None)?
            .into_iter()
            .map(|r| ArrangementMarker {
                id: r.id,
                start_frame: r.start,
                length_frames: Some(r.end - r.start),
                ..r.value
            })
            .collect();
        let mut next = self.clone();
        next.edit(Edit::SetMarkers {
            markers: markers.clone(),
        })?;
        Ok(markers)
    }
    /// Signature spans remain continuous, with the final value active indefinitely.
    pub fn edited_signature_blocks(
        &self,
        primary: Id,
        selected: &[Id],
        edge: Option<ClipEdge>,
        delta: i128,
        linked: bool,
    ) -> Result<Vec<SignatureEvent>> {
        let original = self.signature_events();
        if delta == 0 {
            return Ok(original);
        }
        let ids: HashSet<_> = selected.iter().copied().chain([primary]).collect();
        if edge == Some(ClipEdge::Right)
            && let Some(last) = original.last()
            && ids.contains(&last.id)
        {
            let paired_last = linked
                && original
                    .windows(2)
                    .any(|pair| pair[0].id == primary && pair[1].id == last.id);
            if !paired_last {
                return Err(Error("The final signature continues to the end".into()));
            }
        }
        let ranges = original
            .iter()
            .enumerate()
            .map(|(index, event)| Range {
                id: event.id,
                start: event.start_frame,
                end: original
                    .get(index + 1)
                    .map_or(u64::MAX, |next| next.start_frame),
                value: *event,
            })
            .collect();
        // Body drags move the change itself. Signature extents are derived from
        // subsequent changes, so moving the final value has no finite end to move.
        let mut result = edited(
            ranges,
            primary,
            selected,
            edge.or(Some(ClipEdge::Left)),
            delta,
            linked,
            Some(original[0].id),
        )?;
        if edge == Some(ClipEdge::Right) && !linked && delta < 0 {
            // Shrinking a signature pulls its following transition back, keeping
            // the map continuous rather than creating an unlabelled gap.
            for pair in original.windows(2) {
                if ids.contains(&pair[0].id)
                    && let Some(end) = result.iter().find(|r| r.id == pair[0].id).map(|r| r.end)
                    && let Some(next) = result.iter_mut().find(|r| r.id == pair[1].id)
                    && next.start > end
                {
                    next.start = end;
                }
            }
        }
        let mut events: Vec<_> = result
            .into_iter()
            .map(|r| SignatureEvent {
                id: r.id,
                start_frame: r.start,
                ..r.value
            })
            .collect();
        events.sort_by_key(|e| e.start_frame);
        let mut next = self.clone();
        next.edit(Edit::SetSignatureEvents {
            events: events.clone(),
        })?;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeSignature;
    fn sections() -> Project {
        let markers = [(10, 10), (20, 10), (30, 10), (50, 10)]
            .into_iter()
            .map(|(start_frame, length)| ArrangementMarker {
                id: Id::new_v4(),
                start_frame,
                length_frames: Some(length),
                name: format!("Section {start_frame}"),
            })
            .collect();
        Project {
            markers,
            ..Default::default()
        }
    }
    fn ranges(markers: &[ArrangementMarker]) -> Vec<(u64, u64)> {
        markers
            .iter()
            .map(|m| (m.start_frame, m.start_frame + m.length_frames.unwrap()))
            .collect()
    }
    #[test]
    fn section_resizes_trim_and_remove_neighbors_with_directional_group_priority() {
        let p = sections();
        let ids: Vec<_> = p.markers.iter().map(|m| m.id).collect();
        let result = p
            .edited_sections(ids[0], &[ids[0]], Some(ClipEdge::Right), 15, false)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 35), (35, 40), (50, 60)]);
        assert_eq!(result[1].id, ids[2]);
        let result = p
            .edited_sections(ids[2], &[ids[2]], Some(ClipEdge::Left), -15, false)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 15), (15, 40), (50, 60)]);
        assert_eq!(result[0].id, ids[0]);
        let result = p
            .edited_sections(ids[0], &[ids[2], ids[0]], Some(ClipEdge::Right), 15, false)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 35), (35, 55), (55, 60)]);
        assert_eq!(result[1].id, ids[2]);
        assert!(
            p.edited_sections(ids[0], &[ids[0]], Some(ClipEdge::Right), -10, false)
                .is_err()
        );
        assert_eq!(ranges(&p.markers), [(10, 20), (20, 30), (30, 40), (50, 60)]);
    }
    #[test]
    fn section_shared_edges_keep_outer_endpoints_and_remove_empty_sides() {
        let p = sections();
        let ids: Vec<_> = p.markers.iter().map(|m| m.id).collect();
        let result = p
            .edited_sections(ids[0], &[ids[0], ids[1]], Some(ClipEdge::Right), 5, true)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 25), (25, 30), (30, 40), (50, 60)]);
        let result = p
            .edited_sections(ids[0], &[ids[0]], Some(ClipEdge::Right), 100, true)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 30), (30, 40), (50, 60)]);
        assert!(!result.iter().any(|m| m.id == ids[1]));
        let result = p
            .edited_sections(ids[1], &[ids[1]], Some(ClipEdge::Left), -100, true)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 30), (30, 40), (50, 60)]);
        assert_eq!(result[0].id, ids[1]);
        let result = p
            .edited_sections(ids[1], &[ids[1], ids[2]], Some(ClipEdge::Left), -15, true)
            .unwrap();
        assert_eq!(ranges(&result), [(10, 15), (15, 40), (50, 60)]);
        assert_eq!(result[0].id, ids[1]);
        assert_eq!(
            p.edited_sections(ids[0], &[ids[0]], Some(ClipEdge::Right), 0, true)
                .unwrap(),
            p.markers
        );
    }
    #[test]
    fn signature_blocks_preserve_continuity_and_resolve_covered_changes() {
        let mut p = Project::default();
        let mut events = p.signature_events();
        for (start_frame, numerator) in [(20, 3), (40, 5)] {
            events.push(SignatureEvent {
                id: Id::new_v4(),
                start_frame,
                signature: TimeSignature {
                    numerator,
                    denominator: 4,
                },
            });
        }
        p.edit(Edit::SetSignatureEvents {
            events: events.clone(),
        })
        .unwrap();
        let result = p
            .edited_signature_blocks(
                events[0].id,
                &[events[0].id],
                Some(ClipEdge::Right),
                25,
                false,
            )
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].start_frame, 45);
        assert_eq!(result[1].id, events[2].id);
        let result = p
            .edited_signature_blocks(
                events[0].id,
                &[events[0].id],
                Some(ClipEdge::Right),
                -5,
                false,
            )
            .unwrap();
        assert_eq!(result[1].start_frame, 15);
        assert_eq!(result[2], events[2]);
        let result = p
            .edited_signature_blocks(
                events[1].id,
                &[events[1].id],
                Some(ClipEdge::Left),
                -100,
                true,
            )
            .unwrap();
        assert_eq!(result[0].id, events[0].id);
        assert_eq!(result[0].start_frame, 0);
        assert_eq!(result[1].start_frame, 1);
        let result = p
            .edited_signature_blocks(
                events[0].id,
                &[events[0].id],
                Some(ClipEdge::Right),
                100,
                true,
            )
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].id, events[2].id);
        assert_eq!(result[1].start_frame, 40);
        let pair = p
            .edited_signature_blocks(
                events[1].id,
                &[events[1].id, events[2].id],
                Some(ClipEdge::Right),
                5,
                true,
            )
            .unwrap();
        assert_eq!(pair[1].start_frame, 20);
        assert_eq!(pair[2].start_frame, 45);
        let result = p
            .edited_signature_blocks(events[2].id, &[events[2].id], None, 10, false)
            .unwrap();
        assert_eq!(result[2].start_frame, 50);
        p.edit(Edit::SetSignatureEvents { events: result }).unwrap();
        assert_eq!(p.musical_clock().unwrap().signature_at(100).numerator, 5);
        assert!(
            p.edited_signature_blocks(
                events[2].id,
                &[events[2].id],
                Some(ClipEdge::Right),
                -10,
                false
            )
            .is_err()
        );
    }
}
