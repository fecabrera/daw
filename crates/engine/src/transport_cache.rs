//! Worker-owned checkpoints before latency priming. Never clone DSP on the callback.
use super::{CacheProfile, RenderPlan, Renderer, TransportState};
use std::collections::VecDeque;

// Leave four renderer equivalents for base, warm cursor, active and candidate.
const PREPARATION_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct TransportCacheMemory {
    pub renderer: super::RendererMemory,
    pub checkpoint_bytes: u64,
    pub checkpoint_count: usize,
    pub checkpoint_limit: usize,
    pub warming_bytes: u64,
    pub covered_frame: u64,
    pub replayed_frames: u64,
}

/// Exact history for one immutable processing plan. Resource profiles may change
/// without invalidating DSP snapshots.
/// PCM remains shared; processor, note, modulation, and delay state are independent.
/// Checkpoints never contain attached meter handles.
pub struct TransportCache {
    profile: CacheProfile,
    base: Renderer,
    checkpoints: VecDeque<(u64, Renderer)>,
    live: bool,
    history: bool,
    interval: u64,
    checkpoint_limit: usize,
    renderer_memory: super::RendererMemory,
    warming: Option<(u64, Renderer)>,
    covered_frame: u64,
    replayed_frames: u64,
}
impl TransportCache {
    pub fn new(plan: RenderPlan) -> Self {
        Self::with_profile(plan, CacheProfile::default())
    }
    pub fn with_profile(plan: RenderPlan, profile: CacheProfile) -> Self {
        let mut base = Renderer::new(plan);
        base.reset_processing();
        let history = base
            .midi
            .iter()
            .flatten()
            .any(|m| m.needs_envelope_history());
        let renderer_memory = base.memory_usage();
        let mut cache = Self {
            profile,
            base,
            checkpoint_limit: 0,
            renderer_memory,
            warming: None,
            covered_frame: 0,
            replayed_frames: 0,
            checkpoints: VecDeque::new(),
            live: true,
            history,
            interval: 1,
        };
        cache.configure();
        cache
    }
    fn configure(&mut self) {
        let policy = self.profile.policy();
        let owned = self.renderer_memory.owned_bytes().max(1);
        let entry = owned.saturating_add(2 * std::mem::size_of::<(u64, Renderer)>() as u64);
        self.checkpoint_limit = policy
            .max_checkpoints
            .min((policy.checkpoint_bytes / entry) as usize)
            .min((PREPARATION_BYTES / owned).saturating_sub(4) as usize);
        let minimum = (u128::from(self.base.plan.project.sample_rate_hz)
            * policy.minimum_interval.as_nanos()
            / 1_000_000_000)
            .max(1) as u64;
        self.interval = minimum.max(
            self.base
                .end_frame
                .div_ceil(self.checkpoint_limit.max(1) as u64),
        );
    }
    /// Retain exact snapshots across policy changes. Shrinking samples existing
    /// coverage evenly; increasing density rebuilds optional coverage on the worker.
    pub fn set_profile(&mut self, profile: CacheProfile) {
        if self.profile == profile {
            return;
        }
        let previous_interval = self.interval;
        self.profile = profile;
        self.configure();
        let old_len = self.checkpoints.len();
        let keep = self.checkpoint_limit;
        if old_len > keep {
            self.checkpoints
                .make_contiguous()
                .sort_unstable_by_key(|(frame, _)| *frame);
            let mut index = 0;
            self.checkpoints.retain(|_| {
                let selected = index * keep / old_len != (index + 1) * keep / old_len;
                index += 1;
                selected
            });
            self.checkpoints.shrink_to_fit();
        }
        if self.checkpoint_limit == 0 || self.interval < previous_interval {
            self.warming = None;
            self.covered_frame = 0;
        }
    }
    fn start(&self, frame: u64) -> (u64, Renderer) {
        self.checkpoints
            .iter()
            .filter(|(at, _)| *at <= frame)
            .max_by_key(|(at, _)| *at)
            .map_or_else(|| (0, self.base.clone()), |(at, r)| (*at, r.clone()))
    }
    fn replay(
        &mut self,
        renderer: &mut Renderer,
        from: u64,
        to: u64,
        cancelled: &mut impl FnMut() -> bool,
    ) -> bool {
        let block = u64::from(renderer.plan.project.buffer_size_frames.max(1));
        for frame in from..to {
            if frame % block == 0 && cancelled() {
                return false;
            }
            renderer.offline_frame(frame, self.live);
            let next = frame + 1;
            if self.checkpoint_limit > 0
                && next.is_multiple_of(self.interval)
                && !self.checkpoints.iter().any(|(at, _)| *at == next)
            {
                if self.checkpoints.len() == self.checkpoint_limit {
                    self.checkpoints.pop_front();
                }
                self.checkpoints.push_back((next, renderer.clone()));
            }
        }
        !cancelled()
    }
    /// Reuse only complete snapshots. Cancelled work cannot publish a partial renderer.
    pub fn prepare(
        &mut self,
        state: TransportState,
        mut cancelled: impl FnMut() -> bool,
    ) -> Option<Renderer> {
        let live = state.playing || state.paused;
        if self.live != live {
            self.live = live;
            self.checkpoints.clear();
            self.warming = None;
            self.covered_frame = 0;
        }
        if !self.history {
            let mut renderer = self.base.clone();
            return renderer
                .prepare_transport_cancellable(state, cancelled)
                .then_some(renderer);
        }
        let (from, mut renderer) = self.start(state.frame);
        self.replayed_frames = state.frame - from;
        if !self.replay(&mut renderer, from, state.frame, &mut cancelled) {
            return None;
        }
        renderer.playhead = state.frame;
        renderer.playing = state.playing && renderer.end_frame > 0;
        renderer.paused = state.paused;
        renderer.playback_start = state.playback_start;
        renderer.prime_latency(live);
        Some(renderer)
    }
    pub fn memory_usage(&self) -> TransportCacheMemory {
        TransportCacheMemory {
            renderer: self.renderer_memory,
            checkpoint_bytes: self
                .renderer_memory
                .owned_bytes()
                .saturating_mul(self.checkpoints.len() as u64)
                .saturating_add(
                    self.checkpoints.capacity() as u64
                        * std::mem::size_of::<(u64, Renderer)>() as u64,
                ),
            checkpoint_count: self.checkpoints.len(),
            checkpoint_limit: self.checkpoint_limit,
            warming_bytes: if self.warming.is_some() {
                self.renderer_memory.owned_bytes()
            } else {
                0
            },
            covered_frame: self.covered_frame,
            replayed_frames: self.replayed_frames,
        }
    }
    pub fn can_warm(&self) -> bool {
        self.history && self.checkpoint_limit > 0 && self.covered_frame < self.base.end_frame
    }
    /// Optional work advances one exact cursor through the arrangement. Yield by
    /// wall time and at request boundaries; completed checkpoints remain reusable.
    pub fn warm_step(&mut self, budget: std::time::Duration, mut cancelled: impl FnMut() -> bool) {
        if !self.can_warm() || cancelled() {
            return;
        }
        let (mut frame, mut renderer) = self
            .warming
            .take()
            .unwrap_or_else(|| self.start(self.covered_frame));
        let started = std::time::Instant::now();
        let block = u64::from(self.base.plan.project.buffer_size_frames.max(1));
        while frame < self.base.end_frame && started.elapsed() < budget && !cancelled() {
            let end = frame.saturating_add(block).min(self.base.end_frame);
            // No cancellation inside this block: retain a coherent cursor and
            // check the newest request before starting another bounded block.
            self.replay(&mut renderer, frame, end, &mut || false);
            frame = end;
        }
        self.covered_frame = frame;
        if frame < self.base.end_frame {
            self.warming = Some((frame, renderer));
        }
    }
    pub fn release_optional_memory(&mut self) {
        self.checkpoints.clear();
        self.checkpoints.shrink_to_fit();
        self.warming = None;
        self.covered_frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_changes_keep_valid_snapshots_and_trim_to_the_new_budget() {
        let plan = RenderPlan {
            project: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let mut cache = TransportCache::with_profile(plan, CacheProfile::Responsive);
        cache.interval = 32;
        let mut renderer = cache.base.clone();
        assert!(cache.replay(&mut renderer, 0, 4096, &mut || false));
        assert_eq!(cache.checkpoints.len(), 128);
        let frames: Vec<_> = cache.checkpoints.iter().map(|(at, _)| *at).collect();
        cache.set_profile(CacheProfile::Light);
        assert_eq!(cache.checkpoints.len(), 32);
        assert!(
            cache.memory_usage().checkpoint_bytes <= CacheProfile::Light.policy().checkpoint_bytes
        );
        assert!(cache.checkpoints.iter().all(|(at, _)| frames.contains(at)));
        assert_eq!(cache.start(4096).0, 4096);
        let retained: Vec<_> = cache.checkpoints.iter().map(|(at, _)| *at).collect();
        cache.covered_frame = 4096;
        cache.set_profile(CacheProfile::Responsive);
        assert_eq!(cache.covered_frame, 0);
        assert_eq!(
            cache
                .checkpoints
                .iter()
                .map(|(at, _)| *at)
                .collect::<Vec<_>>(),
            retained
        );
    }

    #[test]
    fn profiles_reduce_slots_for_large_renderers_instead_of_exceeding_byte_limits() {
        for profile in CacheProfile::ALL {
            let project = daw_core::Project {
                name: "x".repeat(4 * 1024 * 1024),
                ..Default::default()
            };
            let cache = TransportCache::with_profile(
                RenderPlan {
                    project,
                    audio: Default::default(),
                    sampler_audio: Default::default(),
                    stretched_audio: Default::default(),
                },
                profile,
            );
            assert!(cache.checkpoint_limit < profile.policy().max_checkpoints);
            assert!(
                cache.renderer_memory.owned_bytes() * cache.checkpoint_limit as u64
                    <= profile.policy().checkpoint_bytes
            );
        }
    }

    #[test]
    fn checkpoint_byte_policy_reserves_working_renderers_and_releases_optional_owners() {
        let mut project = daw_core::Project::default();
        let track = project.add_midi_track().unwrap();
        project.add_midi_pattern(track, 0).unwrap();
        let plan = RenderPlan {
            project,
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let mut cache = TransportCache::new(plan);
        cache.checkpoint_limit = 2;
        cache.interval = 1;
        let mut renderer = cache.base.clone();
        assert!(cache.replay(&mut renderer, 0, 10, &mut || false));
        assert_eq!(cache.memory_usage().checkpoint_count, 2);
        assert!(
            cache.memory_usage().checkpoint_bytes
                <= CacheProfile::Balanced.policy().checkpoint_bytes
        );
        cache.release_optional_memory();
        assert_eq!(cache.memory_usage().checkpoint_bytes, 0);
        assert_eq!(cache.memory_usage().warming_bytes, 0);
        assert_eq!(cache.start(10).0, 0);
    }

    #[test]
    fn idle_warming_cancels_at_a_block_and_resumes_from_complete_history() {
        let plan = RenderPlan {
            project: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let mut cache = TransportCache::new(plan);
        cache.history = true;
        cache.base.end_frame = 2048;
        cache.interval = 512;
        let mut checks = 0;
        cache.warm_step(std::time::Duration::from_secs(1), || {
            checks += 1;
            checks > 2
        });
        assert_eq!(cache.covered_frame, 512);
        assert_eq!(cache.start(512).0, 512);
        cache.warm_step(std::time::Duration::from_secs(1), || false);
        assert_eq!(cache.covered_frame, 2048);
        assert!(!cache.can_warm());
        assert_eq!(cache.memory_usage().warming_bytes, 0);
        assert_eq!(cache.start(2048).0, 2048);
    }

    #[test]
    fn checkpoints_are_bounded_and_cancelled_warming_keeps_completed_states() {
        let plan = RenderPlan {
            project: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let mut cache = TransportCache::new(plan);
        // A one-frame spacing stresses eviction without a long DSP fixture.
        cache.interval = 1;
        let mut renderer = cache.base.clone();
        assert!(cache.replay(&mut renderer, 0, 100, &mut || false));
        assert_eq!(
            cache.checkpoints.len(),
            CacheProfile::Balanced.policy().max_checkpoints
        );
        assert_eq!(cache.start(80).0, 80);
        assert_eq!(cache.start(10).0, 0);
        let before = cache.checkpoints.len();
        assert!(!cache.replay(&mut renderer, 512, 1024, &mut || true));
        assert_eq!(cache.checkpoints.len(), before);
        assert_eq!(cache.start(100).0, 100);
    }
}
