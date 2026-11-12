//! Prepared auxiliary readers. A leaf uses the main clip's complete edit clock.
use super::*;
#[derive(Clone)]
pub(super) struct Tap {
    pub reader: schedule::AudioTrack,
    pub delay: latency::Delay,
    pub clock_delay: u64,
}
impl Tap {
    pub fn new(
        plan: &RenderPlan,
        track: &daw_core::Track,
        select: impl Fn(&daw_core::RenderedAudio) -> Option<Id>,
        clock_delay: u64,
        delay: u64,
    ) -> Option<Self> {
        let mut source = track.clone();
        source.clips = plan.project.rendered_clips(track, select);
        if source.clips.is_empty() {
            return None;
        }
        Some(Self {
            reader: schedule::AudioTrack::new(plan, &source),
            delay: latency::Delay::new(track.id, delay),
            clock_delay,
        })
    }
    pub fn sample(
        &mut self,
        frame: u64,
        region: Option<(u64, u64)>,
        realtime: bool,
    ) -> ([f32; 2], bool) {
        let sample = if frame < self.clock_delay && region.is_none() {
            ([0.0; 2], false)
        } else {
            let (value, mono) = self.reader.sample(
                &[],
                latency::delayed_frame(frame, self.clock_delay, region),
                realtime,
            );
            (value, mono.unwrap_or(false))
        };
        self.delay.tick(sample)
    }
    pub fn active_at(&mut self, frame: u64, region: Option<(u64, u64)>) -> bool {
        let delay = self.clock_delay + self.delay.frames() as u64;
        (frame >= delay || region.is_some())
            && self
                .reader
                .active_at(latency::delayed_frame(frame, delay, region))
    }
    pub fn ready(&mut self, frame: u64, region: Option<(u64, u64)>) -> bool {
        self.reader.ready(
            &[],
            latency::delayed_frame(frame, self.clock_delay, region),
            region.map(|r| r.0),
        )
    }
    pub fn heap_bytes(&self) -> u64 {
        self.reader.heap_bytes() + self.delay.heap_bytes()
    }
}
