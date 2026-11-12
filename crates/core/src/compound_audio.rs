//! Immutable source recipes. Member positions use the compound's local frame clock.
use crate::{Asset, Clip, Error, Id, Project, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const COMPOUND_AUDIO_VERSION: u32 = 1;
/// Model plus compiled-index admission; this is not a duration or track-count limit.
pub const COMPOUND_METADATA_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompoundAudio {
    pub version: u32,
    pub members: Vec<Clip>,
}
impl CompoundAudio {
    pub fn metadata_bytes(&self) -> Option<u64> {
        self.members.iter().try_fold(
            (self.members.capacity() as u64).checked_mul(std::mem::size_of::<Clip>() as u64)?,
            |sum, clip| sum.checked_add(clip.name.capacity() as u64),
        )
    }
}
impl Asset {
    pub fn is_file_audio(&self) -> bool {
        self.is_audio() && self.compound_audio.is_none()
    }
}
impl Project {
    pub(crate) fn validate_compound_asset(&self, asset: &Asset) -> Result<()> {
        let source = asset.compound_audio.as_ref().unwrap();
        if source.version != COMPOUND_AUDIO_VERSION
            || source.members.is_empty()
            || asset.source.kind != "compound_audio"
            || asset.source.path_kind != "embedded"
            || !asset.source.path.is_empty()
            || asset.midi_pattern.is_some()
            || asset.beat_pattern.is_some()
            || asset.source_metadata.sample_rate_hz != self.sample_rate_hz
            || !(1..=2).contains(&asset.source_metadata.channels)
            || asset.source_metadata.sample_format != "ieee_float"
            || asset.source_metadata.bits_per_sample != 32
            || asset.decoded_frame_count == 0
            || asset.decoded_frame_count == u64::MAX
            || source
                .metadata_bytes()
                .is_none_or(|bytes| bytes > COMPOUND_METADATA_LIMIT)
        {
            return Err(Error(
                "Invalid compound audio source or metadata admission exceeded".into(),
            ));
        }
        let mut end = 0;
        let mut ids = HashSet::new();
        for member in &source.members {
            let original = self
                .assets
                .iter()
                .find(|a| a.id == member.asset_id)
                .filter(|a| a.is_audio())
                .ok_or_else(|| Error("Compound member requires an audio asset".into()))?;
            if member.id.is_nil() || !ids.insert(member.id) || member.start_frame < end {
                return Err(Error(
                    "Invalid compound member identity or overlapping ranges".into(),
                ));
            }
            self.validate_clip_asset(member, original)?;
            end = member.end();
        }
        if source.members[0].start_frame != 0 || end != asset.decoded_frame_count {
            return Err(Error(
                "Compound bounds must match its first and last members".into(),
            ));
        }
        Ok(())
    }
    /// Iterative cycle/admission check: malformed manifests never recurse on the stack.
    pub(crate) fn validate_compound_graph(&self) -> Result<()> {
        let assets: HashMap<_, _> = self.assets.iter().map(|a| (a.id, a)).collect();
        let mut marks = HashMap::new();
        let mut depths: HashMap<Id, usize> = HashMap::new();
        let mut bytes = 0u64;
        for root in self.assets.iter().filter(|a| a.compound_audio.is_some()) {
            if marks.get(&root.id) == Some(&2) {
                continue;
            }
            let mut stack = vec![(root.id, false)];
            while let Some((id, leaving)) = stack.pop() {
                if leaving {
                    let depth = assets[&id].compound_audio.as_ref().map_or(0, |s| {
                        1 + s
                            .members
                            .iter()
                            .filter_map(|m| depths.get(&m.asset_id))
                            .copied()
                            .max()
                            .unwrap_or(0)
                    });
                    if depth > 64 {
                        return Err(Error(
                            "Compound nesting admission exceeded (64 levels)".into(),
                        ));
                    }
                    depths.insert(id, depth);
                    marks.insert(id, 2);
                    continue;
                }
                match marks.get(&id) {
                    Some(1) => return Err(Error("Compound audio contains a cycle".into())),
                    Some(2) => continue,
                    _ => {}
                }
                let asset = assets
                    .get(&id)
                    .ok_or_else(|| Error("Compound dependency is missing".into()))?;
                let Some(source) = &asset.compound_audio else {
                    depths.insert(id, 0);
                    marks.insert(id, 2);
                    continue;
                };
                bytes = bytes
                    .checked_add(
                        source
                            .metadata_bytes()
                            .ok_or_else(|| Error("Compound metadata overflow".into()))?,
                    )
                    .filter(|n| *n <= COMPOUND_METADATA_LIMIT)
                    .ok_or_else(|| Error("Compound metadata admission exceeded".into()))?;
                marks.insert(id, 1);
                stack.push((id, true));
                stack.extend(source.members.iter().rev().map(|m| (m.asset_id, false)));
            }
        }
        Ok(())
    }
    /// All immutable member clips, used by worker preparation and source consumers.
    pub fn compound_members(&self) -> impl Iterator<Item = &Clip> {
        self.assets
            .iter()
            .filter_map(|a| a.compound_audio.as_ref())
            .flat_map(|s| &s.members)
    }
    /// A cropped immutable composition needs its complete source clock, including
    /// captured transitions on the removed side of a new cut.
    pub fn compound_window_requires_preparation(&self, clip: &Clip) -> bool {
        clip.repeat.is_none()
            && self.audio_asset(clip.asset_id).is_some_and(|a| {
                a.compound_audio.is_some()
                    && (clip.source_offset_frame != 0
                        || clip.length_frames != a.decoded_frame_count)
            })
    }
    pub fn audio_asset(&self, id: Id) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id && a.is_audio())
    }
}

impl crate::ClipChannels {
    /// Compose clip channel operations in source-to-wrapper order.
    pub fn then(self, next: Self) -> Self {
        Self {
            left: next.left && if next.swapped { self.right } else { self.left },
            right: next.right && if next.swapped { self.left } else { self.right },
            inverted: self.inverted ^ next.inverted,
            swapped: self.swapped ^ next.swapped,
        }
    }
}
/// A prepared envelope clock. Cropping changes `offset`, not the captured envelope.
#[derive(Clone, Debug)]
pub struct CompiledAudioEnvelope {
    pub fades: crate::ClipFades,
    pub length_frames: u64,
    pub offset: u64,
}
#[derive(Clone, Debug)]
pub struct CompiledAudioTrack {
    pub track: crate::Track,
    /// First entry is the original member envelope; subsequent entries are outer edits.
    pub envelopes: Vec<Vec<CompiledAudioEnvelope>>,
}
impl CompiledAudioTrack {
    pub fn metadata_bytes(&self) -> u64 {
        self.track.clips.capacity() as u64 * std::mem::size_of::<Clip>() as u64
            + self
                .track
                .clips
                .iter()
                .map(|c| c.name.capacity() as u64)
                .sum::<u64>()
            + self.envelopes.capacity() as u64
                * std::mem::size_of::<Vec<CompiledAudioEnvelope>>() as u64
            + self
                .envelopes
                .iter()
                .map(|e| e.capacity() as u64 * std::mem::size_of::<CompiledAudioEnvelope>() as u64)
                .sum::<u64>()
    }
}
impl Project {
    pub fn compile_audio_track(
        &self,
        track: &crate::Track,
        materialized: impl Fn(Id) -> bool,
    ) -> Result<crate::Track> {
        Ok(self.compile_audio_track_view(track, materialized)?.track)
    }
    /// Compile immutable members once. Gaps and repetitions do not allocate spans.
    pub fn compile_audio_track_view(
        &self,
        track: &crate::Track,
        materialized: impl Fn(Id) -> bool,
    ) -> Result<CompiledAudioTrack> {
        self.compile_audio_track_view_cancellable(track, materialized, &|| false)
    }
    pub fn compile_audio_track_view_cancellable(
        &self,
        track: &crate::Track,
        materialized: impl Fn(Id) -> bool,
        cancelled: &impl Fn() -> bool,
    ) -> Result<CompiledAudioTrack> {
        let has_compounds = track.clips.iter().any(|c| {
            self.audio_asset(c.asset_id)
                .is_some_and(|a| a.compound_audio.is_some())
        });
        if has_compounds
            && track
                .clips
                .iter()
                .try_fold(0u64, |sum, c| {
                    sum.checked_add(std::mem::size_of::<Clip>() as u64 + c.name.len() as u64)
                })
                .is_none_or(|n| n > COMPOUND_METADATA_LIMIT / 4)
        {
            return Err(Error("Compound input metadata admission exceeded".into()));
        }
        let mut out = track.clone();
        out.clips.clear();
        let mut result = Vec::new();
        let mut used: HashSet<_> = track.clips.iter().map(|c| c.id).collect();
        let mut stack: Vec<_> = track
            .clips
            .iter()
            .rev()
            .cloned()
            .map(|c| (c, Vec::<CompiledAudioEnvelope>::new(), None))
            .collect();
        let mut bytes = 0u64;
        let mut visited = 0usize;
        while let Some((wrapper, outer, original)) = stack.pop() {
            visited += 1;
            if visited.is_multiple_of(1024) && cancelled() {
                return Err(Error("Audio preparation cancelled".into()));
            }
            bytes = bytes
                .checked_add(
                    std::mem::size_of::<Clip>() as u64
                        + wrapper.name.len() as u64
                        + outer.len() as u64 * std::mem::size_of::<CompiledAudioEnvelope>() as u64,
                )
                .filter(|b| !has_compounds || *b <= COMPOUND_METADATA_LIMIT / 4)
                .ok_or_else(|| Error("Compound compiled metadata admission exceeded".into()))?;
            let source = self
                .audio_asset(wrapper.asset_id)
                .and_then(|a| a.compound_audio.as_ref());
            let cut = self.compound_window_requires_preparation(&wrapper);
            if source.is_none()
                || cut
                || wrapper.repeat.is_some()
                || wrapper.stretch.is_some()
                || wrapper.transpose != 0
            {
                if source.is_some()
                    && (cut || wrapper.stretch.is_some() || wrapper.transpose != 0)
                    && !materialized(wrapper.asset_id)
                {
                    return Err(Error(
                        "Compound source transform requires worker preparation".into(),
                    ));
                }
                let original = original.unwrap_or(CompiledAudioEnvelope {
                    fades: wrapper.fades,
                    length_frames: wrapper.length_frames,
                    offset: 0,
                });
                let mut envelopes = vec![original];
                envelopes.extend(outer);
                result.push((wrapper, envelopes));
                continue;
            }
            let source = source.unwrap();
            let wrapper_clock = original.unwrap_or(CompiledAudioEnvelope {
                fades: wrapper.fades,
                length_frames: wrapper.length_frames,
                offset: 0,
            });
            let from = wrapper.source_offset_frame;
            let to = from
                .checked_add(wrapper.length_frames)
                .ok_or_else(|| Error("Compound source range overflow".into()))?;
            let first = source.members.partition_point(|m| m.end() <= from);
            let last = source.members.partition_point(|m| m.start_frame < to);
            let members = &source.members[first..last.max(first)];
            let queued = (stack.len() as u64)
                .saturating_add(members.len() as u64)
                .saturating_mul(
                    std::mem::size_of::<Clip>() as u64
                        + (outer.len() + 1) as u64
                            * std::mem::size_of::<CompiledAudioEnvelope>() as u64,
                );
            if bytes.saturating_add(queued) > COMPOUND_METADATA_LIMIT / 4 {
                return Err(Error("Compound queued metadata admission exceeded".into()));
            }
            stack
                .try_reserve(members.len())
                .map_err(|_| Error("Cannot reserve compound index".into()))?;
            for member in members.iter().rev() {
                let start = member.start_frame.max(from);
                let end = member.end().min(to);
                if end <= start {
                    continue;
                }
                let mut piece = member.retained_range(start, end);
                if let Some(repeat) = member.repeat {
                    piece.repeat = Some(repeat.shifted(i128::from(start - member.start_frame)));
                    piece.source_offset_frame = member.source_offset_frame;
                }
                piece.start_frame = wrapper
                    .start_frame
                    .checked_add(start - from)
                    .ok_or_else(|| Error("Compound timeline overflow".into()))?;
                piece.gain_db += wrapper.gain_db;
                if !crate::linear_gain(piece.gain_db).is_finite() {
                    return Err(Error(
                        "Compound gain exceeds finite processing bounds".into(),
                    ));
                }
                piece.channels = piece.channels.then(wrapper.channels);
                piece.muted |= wrapper.muted;
                let mut original = CompiledAudioEnvelope {
                    fades: member.fades,
                    length_frames: member.length_frames,
                    offset: start - member.start_frame,
                };
                // A new outer edge overrides the exposed captured edge. Interior source envelopes remain intrinsic.
                if !wrapper_clock.fades.fade_in.inherit_source && start == from {
                    original.fades.fade_in.inherit_source = true;
                    piece.fades.fade_in = wrapper_clock.fades.fade_in;
                }
                if !wrapper_clock.fades.fade_out.inherit_source && end == to {
                    original.fades.fade_out.inherit_source = true;
                    piece.fades.fade_out = wrapper_clock.fades.fade_out;
                }
                // Captured transitions only bind when their original dividing line survives.
                if start != member.start_frame {
                    piece.fades.fade_in.crossfade = false;
                }
                if end != member.end() {
                    piece.fades.fade_out.crossfade = false;
                }
                let mut stages = Vec::new();
                if !wrapper_clock.fades.fade_in.inherit_source
                    || !wrapper_clock.fades.fade_out.inherit_source
                {
                    stages.push(CompiledAudioEnvelope {
                        fades: wrapper_clock.fades,
                        length_frames: wrapper_clock.length_frames,
                        offset: wrapper_clock
                            .offset
                            .checked_add(start - from)
                            .ok_or_else(|| Error("Compound envelope overflow".into()))?,
                    });
                }
                for stage in &outer {
                    let mut stage = stage.clone();
                    stage.offset = stage
                        .offset
                        .checked_add(start - from)
                        .ok_or_else(|| Error("Compound envelope overflow".into()))?;
                    stages.push(stage);
                }
                let mut bits = wrapper.id.as_u128().wrapping_mul(0x9e3779b97f4a7c15)
                    ^ member.id.as_u128().rotate_left(43);
                while bits == 0 || !used.insert(Id::from_u128(bits)) {
                    bits = bits.wrapping_add(1);
                }
                piece.id = Id::from_u128(bits);
                stack.push((piece, stages, Some(original)));
            }
        }
        result.sort_by_key(|(c, _)| c.start_frame);
        let mut envelopes = Vec::new();
        for (clip, clocks) in result {
            out.clips.push(clip);
            envelopes.push(clocks);
        }
        Ok(CompiledAudioTrack {
            track: out,
            envelopes,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AudioEdgeGeometry {
    pub channels: u16,
    pub cap: u64,
    pub handles: u64,
}
impl Project {
    /// Captured source continuation is bounded by the original edge's half-length cap.
    pub fn compound_edge_extension(&self, id: Id, left: bool) -> u64 {
        self.compound_extension_at(id, left, 0, &mut HashMap::new())
    }
    fn compound_extension_at(
        &self,
        id: Id,
        left: bool,
        depth: usize,
        cache: &mut HashMap<(Id, bool), u64>,
    ) -> u64 {
        if let Some(value) = cache.get(&(id, left)) {
            return *value;
        }
        if depth >= 256 {
            return 0;
        }
        let Some(source) = self.audio_asset(id).and_then(|a| a.compound_audio.as_ref()) else {
            return 0;
        };
        let member = if left {
            source.members.first()
        } else {
            source.members.last()
        };
        let result = member
            .and_then(|c| self.edge_geometry_at(c, left, depth + 1, cache))
            .map_or(0, |g| g.cap.min(g.handles));
        cache.insert((id, left), result);
        result
    }
    pub(crate) fn audio_edge_geometry(&self, clip: &Clip, left: bool) -> Option<AudioEdgeGeometry> {
        self.edge_geometry_at(clip, left, 0, &mut HashMap::new())
    }
    fn edge_geometry_at(
        &self,
        clip: &Clip,
        left: bool,
        depth: usize,
        cache: &mut HashMap<(Id, bool), u64>,
    ) -> Option<AudioEdgeGeometry> {
        if depth >= 256 || clip.muted {
            return None;
        }
        let asset = self.audio_asset(clip.asset_id)?;
        let cap = clip.length_frames.saturating_sub(1) / 2;
        let Some(recipe) = &asset.compound_audio else {
            let handles = if clip.repeat.is_some() {
                u64::MAX
            } else if left {
                clip.source_offset_frame
            } else {
                clip.source_length(asset.decoded_frame_count)?
                    .saturating_sub(clip.source_offset_frame.saturating_add(clip.length_frames))
            };
            return Some(AudioEdgeGeometry {
                channels: asset.source_metadata.channels,
                cap,
                handles,
            });
        };
        let position = clip.source_frame(if left {
            0
        } else {
            clip.length_frames.checked_sub(1)?
        });
        let source_point = clip
            .stretch
            .map_or(position, |s| s.original_frame(position));
        let member = recipe
            .members
            .iter()
            .find(|m| m.start_frame <= source_point && source_point < m.end())?;
        let mut exposed = member.clone();
        let advance = source_point - member.start_frame;
        if left {
            if let Some(repeat) = exposed.repeat {
                exposed.repeat = Some(repeat.shifted(i128::from(advance)));
            } else {
                exposed.source_offset_frame = exposed.source_frame(advance);
            }
            exposed.length_frames = exposed.length_frames.saturating_sub(advance);
        } else {
            exposed.length_frames = advance + 1;
        }
        let child = self.edge_geometry_at(&exposed, left, depth + 1, cache)?;
        let scale = |frames| clip.stretch.map_or(Some(frames), |s| s.scale(frames));
        let inherited = if left {
            clip.fades.fade_in.inherit_source
        } else {
            clip.fades.fade_out.inherit_source
        };
        let extension = self.compound_extension_at(asset.id, left, depth + 1, cache);
        let interior = if left {
            source_point
        } else {
            asset.decoded_frame_count.saturating_sub(source_point + 1)
        };
        Some(AudioEdgeGeometry {
            channels: child.channels,
            cap: if inherited { scale(child.cap)? } else { cap },
            handles: if clip.repeat.is_some() {
                u64::MAX
            } else {
                scale(interior.saturating_add(extension))?
            },
        })
    }
}
