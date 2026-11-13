//! Worker source access without track, bus or Master processing.
use crate::{RenderPlan, schedule::AudioTrack};
use daw_core::{Id, Track};

/// An independently prepared source clock. Its readers belong to the worker.
/// Playback uses the same source scheduler through the owning track.
pub struct CompoundAudioSource {
    source: AudioTrack,
    pub length_frames: u64,
}
impl CompoundAudioSource {
    pub fn new(plan: &RenderPlan, asset_id: Id) -> Result<Self, String> {
        let asset = plan
            .project
            .audio_asset(asset_id)
            .ok_or("Audio source is missing")?;
        let recipe = asset
            .compound_audio
            .as_ref()
            .ok_or("Expected a compound source")?;
        let mut project = daw_core::Project::default();
        let id = project.add_track().map_err(|e| e.to_string())?;
        let mut track: Track = project.tracks.remove(0);
        track.id = id;
        track.clips = recipe.members.clone();
        let source = AudioTrack::new(plan, &track);
        if let Some(error) = source.source_error() {
            return Err(error);
        }
        Ok(Self {
            source,
            length_frames: asset.decoded_frame_count,
        })
    }
    /// Worker-only. Read failures must never publish a successful silent extraction.
    pub fn sample(&mut self, frame: u64) -> Result<([f32; 2], Option<bool>), String> {
        self.sample_signed(i128::from(frame))
    }
    pub fn sample_signed(&mut self, frame: i128) -> Result<([f32; 2], Option<bool>), String> {
        let sample = if frame < 0 || frame >= i128::from(self.length_frames) {
            self.source.sample_extended(frame)?
        } else {
            if !self.source.worker_window_available(frame as u64) {
                return Err("Compound source range is unavailable".into());
            }
            self.source.sample(&[], frame as u64, false)
        };
        if self.source.source_failed() {
            return Err(self
                .source
                .source_error()
                .unwrap_or_else(|| "Compound source read failed".into()));
        }
        Ok(sample)
    }
}

impl RenderPlan {
    /// Worker admission without effect construction or a device. Validate the exact
    /// visible source schedules, including nested loop readers, before publication.
    pub fn compound_schedule_bytes(&self, cancelled: &impl Fn() -> bool) -> Result<u64, String> {
        let mut bytes = 0u64;
        for track in &self.project.tracks {
            if cancelled() {
                return Err("Audio preparation cancelled".into());
            }
            if !track.clips.iter().any(|c| {
                self.project
                    .audio_asset(c.asset_id)
                    .is_some_and(|a| a.compound_audio.is_some())
            }) {
                continue;
            }
            let source = AudioTrack::new(self, track);
            if let Some(error) = source.source_error() {
                return Err(error);
            }
            bytes = bytes
                .checked_add(source.heap_bytes())
                .ok_or("Compound schedule size overflow")?;
            if bytes > daw_core::COMPOUND_METADATA_LIMIT / 4 {
                return Err("Compound schedule metadata admission exceeded".into());
            }
        }
        Ok(bytes)
    }
}

/// Channel tags remain source-time metadata when a compound is linearized for
/// pitch/stretch. A silent gap keeps `None`; equal L/R samples do not imply mono.
#[derive(Clone)]
pub(crate) struct CompoundLayout {
    ranges: crate::schedule::Ranges,
    members: Vec<LayoutMember>,
}
#[derive(Clone)]
struct LayoutMember {
    clip: daw_core::Clip,
    child: Option<Box<CompoundLayout>>,
    mono: Option<bool>,
}
impl CompoundLayout {
    pub fn new(plan: &RenderPlan, id: Id) -> Result<Self, String> {
        Self::build(plan, id, 0, &mut (daw_core::COMPOUND_METADATA_LIMIT / 8))
    }
    fn build(plan: &RenderPlan, id: Id, depth: usize, budget: &mut u64) -> Result<Self, String> {
        if depth >= 64 {
            return Err("Compound layout nesting admission exceeded".into());
        }
        let recipe = plan
            .project
            .audio_asset(id)
            .and_then(|a| a.compound_audio.as_ref())
            .ok_or("Compound layout source missing")?;
        let bytes = recipe.members.len() as u64 * std::mem::size_of::<LayoutMember>() as u64;
        *budget = budget
            .checked_sub(bytes)
            .ok_or("Compound layout metadata admission exceeded")?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(recipe.members.len())
            .map_err(|_| "Cannot reserve compound channel layout")?;
        for clip in &recipe.members {
            let child = if plan
                .project
                .audio_asset(clip.asset_id)
                .is_some_and(|a| a.compound_audio.is_some())
            {
                Some(Box::new(Self::build(
                    plan,
                    clip.asset_id,
                    depth + 1,
                    budget,
                )?))
            } else {
                None
            };
            let mono = plan.audio_for_clip(clip).map(|a| a.metadata.channels == 1);
            members.push(LayoutMember {
                clip: clip.clone(),
                child,
                mono,
            });
        }
        Ok(Self {
            ranges: crate::schedule::Ranges::new(
                members.iter().map(|m| (m.clip.start_frame, m.clip.end())),
            ),
            members,
        })
    }
    /// Authored activity, independent of decoded availability or sample energy.
    /// Historical Pre overrides must not replace ordinary post-effect audio.
    pub fn active_at(&mut self, frame: u64) -> bool {
        let Some(index) = self.ranges.at(frame) else {
            return false;
        };
        let member = &mut self.members[index];
        if member.clip.muted {
            return false;
        }
        member.child.as_mut().is_none_or(|child| {
            let position = member.clip.source_frame(frame - member.clip.start_frame);
            child.active_at(
                member
                    .clip
                    .stretch
                    .map_or(position, |s| s.original_frame(position)),
            )
        })
    }
    pub fn at_continued(&mut self, frame: i128) -> Option<bool> {
        let last = self.members.last()?.clip.end().checked_sub(1)?;
        self.at(frame.clamp(0, i128::from(last)) as u64)
    }
    pub fn at(&mut self, frame: u64) -> Option<bool> {
        let member = &mut self.members[self.ranges.at(frame)?];
        if member.clip.muted {
            return None;
        }
        if let Some(child) = &mut member.child {
            let frame = member.clip.source_frame(frame - member.clip.start_frame);
            child.at(member
                .clip
                .stretch
                .map_or(frame, |s| s.original_frame(frame)))
        } else {
            member.mono
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        self.ranges.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.members)
            + self
                .members
                .iter()
                .map(|m| {
                    m.clip.name.capacity() as u64
                        + m.child
                            .as_ref()
                            .map_or(0, |c| std::mem::size_of::<Self>() as u64 + c.heap_bytes())
                })
                .sum::<u64>()
    }
}
