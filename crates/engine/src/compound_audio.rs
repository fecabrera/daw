//! Worker source access without track, bus or Master processing.
use crate::{RenderPlan, schedule::AudioTrack};
use daw_core::{Id, Track};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// An independently prepared source clock. Its readers belong to the worker.
/// Playback uses the same source scheduler through the owning track.
pub struct CompoundAudioSource {
    source: AudioTrack,
    pub length_frames: u64,
    analysis: Option<AnalysisBudget>,
}
impl CompoundAudioSource {
    pub fn new(plan: &RenderPlan, asset_id: Id) -> Result<Self, String> {
        Self::build(plan, asset_id, None)
    }

    pub fn for_analysis(
        plan: &RenderPlan,
        asset_id: Id,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let mut spans = 4096;
        let per_frame = inspect_recipe(plan, asset_id, &mut spans, &cancelled, 0, true)?;
        Self::build(
            plan,
            asset_id,
            Some(AnalysisBudget {
                visits: 128_000_000,
                per_frame,
                cancelled,
            }),
        )
    }

    fn build(
        plan: &RenderPlan,
        asset_id: Id,
        analysis: Option<AnalysisBudget>,
    ) -> Result<Self, String> {
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
        let source = if let Some(budget) = &analysis {
            let mut source = AudioTrack::new_for_analysis(plan, &track, &|| {
                budget.cancelled.load(Ordering::Relaxed)
            });
            source.limit_analysis_readers(
                &mut 16,
                &Arc::new(daw_media::OptionalReadBudget::new(
                    128 * 1024 * 1024,
                    budget.cancelled.clone(),
                )),
            )?;
            source
        } else {
            AudioTrack::new(plan, &track)
        };
        if let Some(error) = source.source_error() {
            return Err(error);
        }
        Ok(Self {
            source,
            length_frames: asset.decoded_frame_count,
            analysis,
        })
    }
    pub fn read_analysis_frames(&mut self, start: u64, out: &mut [[f32; 2]]) -> Result<(), String> {
        let budget = self
            .analysis
            .as_mut()
            .ok_or("Expected an analysis-only source")?;
        let end = start
            .checked_add(out.len() as u64)
            .filter(|end| *end <= self.length_frames)
            .ok_or("Compound tempo range is outside its source")?;
        budget.visits = budget
            .visits
            .checked_sub(budget.per_frame.saturating_mul(out.len() as u64))
            .ok_or("Tempo analysis schedule visit limit exceeded")?;
        for (index, (frame, output)) in (start..end).zip(out).enumerate() {
            if index.is_multiple_of(256) {
                if budget.cancelled.load(Ordering::Relaxed) {
                    return Err("Tempo analysis cancelled".into());
                }
                if let Some(error) = self.source.source_error() {
                    return Err(error);
                }
            }
            if !self.source.worker_window_available(frame) {
                return Err("Incomplete compound source range".into());
            }
            *output = self.source.sample(&[], frame, false).0;
        }
        // A worker-local admission error can be masked by the normal scheduler's
        // silent fallback. Inspect errors even when shared disk state is healthy.
        if let Some(error) = self.source.source_error() {
            return Err(error);
        }
        Ok(())
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

struct AnalysisBudget {
    visits: u64,
    per_frame: u64,
    cancelled: Arc<AtomicBool>,
}

/// Preflight every main member, including sections outside the sampled windows.
/// Count expanded instances before scheduler and channel layout construction.
fn inspect_recipe(
    plan: &RenderPlan,
    id: Id,
    remaining: &mut usize,
    cancelled: &AtomicBool,
    depth: usize,
    audible: bool,
) -> Result<u64, String> {
    if depth >= 64 || cancelled.load(Ordering::Relaxed) {
        return Err("Tempo analysis cancelled or compound nesting limit exceeded".into());
    }
    let recipe = plan
        .project
        .audio_asset(id)
        .and_then(|asset| asset.compound_audio.as_ref())
        .ok_or("Missing compound source")?;
    *remaining = remaining
        .checked_sub(recipe.members.len())
        .ok_or("Tempo analysis compound span limit exceeded")?;
    let mut worst_child = 0;
    for clip in &recipe.members {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Tempo analysis cancelled".into());
        }
        let asset = plan
            .project
            .audio_asset(clip.asset_id)
            .ok_or("Missing compound member")?;
        let audible = audible && !clip.muted;
        let transformed = clip.stretch.is_some()
            || clip.transpose != 0
            || plan.project.compound_window_requires_preparation(clip);
        if asset.compound_audio.is_some() {
            // A prepared transform can still construct a channel layout for
            // descendants. Count those descriptors even when the clip is muted.
            let cost = inspect_recipe(plan, asset.id, remaining, cancelled, depth + 1, audible)?;
            if audible {
                worst_child = worst_child.max(cost);
            }
            if !transformed {
                continue;
            }
        }
        if !audible {
            continue;
        }
        let pcm = plan
            .audio_for_clip(clip)
            .ok_or("Missing or unprepared compound member")?;
        if pcm.disk.as_ref().is_some_and(|disk| disk.failed()) {
            return Err("Compound source read failed".into());
        }
        let length = clip
            .repeat
            .map_or(clip.length_frames, |repeat| repeat.length_frames);
        if clip
            .source_offset_frame
            .checked_add(length)
            .is_none_or(|end| end > pcm.frame_count())
        {
            return Err("Incomplete compound member range".into());
        }
        if transformed
            && asset.compound_audio.is_some()
            && daw_media::prepared_clip_audio(
                clip,
                &plan.audio,
                &plan.stretched_audio,
                plan.project.stretch_engine,
            )
            .is_none()
        {
            return Err("Compound source transform is unprepared".into());
        }
    }
    // A frame can read the body and an internal transition. Readiness and
    // sampling each walk those paths, including nested readers.
    Ok(4 + 2 * worst_child.min(128_000_000))
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
