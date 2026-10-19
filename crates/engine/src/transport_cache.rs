//! Worker-owned checkpoints before latency priming. Never clone DSP on the callback.
use super::{RenderPlan, Renderer, TransportState};
use std::collections::VecDeque;

const MAX_CHECKPOINTS: usize = 64;

/// Exact history for one immutable plan. A new plan requires a new cache.
/// PCM remains shared; processor, note, modulation, and delay state are independent.
/// Checkpoints never contain attached meter handles.
pub struct TransportCache {
    base: Renderer,
    checkpoints: VecDeque<(u64, Renderer)>,
    live: bool,
    history: bool,
    interval: u64,
}
impl TransportCache {
    pub fn new(plan: RenderPlan) -> Self {
        let interval = (u64::from(plan.project.sample_rate_hz) / 4).max(1);
        let mut base = Renderer::new(plan);
        base.reset_processing();
        let history = base
            .midi
            .iter()
            .flatten()
            .any(|m| m.needs_envelope_history());
        Self {
            base,
            checkpoints: VecDeque::new(),
            live: false,
            history,
            interval,
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
            if next.is_multiple_of(self.interval)
                && !self.checkpoints.iter().any(|(at, _)| *at == next)
            {
                if self.checkpoints.len() == MAX_CHECKPOINTS {
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
        }
        if !self.history {
            let mut renderer = self.base.clone();
            return renderer
                .prepare_transport_cancellable(state, cancelled)
                .then_some(renderer);
        }
        let (from, mut renderer) = self.start(state.frame);
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
    /// Idle worker prefetch: at most eight seconds through the current loop, or four
    /// seconds ahead without a loop. New requests cancel this at a block boundary.
    pub fn warm(&mut self, frame: u64, mut cancelled: impl FnMut() -> bool) {
        if !self.history || cancelled() {
            return;
        }
        let rate = u64::from(self.base.plan.project.sample_rate_hz);
        let region = &self.base.plan.project.transport.r#loop;
        let end = if self.live && region.enabled {
            region.end_frame.min(frame.saturating_add(rate * 8))
        } else {
            self.base.end_frame.min(frame.saturating_add(rate * 4))
        };
        let (from, mut renderer) = self.start(end);
        self.replay(&mut renderer, from, end, &mut cancelled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(cache.checkpoints.len(), MAX_CHECKPOINTS);
        assert_eq!(cache.start(80).0, 80);
        assert_eq!(cache.start(10).0, 0);
        let before = cache.checkpoints.len();
        assert!(!cache.replay(&mut renderer, 512, 1024, &mut || true));
        assert_eq!(cache.checkpoints.len(), before);
        assert_eq!(cache.start(100).0, 100);
    }
}
