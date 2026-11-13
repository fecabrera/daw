//! Captured routing uses companion recipes on the same immutable compound clock.
use crate::{
    Asset, COMPOUND_METADATA_LIMIT, Error, Id, Project, RenderedAudio, RenderedFeed, Result,
    SILENT_GAIN_DB,
};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Copy)]
enum Tap {
    PreSend,
    Provider(Id, bool),
}
impl Project {
    /// Upgrade routing metadata transactionally, without decoding or rendering PCM.
    pub fn prepare_compound_routing(&mut self) -> Result<()> {
        self.validate()?;
        let mut candidate = self.clone();
        candidate.prepare_compound_routing_in_place()?;
        *self = candidate;
        Ok(())
    }
    fn prepare_compound_routing_in_place(&mut self) -> Result<()> {
        let used = self
            .assets
            .iter()
            .filter_map(|a| a.compound_audio.as_ref())
            .try_fold(0u64, |sum, source| {
                sum.checked_add(source.metadata_bytes()?)
            })
            .ok_or_else(|| Error("Compound routing metadata overflow".into()))?;
        let mut remaining = COMPOUND_METADATA_LIMIT
            .checked_sub(used)
            .ok_or_else(|| Error("Compound routing metadata admission exceeded".into()))?;
        let auxiliary: HashSet<_> = self
            .assets
            .iter()
            .filter_map(|a| a.rendered_audio.as_ref().map(|r| (a.id, r)))
            .flat_map(|(id, r)| r.dependencies().filter(move |child| *child != id))
            .collect();
        let mut done = HashSet::new();
        let mut order = Vec::new();
        for root in self.assets.iter().filter(|a| a.compound_audio.is_some()) {
            let mut stack = vec![(root.id, false)];
            while let Some((id, leaving)) = stack.pop() {
                if done.contains(&id) {
                    continue;
                }
                if leaving {
                    done.insert(id);
                    order.push(id);
                } else {
                    stack.push((id, true));
                    if let Some(source) =
                        self.audio_asset(id).and_then(|a| a.compound_audio.as_ref())
                    {
                        stack.extend(source.members.iter().map(|c| (c.asset_id, false)));
                    }
                }
            }
        }
        for id in order {
            let asset = self.audio_asset(id).unwrap();
            if auxiliary.contains(&id)
                || asset.rendered_audio.is_some()
                || asset.compound_audio.is_none()
            {
                continue;
            }
            let providers: BTreeSet<_> = asset
                .compound_audio
                .as_ref()
                .unwrap()
                .members
                .iter()
                .filter_map(|c| self.audio_asset(c.asset_id)?.rendered_audio.as_ref())
                .flat_map(|r| r.feeds.iter().map(|f| f.provider))
                .collect();
            if providers.is_empty() {
                continue;
            }
            let original = asset.clone();
            let pre_send_asset = self.routing_recipe(&original, Tap::PreSend, &mut remaining)?;
            let mut feeds = Vec::new();
            for provider in providers {
                feeds.push(RenderedFeed {
                    provider,
                    raw_asset: self.routing_recipe(
                        &original,
                        Tap::Provider(provider, false),
                        &mut remaining,
                    )?,
                    post_asset: self.routing_recipe(
                        &original,
                        Tap::Provider(provider, true),
                        &mut remaining,
                    )?,
                });
            }
            self.assets
                .iter_mut()
                .find(|a| a.id == id)
                .unwrap()
                .rendered_audio = Some(RenderedAudio {
                version: 2,
                pre_send_asset,
                feeds,
            });
        }
        self.validate()
    }
    fn routing_recipe(&mut self, original: &Asset, tap: Tap, remaining: &mut u64) -> Result<Id> {
        let bytes = original
            .compound_audio
            .as_ref()
            .unwrap()
            .metadata_bytes()
            .and_then(|n| n.checked_add(std::mem::size_of::<Asset>() as u64))
            .ok_or_else(|| Error("Compound routing metadata overflow".into()))?;
        *remaining = remaining
            .checked_sub(bytes)
            .ok_or_else(|| Error("Compound routing metadata admission exceeded".into()))?;
        self.assets
            .try_reserve(1)
            .map_err(|_| Error("Cannot reserve compound routing metadata".into()))?;
        let mut asset = original.clone();
        asset.id = match tap {
            Tap::PreSend => derived(original.id, Id::nil(), 1),
            Tap::Provider(provider, post) => {
                derived(original.id, provider, if post { 3 } else { 2 })
            }
        };
        asset.rendered_audio = None;
        for member in &mut asset.compound_audio.as_mut().unwrap().members {
            member.id = derived(asset.id, member.id, 4);
            let captured = self
                .audio_asset(member.asset_id)
                .and_then(|a| a.rendered_audio.as_ref());
            let selected = match tap {
                Tap::PreSend => captured.map(|r| r.pre_send_asset),
                Tap::Provider(provider, post) => captured
                    .and_then(|r| r.feeds.iter().find(|f| f.provider == provider))
                    .map(|f| if post { f.post_asset } else { f.raw_asset }),
            };
            if let Some(id) = selected {
                member.asset_id = id;
            } else {
                // Keep source geometry without inventing a historical contribution.
                if let Some(captured) = captured {
                    member.asset_id = captured.pre_send_asset;
                }
                if matches!(tap, Tap::PreSend) {
                    // Ordinary audio keeps the current track's post-effect Pre
                    // stage. It must not activate the historical override.
                    member.muted = true;
                } else {
                    member.gain_db = SILENT_GAIN_DB;
                }
            }
        }
        let id = asset.id;
        self.assets.push(asset);
        Ok(id)
    }
}

// Stable private identities make read-only legacy inspection agree with opening.
// Collisions, including malicious source IDs, are rejected by Project::validate.
fn derived(owner: Id, source: Id, tag: u128) -> Id {
    let value = owner
        .as_u128()
        .wrapping_mul(0x9e3779b97f4a7c15d1b54a32d192ed03)
        ^ source.as_u128().rotate_left(47)
        ^ tag.wrapping_mul(0x94d049bb133111ebbf58476d1ce4e5b9);
    let mut bytes = value.to_be_bytes();
    bytes[6] = (bytes[6] & 15) | 0x80;
    bytes[8] = (bytes[8] & 63) | 0x80;
    Id::from_bytes(bytes)
}
