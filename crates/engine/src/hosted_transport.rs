//! Hosted graphs keep evaluated block state separate from the consumed cursor.
use super::{Renderer, TransportState, hosted_graph::HostedGraph, latency};
impl Renderer {
    fn prime_hosted(&mut self, graph: &mut HostedGraph, live: bool, realtime: bool) {
        if graph.primed {
            return;
        }
        let mut offset = 0;
        while offset < self.alignment.total {
            let start = latency::advanced_frame(self.playhead, offset, self.loop_region(live));
            let count = self.hosted_quantum(
                graph,
                start,
                ((self.alignment.total - offset) as usize).min(graph.quantum),
                live,
            );
            graph.render(self, start, count, live, true, realtime);
            offset += count as u64;
        }
        graph.count = 0;
        graph.consumed = 0;
        graph.primed = true;
    }
    pub(super) fn hosted_quantum(
        &self,
        graph: &HostedGraph,
        frame: u64,
        maximum: usize,
        live: bool,
    ) -> usize {
        let mut count = maximum.min(graph.quantum).max(1);
        let region = self.loop_region(live);
        for delay in &graph.clock_delays {
            let content = latency::delayed_frame(frame, *delay, region);
            if let Some((_, end)) = region {
                count = count.min((end.saturating_sub(content)).max(1) as usize);
            }
            // Every native call has a constant transport at its own delayed
            // input clock, including latency-shifted loop/tempo/signature edges.
            for boundary in self
                .plan
                .project
                .timing
                .tempos
                .iter()
                .map(|e| e.start_frame)
                .chain(
                    self.plan
                        .project
                        .timing
                        .signatures
                        .iter()
                        .map(|e| e.start_frame),
                )
            {
                if boundary > content {
                    count = count.min((boundary - content) as usize);
                }
            }
        }
        count.max(1)
    }
    pub(super) fn hosted_offline(&mut self, out: &mut [[f32; 2]]) {
        let mut graph = self.hosted.take().expect("hosted graph");
        self.prime_hosted(&mut graph, false, false);
        let mut position = 0;
        while position < out.len() {
            let start = self.playhead.saturating_add(self.alignment.total);
            let count = self.hosted_quantum(&graph, start, out.len() - position, false);
            graph.render(self, start, count, false, true, false);
            for (target, value) in out[position..position + count]
                .iter_mut()
                .zip(&graph.output)
            {
                *target = value.map(|v| v * self.master_target);
            }
            self.playhead = self.playhead.saturating_add(count as u64);
            position += count;
        }
        graph.count = 0;
        graph.consumed = 0;
        self.hosted = Some(graph);
    }
    pub(super) fn hosted_sample(&mut self) -> [f32; 2] {
        let mut graph = self.hosted.take().expect("hosted graph");
        if self.plan.external.publish {
            graph.activate();
        }

        let monitor = self.has_live_midi_routes() || self.live_midi_active();
        if !self.playing && !monitor {
            graph.flush();
            self.hosted = Some(graph);
            return self.hosted_finish(self.latent_held_mix, false);
        }
        if self.playing
            && let Some((first, end)) = self.loop_region(true)
            && (self.playhead < first || self.playhead >= end)
        {
            self.playhead = first;
            graph.invalidate();
            self.transition_from = self.previous;
            self.transition = (self.plan.project.sample_rate_hz / 200).max(1);
        }
        if self.playing {
            self.prime_hosted(&mut graph, true, true);
        }
        if graph.consumed >= graph.count {
            let start = if self.playing {
                latency::advanced_frame(self.playhead, self.alignment.total, self.loop_region(true))
            } else {
                self.live_clock
            };
            let count = self.hosted_quantum(&graph, start, graph.quantum, true);
            let region = self.loop_region(self.playing);
            let mut ready = self.streaming_ready(start, region);
            for tap in graph.taps_mut() {
                ready &= tap.ready(start, region);
            }
            if !ready {
                self.buffering = true;
                self.hosted = Some(graph);
                return [0.0; 2];
            }
            self.buffering = false;
            graph.render(self, start, count, true, self.playing, true);
        }
        let mut sample = graph.output[graph.consumed];
        graph.consumed += 1;
        self.latent_held_mix = sample;
        sample = self.hosted_finish(sample, monitor);
        if self.playing {
            self.playhead = self.playhead.saturating_add(1);
            if let Some((start, end)) = self.loop_region(true)
                && self.playhead >= end
            {
                self.playhead = start;
            }
        } else {
            self.live_clock = self.live_clock.wrapping_add(1);
        }
        self.hosted = Some(graph);
        sample
    }
    fn hosted_finish(&mut self, mut sample: [f32; 2], monitor: bool) -> [f32; 2] {
        let fade = (self.plan.project.sample_rate_hz / 200).max(1) as f32;
        let target = if self.playing || monitor { 1.0 } else { 0.0 };
        self.ramp += (target - self.ramp).clamp(-1.0 / fade, 1.0 / fade);
        self.advance_live_master();
        sample = sample.map(|v| {
            v * self.master
                * if !self.playing && monitor {
                    1.0
                } else {
                    self.ramp
                }
        });
        self.finish_live_sample(sample, fade)
    }
    pub(super) fn prepare_hosted_transport(
        &mut self,
        state: TransportState,
        cancelled: &mut impl FnMut() -> bool,
    ) -> bool {
        self.playhead = 0;
        self.playing = false;
        self.paused = false;
        let mut scratch = [[0.0; 2]; 4096];
        self.suspend_analysis(true);
        while self.playhead < state.frame {
            if cancelled() {
                self.suspend_analysis(false);
                return false;
            }
            let count = (state.frame - self.playhead).min(scratch.len() as u64) as usize;
            self.hosted_offline(&mut scratch[..count]);
        }
        self.suspend_analysis(false);
        self.playing = state.playing;
        self.paused = state.paused;
        self.playback_start = state.playback_start;
        !cancelled()
    }
}
