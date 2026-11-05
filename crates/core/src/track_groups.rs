//! Saved bus hierarchy and transactional track-tree edits. Audio owns no model edits.
use crate::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

impl Project {
    /// Append an empty root bus without changing existing group membership.
    pub fn add_group_track(&mut self) -> Result<Id> {
        let mut next = self.clone();
        let number = next
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Group)
            .count()
            + 1;
        let id = next.add_track()?;
        let track = next.tracks.last_mut().unwrap();
        track.kind = TrackKind::Group;
        track.name = format!("Group {number}");
        next.validate()?;
        *self = next;
        Ok(id)
    }

    /// Create a child at the end of a group's subtree as one validated change.
    pub fn add_track_in_group(&mut self, group_id: Id, kind: TrackKind) -> Result<Id> {
        if !self
            .tracks
            .iter()
            .any(|track| track.id == group_id && track.kind == TrackKind::Group)
        {
            return Err(Error("Group not found".into()));
        }
        let mut next = self.clone();
        let index = next.track_subtree(group_id)?.end;
        let id = match kind {
            TrackKind::Audio => next.add_track()?,
            TrackKind::Midi => next.add_midi_track()?,
            TrackKind::Beat => next.add_beat_track()?,
            TrackKind::Group => next.add_group_track()?,
            TrackKind::Return => return Err(Error("Returns cannot belong to groups".into())),
        };
        next.edit(Edit::MoveTrack {
            track_id: id,
            group_id: Some(group_id),
            index,
        })?;
        *self = next;
        Ok(id)
    }

    pub(crate) fn validate_track_groups(&self) -> Result<()> {
        let mut path = Vec::new();
        for track in &self.tracks {
            if let Some(parent) = track.group_id {
                let Some(position) = path.iter().position(|id| *id == parent) else {
                    return Err(Error(
                        "Group children must follow their parent in contiguous order".into(),
                    ));
                };
                path.truncate(position + 1);
            } else {
                path.clear();
            }
            if track.kind == TrackKind::Group {
                if track.id.is_nil() || !track.clips.is_empty() || track.instrument.is_some() {
                    return Err(Error(
                        "A group bus cannot contain clips or an instrument".into(),
                    ));
                }
                path.push(track.id);
            } else if track.group_collapsed {
                return Err(Error("Only a group bus can be collapsed".into()));
            }
        }
        Ok(())
    }

    pub fn track_is_descendant(&self, track_id: Id, group_id: Id) -> bool {
        let mut parent = self
            .tracks
            .iter()
            .find(|t| t.id == track_id)
            .and_then(|t| t.group_id);
        for _ in 0..self.tracks.len() {
            let Some(id) = parent else {
                return false;
            };
            if id == group_id {
                return true;
            }
            parent = self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.group_id);
        }
        false
    }
    pub fn track_depth(&self, track_id: Id) -> usize {
        let mut depth = 0;
        let mut parent = self
            .tracks
            .iter()
            .find(|t| t.id == track_id)
            .and_then(|t| t.group_id);
        while let Some(id) = parent {
            depth += 1;
            if depth >= self.tracks.len() {
                break;
            }
            parent = self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.group_id);
        }
        depth
    }
    /// Presentation only; hidden descendants remain in the model and renderer.
    pub fn visible_tracks(&self) -> Vec<usize> {
        let mut visible = HashMap::new();
        let mut result = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let shown = track
                .group_id
                .is_none_or(|id| *visible.get(&id).unwrap_or(&false));
            if shown {
                result.push(index);
            }
            visible.insert(track.id, shown && !track.group_collapsed);
        }
        result
    }
    pub fn track_subtree(&self, id: Id) -> Result<Range<usize>> {
        let start = self
            .tracks
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| Error("Track not found".into()))?;
        let end = (start + 1..self.tracks.len())
            .find(|&i| !self.track_is_descendant(self.tracks[i].id, id))
            .unwrap_or(self.tracks.len());
        Ok(start..end)
    }
    /// Filter a selection to subtree roots so a child cannot be edited twice.
    pub fn track_selection_roots(&self, selection: &[Id]) -> Vec<Id> {
        let selected: HashSet<_> = selection.iter().copied().collect();
        let mut included = HashMap::new();
        let mut roots = Vec::new();
        for track in &self.tracks {
            let inherited = track
                .group_id
                .is_some_and(|id| *included.get(&id).unwrap_or(&false));
            let own = selected.contains(&track.id);
            if own && !inherited {
                roots.push(track.id);
            }
            included.insert(track.id, own || inherited);
        }
        roots
    }

    /// Local bus gates. A soloed descendant opens its ancestor path; a group's
    /// own solo does not override a child's mute or an explicit sibling solo.
    pub fn track_mix_audible(&self) -> Vec<bool> {
        self.routing_mix_gates()
    }
    pub(crate) fn track_audible(&self, id: Id) -> bool {
        if !self
            .tracks
            .iter()
            .any(|t| matches!(t.kind, TrackKind::Group | TrackKind::Return))
        {
            return self.tracks.iter().find(|t| t.id == id).is_some_and(|t| {
                mix_audible(t.muted, t.soloed, self.tracks.iter().any(|t| t.soloed))
            });
        }
        let gates = self.track_mix_audible();
        let mut current = Some(id);
        for _ in 0..=self.tracks.len() {
            let Some(id) = current else {
                return true;
            };
            let Some(i) = self.tracks.iter().position(|t| t.id == id) else {
                return false;
            };
            if !gates[i] {
                return false;
            }
            current = self.tracks[i].group_id;
        }
        false
    }

    pub(crate) fn group_tracks(
        &mut self,
        selection: &[Id],
        group_id: Id,
        name: String,
    ) -> Result<()> {
        if group_id.is_nil() || self.tracks.iter().any(|t| t.id == group_id) {
            return Err(Error("Invalid group ID".into()));
        }
        if selection.is_empty()
            || selection
                .iter()
                .any(|id| !self.tracks.iter().any(|t| t.id == *id))
        {
            return Err(Error("Select tracks to group".into()));
        }
        if self
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Return && selection.contains(&t.id))
        {
            return Err(Error("Returns cannot be grouped".into()));
        }
        let roots = self.track_selection_roots(selection);
        let first = self.tracks.iter().find(|t| t.id == roots[0]).unwrap();
        let mut parent = first.group_id;
        while let Some(id) = parent {
            if roots.iter().all(|root| self.track_is_descendant(*root, id)) {
                break;
            }
            parent = self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.group_id);
        }
        let members: HashSet<_> = self
            .tracks
            .iter()
            .filter(|t| {
                roots.contains(&t.id)
                    || roots
                        .iter()
                        .any(|root| self.track_is_descendant(t.id, *root))
            })
            .map(|t| t.id)
            .collect();
        let first_index = self
            .tracks
            .iter()
            .position(|t| members.contains(&t.id))
            .unwrap();
        let mut branch = first_index;
        while self.tracks[branch].group_id != parent {
            let id = self.tracks[branch]
                .group_id
                .ok_or_else(|| Error("Invalid group hierarchy".into()))?;
            branch = self
                .tracks
                .iter()
                .position(|t| t.id == id)
                .ok_or_else(|| Error("Group not found".into()))?;
        }
        let insert = self.tracks[..branch]
            .iter()
            .filter(|t| !members.contains(&t.id))
            .count();
        let mut children = Vec::new();
        let mut remaining = Vec::new();
        for mut track in std::mem::take(&mut self.tracks) {
            if members.contains(&track.id) {
                if roots.contains(&track.id) {
                    track.group_id = Some(group_id);
                }
                children.push(track);
            } else {
                remaining.push(track);
            }
        }
        let group = Track {
            sends: vec![],
            id: group_id,
            name: clip_name(&name)?.to_owned(),
            kind: TrackKind::Group,
            midi_options: MidiOptions::default(),
            group_id: parent,
            group_collapsed: false,
            effects: vec![],
            effects_compact: false,
            instrument: None,
            beat: None,
            color: default_track_color(insert),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            clips: vec![],
        };
        remaining.splice(insert..insert, std::iter::once(group).chain(children));
        self.tracks = remaining;
        Ok(())
    }
    pub(crate) fn ungroup_track(&mut self, id: Id) -> Result<()> {
        let i = self
            .tracks
            .iter()
            .position(|t| t.id == id && t.kind == TrackKind::Group)
            .ok_or_else(|| Error("Group not found".into()))?;
        let parent = self.tracks[i].group_id;
        self.tracks.remove(i);
        for track in &mut self.tracks {
            if track.group_id == Some(id) {
                track.group_id = parent;
            }
        }
        Ok(())
    }
    /// Indices of complete selected subtrees, in project order.
    pub fn moving_track_indices(&self, selection: &[Id]) -> Vec<usize> {
        let selected: HashSet<_> = selection.iter().copied().collect();
        let mut included = HashMap::new();
        let mut indices = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let moving = selected.contains(&track.id)
                || track
                    .group_id
                    .is_some_and(|id| *included.get(&id).unwrap_or(&false));
            included.insert(track.id, moving);
            if moving {
                indices.push(index);
            }
        }
        indices
    }
    pub fn can_move_tracks(&self, selection: &[Id], parent: Option<Id>, index: usize) -> bool {
        self.track_move_layout(selection, parent, index).is_ok()
    }
    /// Lightweight preview: original row indices and destination parents only.
    /// The project is in validated preorder; audio and instrument content is never copied.
    pub fn track_move_layout(
        &self,
        selection: &[Id],
        parent: Option<Id>,
        index: usize,
    ) -> Result<Vec<(usize, Option<Id>)>> {
        if self
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Return && selection.contains(&t.id))
        {
            return Err(Error("Reorder Returns within their pinned bank".into()));
        }
        let ids: HashSet<_> = self.tracks.iter().map(|t| t.id).collect();
        if selection.is_empty() || selection.iter().any(|id| !ids.contains(id)) {
            return Err(Error("Select tracks to move".into()));
        }
        let roots: HashSet<_> = self.track_selection_roots(selection).into_iter().collect();
        let moved: HashSet<_> = self.moving_track_indices(selection).into_iter().collect();
        if index > self.tracks.len() - moved.len()
            || parent.is_some_and(|id| {
                !self
                    .tracks
                    .iter()
                    .enumerate()
                    .any(|(i, t)| t.id == id && t.kind == TrackKind::Group && !moved.contains(&i))
            })
        {
            return Err(Error("Invalid group or track drop position".into()));
        }
        let mut children = Vec::new();
        let mut remaining = Vec::new();
        for (i, track) in self.tracks.iter().enumerate() {
            if moved.contains(&i) {
                children.push((
                    i,
                    if roots.contains(&track.id) {
                        parent
                    } else {
                        track.group_id
                    },
                ));
            } else {
                remaining.push((i, track.group_id));
            }
        }
        remaining.splice(index..index, children);
        // Check contiguous parent-before-child order using topology alone.
        let mut path = Vec::new();
        let mut active = HashSet::new();
        for &(i, parent) in &remaining {
            if let Some(parent) = parent {
                if !active.contains(&parent) {
                    return Err(Error("Invalid group or track drop position".into()));
                }
                while path.last() != Some(&parent) {
                    active.remove(&path.pop().unwrap());
                }
            } else {
                path.clear();
                active.clear();
            }
            if self.tracks[i].kind == TrackKind::Group {
                path.push(self.tracks[i].id);
                active.insert(self.tracks[i].id);
            }
        }
        Ok(remaining)
    }
    /// Index is measured after removing all selected subtrees.
    pub fn moved_tracks(&self, selection: &[Id], parent: Option<Id>, index: usize) -> Result<Self> {
        let layout = self.track_move_layout(selection, parent, index)?;
        let mut next = self.clone();
        let mut tracks: Vec<_> = std::mem::take(&mut next.tracks)
            .into_iter()
            .map(Some)
            .collect();
        next.tracks = layout
            .into_iter()
            .map(|(i, parent)| {
                let mut track = tracks[i].take().unwrap();
                track.group_id = parent;
                track
            })
            .collect();
        next.validate()?;
        Ok(next)
    }
    pub fn can_move_track(&self, id: Id, parent: Option<Id>, index: usize) -> bool {
        self.can_move_tracks(&[id], parent, index)
    }
    pub(crate) fn move_track(&mut self, id: Id, parent: Option<Id>, index: usize) -> Result<()> {
        *self = self.moved_tracks(&[id], parent, index)?;
        Ok(())
    }
    pub(crate) fn duplicate_group(&mut self, id: Id, new_id: Id) -> Result<()> {
        let range = self.track_subtree(id)?;
        let ids: HashMap<_, _> = self.tracks[range.clone()]
            .iter()
            .map(|t| (t.id, if t.id == id { new_id } else { Id::new_v4() }))
            .collect();
        let copies: Vec<_> = self.tracks[range.clone()]
            .iter()
            .map(|t| {
                let mut copy = duplicate_track_settings(t, ids[&t.id]);
                copy.group_id = t.group_id.map(|p| *ids.get(&p).unwrap_or(&p));
                copy
            })
            .collect();
        self.tracks.splice(range.end..range.end, copies);
        Ok(())
    }
}

pub(crate) fn duplicate_track_settings(track: &Track, id: Id) -> Track {
    let mut copy = track.clone();
    copy.id = id;
    for send in &mut copy.sends {
        send.id = Id::new_v4();
    }
    copy.clips.clear();
    for s in copy
        .instrument
        .iter_mut()
        .flat_map(|i| &mut i.layers)
        .filter_map(|l| l.sampler.as_mut())
    {
        s.id = Id::new_v4();
    }
    if let Some(beat) = &mut copy.beat {
        beat.id = Id::new_v4();
        let mut remap = HashMap::new();
        for row in &mut beat.subtracks {
            let old = row.id;
            row.id = Id::new_v4();
            remap.insert(old, row.id);
            row.instrument = row.instrument.duplicate();
        }
        for row in &mut beat.subtracks {
            for stop in &mut row.stops {
                *stop = remap[stop];
            }
        }
        let temporary = InstrumentSettings {
            main: beat.output.clone(),
            ..Default::default()
        };
        beat.output = temporary.duplicate().main;
    }
    let mut effect_ids = HashMap::new();
    for effect in copy
        .effects
        .iter_mut()
        .chain(copy.instrument.iter_mut().flat_map(|i| {
            i.layers
                .iter_mut()
                .flat_map(|l| &mut l.effects)
                .chain(&mut i.main.effects)
        }))
    {
        // Route IDs are scoped to their copied owner, as in ordinary track copies.
        let old = effect.id;
        effect.id = Id::new_v4();
        effect_ids.insert(old, effect.id);
    }
    if let Some(i) = &mut copy.instrument {
        for unit in i
            .layers
            .iter_mut()
            .flat_map(|l| &mut l.component_order)
            .chain(&mut i.main.component_order)
        {
            if let InstrumentComponent::Effect(id) = unit
                && let Some(new) = effect_ids.get(id)
            {
                *id = *new;
            }
        }
    }
    copy
}
