//! Bounce selection and atomic installation. All audio work belongs to workers.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BounceMode {
    Clips,
    Tracks,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BounceRequest {
    pub mode: BounceMode,
    pub ids: Vec<Id>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BounceTarget {
    pub track_id: Id,
    pub clip_ids: Vec<Id>,
    pub start_frame: u64,
    pub end_frame: u64,
}
#[derive(Clone, Debug)]
pub struct BounceTargets {
    pub targets: Vec<BounceTarget>,
    pub warnings: Vec<String>,
}
/// Required capability: older readers reject `rendered_audio` instead of applying
/// an extra boundary envelope or losing routing feeds silently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderedAudio {
    pub version: u32,
    pub pre_send_asset: Id,
    pub feeds: Vec<RenderedFeed>,
}
impl RenderedAudio {
    pub fn dependencies(&self) -> impl Iterator<Item = Id> + '_ {
        std::iter::once(self.pre_send_asset)
            .chain(self.feeds.iter().flat_map(|f| [f.raw_asset, f.post_asset]))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderedFeed {
    pub provider: Id,
    pub raw_asset: Id,
    pub post_asset: Id,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BounceOutput {
    pub source_track: Id,
    pub assets: Vec<Asset>,
    pub track: Track,
}
impl Project {
    /// A complete rendered-source loop already contains its captured edge fades.
    /// Cropped loop ranges still need the ordinary seam envelope.
    pub fn rendered_loop_has_baked_edges(&self, clip: &Clip) -> bool {
        let Some(asset) = self.audio_asset(clip.asset_id) else {
            return false;
        };
        clip.source_offset_frame == 0
            && clip.repeat.is_some_and(|r| {
                Some(r.length_frames) == clip.source_length(asset.decoded_frame_count)
            })
            && self.assets.iter().any(|a| {
                a.rendered_audio
                    .as_ref()
                    .is_some_and(|r| a.id == asset.id || r.dependencies().any(|id| id == asset.id))
            })
    }

    pub fn bounce_targets(&self, request: &BounceRequest) -> Result<BounceTargets> {
        if request.ids.is_empty() {
            return Err(Error("Select clips or tracks to bounce".into()));
        }
        let wanted: HashSet<_> = request.ids.iter().copied().collect();
        let present: HashSet<_> = match request.mode {
            BounceMode::Tracks => self.tracks.iter().map(|t| t.id).collect(),
            BounceMode::Clips => self
                .tracks
                .iter()
                .flat_map(|t| t.clips.iter().map(|c| c.id))
                .collect(),
        };
        if !wanted.is_subset(&present) {
            return Err(Error("Bounce selection no longer exists".into()));
        }
        let mut targets = Vec::new();
        let mut warnings = Vec::new();
        for t in &self.tracks {
            let clips: Vec<_> = t
                .clips
                .iter()
                .filter(|c| match request.mode {
                    BounceMode::Clips => wanted.contains(&c.id),
                    BounceMode::Tracks => wanted.contains(&t.id),
                })
                .collect();
            let selected =
                !clips.is_empty() || (request.mode == BounceMode::Tracks && wanted.contains(&t.id));
            if !selected {
                continue;
            }
            if !matches!(t.kind, TrackKind::Audio | TrackKind::Midi | TrackKind::Beat)
                || clips.is_empty()
            {
                warnings.push(format!("{}: skipped (no clip content to bounce)", t.name));
                continue;
            }
            if clips.iter().all(|c| c.muted) {
                warnings.push(format!("{}: disabled clips produce silent output", t.name));
            }
            if request.mode == BounceMode::Clips
                && t.clips.iter().any(|c| !wanted.contains(&c.id) && !c.muted)
            {
                warnings.push(format!("{}: separately processed selected and remaining clips can change shared effect interaction",t.name));
            }
            targets.push(BounceTarget {
                track_id: t.id,
                clip_ids: clips.iter().map(|c| c.id).collect(),
                start_frame: clips.iter().map(|c| c.start_frame).min().unwrap(),
                end_frame: clips.iter().map(|c| c.end()).max().unwrap(),
            });
        }
        if targets.is_empty() {
            return Err(Error(
                "No selected Audio, MIDI or Beat tracks contain clips".into(),
            ));
        }
        Ok(BounceTargets { targets, warnings })
    }
    pub fn bounced(&self, request: &BounceRequest, outputs: Vec<BounceOutput>) -> Result<Self> {
        let targets = self.bounce_targets(request)?.targets;
        if outputs.len() != targets.len() {
            return Err(Error("Incomplete bounce output".into()));
        }
        let mut outputs: HashMap<_, _> = outputs.into_iter().map(|o| (o.source_track, o)).collect();
        if outputs.len() != targets.len() {
            return Err(Error("Duplicate bounce output".into()));
        }
        let mut next = self.clone();
        for target in targets.iter().rev() {
            let output = outputs
                .remove(&target.track_id)
                .ok_or_else(|| Error("Bounce target mismatch".into()))?;
            let position = next
                .tracks
                .iter()
                .position(|t| t.id == target.track_id)
                .unwrap();
            let original = &mut next.tracks[position];
            if output.track.kind != TrackKind::Audio
                || output.track.group_id != original.group_id
                || output.track.clips.len() != 1
                || output.track.clips[0].start_frame != target.start_frame
                || output.track.clips[0].end() < target.end_frame
                || output.track.gain_db != 0.0
                || output.track.pan != 0.0
                || !output.track.effects.is_empty()
                || output.track.instrument.is_some()
                || output.track.beat.is_some()
            {
                return Err(Error("Invalid bounced track".into()));
            }
            let main = output
                .assets
                .iter()
                .find(|a| a.id == output.track.clips[0].asset_id)
                .ok_or_else(|| Error("Missing bounced source".into()))?;
            let clip = &output.track.clips[0];
            if main.rendered_audio.is_none()
                || clip.source_offset_frame != 0
                || clip.length_frames != main.decoded_frame_count
                || clip.repeat.is_some()
                || clip.stretch.is_some()
                || clip.gain_db != 0.0
                || clip.transpose != 0
                || !clip.channels.is_default()
                || clip.muted
                || !clip.fades.fade_in.inherit_source
                || !clip.fades.fade_out.inherit_source
                || clip.end().saturating_sub(target.end_frame) > u64::from(self.sample_rate_hz) * 10
                || output.track.muted != original.muted
                || output.track.soloed != original.soloed
                || output.track.color != original.color
            {
                return Err(Error(
                    "Invalid neutral bounced source or track controls".into(),
                ));
            }
            if output.track.sends.len() != original.sends.len()
                || output
                    .track
                    .sends
                    .iter()
                    .zip(&original.sends)
                    .any(|(copy, source)| {
                        copy.id == source.id || {
                            let mut expected = source.clone();
                            expected.id = copy.id;
                            *copy != expected
                        }
                    })
            {
                return Err(Error("Bounce must copy sends with fresh identities".into()));
            }
            for clip in &mut original.clips {
                if target.clip_ids.contains(&clip.id) {
                    clip.muted = true;
                }
            }
            next.assets.extend(output.assets);
            next.tracks.insert(position + 1, output.track);
        }
        next.validate()?;
        Ok(next)
    }
    pub(crate) fn validate_rendered_sources(&self) -> Result<()> {
        for a in &self.assets {
            let Some(source) = &a.rendered_audio else {
                if a.source.kind == "rendered_audio" {
                    return Err(Error("Missing rendered audio capability".into()));
                }
                continue;
            };
            if source.version != 1
                || a.source.kind != "rendered_audio"
                || a.compound_audio.is_some()
                || a.midi_pattern.is_some()
                || a.beat_pattern.is_some()
                || a.source_metadata.channels != 2
                || a.source_metadata.sample_format != "ieee_float"
                || a.source_metadata.bits_per_sample != 32
                || source.feeds.is_empty()
                || (source.feeds.len() as u64)
                    .saturating_mul(std::mem::size_of::<RenderedFeed>() as u64)
                    > COMPOUND_METADATA_LIMIT
                || source.pre_send_asset == a.id
                || source.feeds.iter().any(|f| f.raw_asset == a.id)
            {
                return Err(Error("Invalid rendered audio capability".into()));
            }
            let mut providers = HashSet::new();
            for feed in &source.feeds {
                if feed.provider.is_nil() || !providers.insert(feed.provider) {
                    return Err(Error("Invalid captured provider identity".into()));
                }
            }
            for id in source.dependencies() {
                let leaf = self
                    .assets
                    .iter()
                    .find(|l| l.id == id)
                    .ok_or_else(|| Error("Missing rendered audio dependency".into()))?;
                if id != a.id && (leaf.rendered_audio.is_some() || !leaf.is_file_audio())
                    || leaf.decoded_frame_count != a.decoded_frame_count
                    || leaf.source_metadata != a.source_metadata
                {
                    return Err(Error("Rendered audio clocks or formats differ".into()));
                }
            }
        }
        Ok(())
    }
    /// Copy all clip edits to a captured leaf, retaining its common source clock.
    pub fn rendered_clips(
        &self,
        track: &Track,
        select: impl Fn(&RenderedAudio) -> Option<Id>,
    ) -> Vec<Clip> {
        track
            .clips
            .iter()
            .filter_map(|c| {
                let source = self.audio_asset(c.asset_id)?.rendered_audio.as_ref()?;
                let mut clip = c.clone();
                clip.asset_id = select(source)?;
                Some(clip)
            })
            .collect()
    }
    pub fn rendered_providers(&self, track: &Track) -> Vec<Id> {
        let mut ids: Vec<_> = track
            .clips
            .iter()
            .filter_map(|c| self.audio_asset(c.asset_id)?.rendered_audio.as_ref())
            .flat_map(|a| a.feeds.iter().map(|f| f.provider))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}
