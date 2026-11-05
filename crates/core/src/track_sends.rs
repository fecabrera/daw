//! Saved send routes and source/Return audibility. Rendering owns prepared edges.
use crate::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendTap {
    PreFader,
    #[default]
    PostFader,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackSend {
    pub id: Id,
    pub return_id: Id,
    pub enabled: bool,
    pub gain_db: f32,
    pub tap: SendTap,
}
impl TrackSend {
    pub fn new(return_id: Id) -> Self {
        Self {
            id: Id::new_v4(),
            return_id,
            enabled: true,
            gain_db: SILENT_GAIN_DB,
            tap: SendTap::PostFader,
        }
    }
}

impl Project {
    pub fn add_return_track(&mut self) -> Result<Id> {
        let id = Id::new_v4();
        let mut number = 1;
        while self
            .tracks
            .iter()
            .any(|t| t.name == format!("Return {number}"))
        {
            number += 1;
        }
        self.edit(Edit::AddReturn {
            track_id: id,
            name: format!("Return {number}"),
        })?;
        Ok(id)
    }
    pub(crate) fn insert_return(&mut self, id: Id, name: String) -> Result<()> {
        self.add_track()?;
        let t = self.tracks.last_mut().unwrap();
        t.id = id;
        t.name = clip_name(&name)?.to_owned();
        t.kind = TrackKind::Return;
        Ok(())
    }
    pub(crate) fn validate_sends(&self) -> Result<()> {
        let returns: HashSet<_> = self
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Return)
            .map(|t| t.id)
            .collect();
        for t in &self.tracks {
            if t.kind == TrackKind::Return
                && (t.id.is_nil()
                    || t.group_id.is_some()
                    || !t.clips.is_empty()
                    || t.instrument.is_some()
                    || t.beat.is_some()
                    || !t.sends.is_empty())
            {
                return Err(Error(
                    "A Return must be a root bus without clips, instruments, children or sends"
                        .into(),
                ));
            }
            let mut targets = HashSet::new();
            for send in &t.sends {
                if send.id.is_nil()
                    || !returns.contains(&send.return_id)
                    || !targets.insert(send.return_id)
                    || !send.gain_db.is_finite()
                    || !(SILENT_GAIN_DB..=12.0).contains(&send.gain_db)
                {
                    return Err(Error(
                        "Invalid send identity, Return, duplicate destination or gain".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    /// Control-thread masks. Mute is deliberately excluded from send eligibility.
    pub fn track_send_audible(&self) -> Vec<bool> {
        let local = self.source_routing_gates(true);
        let index: HashMap<_, _> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        self.tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if t.kind == TrackKind::Return {
                    return false;
                }
                let mut at = Some(i);
                for _ in 0..=self.tracks.len() {
                    let Some(i) = at else {
                        return true;
                    };
                    if !local[i] {
                        return false;
                    }
                    at = self.tracks[i]
                        .group_id
                        .and_then(|id| index.get(&id).copied());
                }
                false
            })
            .collect()
    }
    pub(crate) fn source_routing_gates(&self, ignore_mute: bool) -> Vec<bool> {
        let index: HashMap<_, _> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let mut solo: Vec<_> = self
            .tracks
            .iter()
            .map(|t| t.kind != TrackKind::Return && t.soloed)
            .collect();
        for (i, t) in self.tracks.iter().enumerate().rev() {
            if let Some(parent) = t.group_id.and_then(|id| index.get(&id)) {
                solo[*parent] |= solo[i];
            }
        }
        let mut siblings = HashMap::new();
        for (i, t) in self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind != TrackKind::Return)
        {
            *siblings.entry(t.group_id).or_insert(false) |= solo[i];
        }
        self.tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if t.kind == TrackKind::Return {
                    true
                } else {
                    mix_audible(!ignore_mute && t.muted, solo[i], siblings[&t.group_id])
                }
            })
            .collect()
    }
    pub(crate) fn routing_mix_gates(&self) -> Vec<bool> {
        let mut gates = self.source_routing_gates(false);
        let return_solo = self
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Return && t.soloed);
        let source_solo = self
            .tracks
            .iter()
            .any(|t| t.kind != TrackKind::Return && t.soloed);
        for (i, t) in self.tracks.iter().enumerate() {
            if t.kind == TrackKind::Return {
                gates[i] = mix_audible(t.muted, t.soloed, return_solo);
            } else if t.group_id.is_none() && return_solo && !source_solo {
                gates[i] = false;
            }
        }
        gates
    }
}
