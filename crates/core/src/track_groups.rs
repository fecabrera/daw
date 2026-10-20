//! Saved bus hierarchy and transactional track-tree edits. Audio owns no model edits.
use crate::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

impl Project {
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
        self.tracks
            .iter()
            .filter(|t| {
                selection.contains(&t.id)
                    && !selection
                        .iter()
                        .any(|id| self.track_is_descendant(t.id, *id))
            })
            .map(|t| t.id)
            .collect()
    }
    /// Local bus gates. A soloed descendant opens its ancestor path; a group's
    /// own solo does not override a child's mute or an explicit sibling solo.
    pub fn track_mix_audible(&self) -> Vec<bool> {
        let index: HashMap<_, _> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let mut solo: Vec<_> = self.tracks.iter().map(|t| t.soloed).collect();
        for (i, track) in self.tracks.iter().enumerate().rev() {
            if let Some(parent) = track.group_id.and_then(|id| index.get(&id)) {
                solo[*parent] |= solo[i];
            }
        }
        let mut siblings = HashMap::new();
        for (i, track) in self.tracks.iter().enumerate() {
            *siblings.entry(track.group_id).or_insert(false) |= solo[i];
        }
        self.tracks
            .iter()
            .enumerate()
            .map(|(i, t)| mix_audible(t.muted, solo[i], siblings[&t.group_id]))
            .collect()
    }
    pub(crate) fn track_audible(&self, id: Id) -> bool {
        if !self.tracks.iter().any(|t| t.kind == TrackKind::Group) {
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
            id: group_id,
            name: clip_name(&name)?.to_owned(),
            kind: TrackKind::Group,
            group_id: parent,
            group_collapsed: false,
            effects: vec![],
            effects_compact: false,
            instrument: None,
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
    pub fn can_move_track(&self, id: Id, parent: Option<Id>, index: usize) -> bool {
        let Ok(range) = self.track_subtree(id) else {
            return false;
        };
        if index > self.tracks.len() - range.len() {
            return false;
        }
        if parent.is_some_and(|p| {
            p == id
                || self.track_is_descendant(p, id)
                || !self
                    .tracks
                    .iter()
                    .any(|t| t.id == p && t.kind == TrackKind::Group)
        }) {
            return false;
        }
        let remaining: Vec<_> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| !range.contains(i))
            .map(|(_, t)| t)
            .collect();
        // Insert at a direct-child boundary, including the end of the parent's subtree.
        if let Some(next) = remaining.get(index)
            && next.group_id == parent
        {
            return true;
        }
        if let Some(parent) = parent {
            let Some(p) = remaining.iter().position(|t| t.id == parent) else {
                return false;
            };
            index > p
                && remaining[p + 1..index]
                    .iter()
                    .all(|t| self.track_is_descendant(t.id, parent))
                && remaining
                    .get(index)
                    .is_none_or(|t| !self.track_is_descendant(t.id, parent))
        } else {
            index == remaining.len()
        }
    }
    pub(crate) fn move_track(&mut self, id: Id, parent: Option<Id>, index: usize) -> Result<()> {
        if !self.can_move_track(id, parent, index) {
            return Err(Error("Invalid group or track drop position".into()));
        }
        let range = self.track_subtree(id)?;
        let mut moved: Vec<_> = self.tracks.drain(range).collect();
        moved[0].group_id = parent;
        self.tracks.splice(index..index, moved);
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
    copy.clips.clear();
    for s in copy
        .instrument
        .iter_mut()
        .flat_map(|i| &mut i.layers)
        .filter_map(|l| l.sampler.as_mut())
    {
        s.id = Id::new_v4();
    }
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
        effect.id = Id::new_v4();
    }
    copy
}
