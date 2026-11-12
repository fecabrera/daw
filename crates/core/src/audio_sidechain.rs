//! Shared source eligibility and the combined audio/control dependency graph.
use crate::*;
use std::collections::{HashMap, HashSet};
impl Project {
    /// Saved connection owners. Beat instruments resolve to their parent until compilation.
    pub fn audio_sidechain_routes(&self) -> Vec<(Id, Modulation)> {
        let mut routes = Vec::new();
        for t in &self.tracks {
            routes.extend(
                t.effects
                    .iter()
                    .flat_map(|e| e.unit.modulations())
                    .chain(t.instrument.iter().flat_map(|i| i.modulation_routes()))
                    .filter(|r| r.sidechain.is_some())
                    .map(|r| (t.id, *r)),
            );
            if let Some(beat) = &t.beat {
                routes.extend(
                    beat.output
                        .modulation_routes()
                        .chain(
                            beat.subtracks
                                .iter()
                                .flat_map(|s| s.instrument.modulation_routes()),
                        )
                        .filter(|r| r.sidechain.is_some())
                        .map(|r| (t.id, *r)),
                );
            }
        }
        routes.extend(
            self.master
                .effects
                .iter()
                .flat_map(|e| e.unit.modulations())
                .filter(|r| r.sidechain.is_some())
                .map(|r| (self.project_id, *r)),
        );
        routes
    }
    fn sidechain_edges(&self) -> Vec<(Id, Id)> {
        self.tracks
            .iter()
            .flat_map(|t| {
                t.group_id
                    .into_iter()
                    .map(|p| (t.id, p))
                    .chain(t.sends.iter().map(|s| (t.id, s.return_id)))
            })
            .chain(
                self.audio_sidechain_routes()
                    .into_iter()
                    .filter_map(|(dest, r)| r.sidechain.map(|s| (s.track, dest))),
            )
            .chain(
                self.audio_sidechain_routes()
                    .into_iter()
                    .flat_map(|(dest, route)| {
                        let provider = route.sidechain.unwrap().track;
                        self.tracks
                            .iter()
                            .filter(move |t| {
                                self.tracks.iter().any(|s| s.id == provider)
                                    && self.rendered_providers(t).contains(&provider)
                            })
                            .map(move |t| (t.id, dest))
                    }),
            )
            .collect()
    }
    /// Missing identities remain saved and neutral; existing edges must form an acyclic graph.
    pub fn sidechain_order(&self) -> Result<Vec<usize>> {
        let index: HashMap<_, _> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        if index.len() != self.tracks.len() {
            return Err(Error("Duplicate track ID in audio/control graph".into()));
        }
        let edges = self.sidechain_edges();
        let mut incoming = vec![0usize; index.len()];
        let mut outgoing = vec![Vec::new(); index.len()];
        for (source, dest) in edges {
            if let (Some(&s), Some(&d)) = (index.get(&source), index.get(&dest)) {
                incoming[d] += 1;
                outgoing[s].push(d);
            }
        }
        let mut ids = HashSet::new();
        for (_, route) in self.audio_sidechain_routes() {
            if !route.valid()
                || !ids.insert(route.id)
                || route.sidechain.is_some_and(|s| s.track == self.project_id)
            {
                return Err(Error(
                    "Invalid audio sidechain settings or duplicate connection identity".into(),
                ));
            }
        }
        let mut pending: Vec<_> = incoming
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (*n == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(index.len());
        while let Some(i) = pending.pop() {
            order.push(i);
            for &d in &outgoing[i] {
                incoming[d] -= 1;
                if incoming[d] == 0 {
                    pending.push(d);
                }
            }
        }
        if order.len() != index.len() {
            return Err(Error(
                "Audio sidechain creates an audio/control feedback cycle".into(),
            ));
        }
        Ok(order)
    }
    /// Use this predicate for both source browsing and final transaction validation.
    pub fn sidechain_source_available(&self, destination: Id, source: Id) -> bool {
        if source == destination
            || source == self.project_id
            || !self.tracks.iter().any(|t| t.id == source)
        {
            return false;
        }
        let edges = self.sidechain_edges();
        let mut pending = vec![destination];
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if id == source {
                return false;
            }
            if visited.insert(id) {
                pending.extend(edges.iter().filter(|(s, _)| *s == id).map(|(_, d)| *d));
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn effect(source: Id) -> EffectInstance {
        let mut e = EffectInstance::drive();
        let mut route = Modulation::new(ModulationParameter::Volume);
        route.sidechain = Some(daw_unit_core::AudioSidechain::new(source));
        let EffectSettings::Drive(s) = &mut e.unit else {
            unreachable!()
        };
        s.modulations.push(route);
        e
    }
    #[test]
    fn sidechain_cycles_include_disabled_routes_groups_and_returns_and_edits_are_atomic() {
        let mut p = Project::default();
        let a = p.add_track().unwrap();
        let b = p.add_track().unwrap();
        p.edit(Edit::SetEffects {
            track_id: b,
            effects: vec![effect(a)],
            compact: false,
        })
        .unwrap();
        assert!(!p.sidechain_source_available(a, b));
        assert!(!p.sidechain_source_available(a, a));
        let original = p.clone();
        assert!(
            p.edit(Edit::SetEffects {
                track_id: a,
                effects: vec![effect(b)],
                compact: false
            })
            .is_err()
        );
        assert_eq!(p, original);
        let group = Id::new_v4();
        p.edit(Edit::GroupTracks {
            track_ids: vec![a],
            group_id: group,
            name: "Drums".into(),
        })
        .unwrap();
        assert!(!p.sidechain_source_available(a, group));
        let ret = p.add_return_track().unwrap();
        let mut send = TrackSend::new(ret);
        send.enabled = false;
        p.tracks
            .iter_mut()
            .find(|t| t.id == b)
            .unwrap()
            .sends
            .push(send);
        assert!(!p.sidechain_source_available(a, ret));
        assert!(p.validate().is_ok());
    }
    #[test]
    fn sidechain_copy_delete_missing_and_save_round_trip_retain_source_identity() {
        let mut p = Project::default();
        let a = p.add_track().unwrap();
        let b = p.add_track().unwrap();
        p.edit(Edit::SetEffects {
            track_id: b,
            effects: vec![effect(a)],
            compact: false,
        })
        .unwrap();
        let copy = Id::new_v4();
        p.edit(Edit::DuplicateTrack {
            track_id: b,
            new_track_id: copy,
        })
        .unwrap();
        let routes = p.audio_sidechain_routes();
        assert_eq!(routes.len(), 2);
        assert_ne!(routes[0].1.id, routes[1].1.id);
        assert!(routes.iter().all(|(_, r)| r.sidechain.unwrap().track == a));
        let encoded = serde_json::to_string(&p).unwrap();
        let restored: Project = serde_json::from_str(&encoded).unwrap();
        assert_eq!(p, restored);
        p.edit(Edit::DeleteTrack(a)).unwrap();
        assert!(p.validate().is_ok());
        assert!(
            p.audio_sidechain_routes()
                .iter()
                .all(|(_, r)| r.sidechain.unwrap().track == a)
        );
    }
}
