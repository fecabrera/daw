//! Prepared auxiliary edges. IDs are resolved and delay storage allocated by workers.
use super::*;
use daw_core::{SendTap, TrackKind};
use daw_unit_core::ModulationRamp as Ramp;

#[derive(Clone)]
struct SourceMix {
    values: [Ramp; 8],
    current: [f32; 8],
}
impl SourceMix {
    fn new(t: &daw_core::Track) -> Self {
        let gain = if t.gain_db <= daw_core::SILENT_GAIN_DB {
            0.0
        } else {
            linear_gain(t.gain_db)
        };
        let [mono, stereo] = super::track_processing::track_pan_coefficients(t);
        let current = [
            mono[0],
            mono[1],
            stereo[0],
            stereo[1],
            mono[0] * gain,
            mono[1] * gain,
            stereo[0] * gain,
            stereo[1] * gain,
        ];
        Self {
            values: current.map(Ramp::new),
            current,
        }
    }
    fn advance(&mut self, live: bool) {
        self.current = self
            .values
            .each_mut()
            .map(|r| if live { r.next_value() } else { r.target() });
    }
}
#[derive(Clone)]
struct Edge {
    source: usize,
    delay: latency::Delay,
    pre_delay: latency::Delay,
    level: Ramp,
    post: Ramp,
}
#[derive(Clone)]
pub(super) struct Sends {
    sources: Vec<SourceMix>,
    edges: Vec<Edge>,
    incoming: Vec<Vec<usize>>,
    frames: u32,
    pre_taps: Vec<Option<super::captured_taps::Tap>>,
}
impl Sends {
    pub fn new(plan: &RenderPlan, alignment: &latency::Alignment) -> Self {
        let project = &plan.project;
        let audible = project.track_send_audible();
        let mut sends = Self {
            pre_taps: project
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if t.sends.is_empty() {
                        None
                    } else {
                        super::captured_taps::Tap::new(
                            plan,
                            t,
                            |r| Some(r.pre_send_asset),
                            alignment.node_frames[i],
                            0,
                        )
                    }
                })
                .collect(),
            sources: if alignment.send_delays.is_empty() {
                Vec::new()
            } else {
                project.tracks.iter().map(SourceMix::new).collect()
            },
            edges: Vec::new(),
            incoming: vec![vec![]; project.tracks.len()],
            frames: (project.sample_rate_hz / 200).max(1),
        };
        for &(id, source, destination, delay) in &alignment.send_delays {
            let s = project.tracks[source]
                .sends
                .iter()
                .find(|s| s.id == id)
                .unwrap();
            let level = if s.enabled && audible[source] && s.gain_db > daw_core::SILENT_GAIN_DB {
                linear_gain(s.gain_db)
            } else {
                0.0
            };
            let i = sends.edges.len();
            sends.incoming[destination].push(i);
            sends.edges.push(Edge {
                source,
                delay: latency::Delay::new(id, delay),
                pre_delay: latency::Delay::new(id, delay),
                level: Ramp::new(level),
                post: Ramp::new(f32::from(s.tap == SendTap::PostFader)),
            });
        }
        sends
    }
    pub(super) fn taps_mut(&mut self) -> impl Iterator<Item = &mut super::captured_taps::Tap> {
        self.pre_taps.iter_mut().flatten()
    }
    pub(super) fn taps(&self) -> impl Iterator<Item = &super::captured_taps::Tap> {
        self.pre_taps.iter().flatten()
    }
    pub fn advance(&mut self, live: bool) {
        for s in &mut self.sources {
            s.advance(live);
        }
    }
    pub fn sum(
        &mut self,
        destination: usize,
        raw: &[latency::Sample],
        live: bool,
        frame: u64,
        region: Option<(u64, u64)>,
        realtime: bool,
    ) -> [f32; 2] {
        let mut sum = [0.0; 2];
        for &i in &self.incoming[destination] {
            let edge = &mut self.edges[i];
            let (sample, mono) = edge.delay.tick(raw[edge.source]);
            let gain = if live {
                edge.level.next_value()
            } else {
                edge.level.target()
            };
            let post = if live {
                edge.post.next_value()
            } else {
                edge.post.target()
            };
            let values = self.sources[edge.source].current;
            let pre = self.pre_taps[edge.source].as_mut().and_then(|tap| {
                let value = edge.pre_delay.tick(tap.sample(frame, region, realtime)).0;
                let clock = latency::delayed_frame(frame, edge.pre_delay.frames() as u64, region);
                tap.active_at(clock, region).then_some(value)
            });
            let base = if mono { 0 } else { 2 };
            for ch in 0..2 {
                // Captured pre-send audio already includes the original pan, but
                // never its fader. Keep the output track's new pan/level live.
                let pre = pre.map_or(sample[ch] * values[base + ch], |v| v[ch] * values[2 + ch]);
                sum[ch] += (pre * (1.0 - post) + sample[ch] * values[base + 4 + ch] * post) * gain;
            }
        }
        sum
    }
    /// Called only after Alignment has verified identical ordered nodes and edges.
    /// Route identity resolution stays on workers; inheritance uses dense indices.
    pub fn inherit(&mut self, old: &mut Self) {
        for (source, old_source) in self.sources.iter_mut().zip(&old.sources) {
            for (r, previous) in source.values.iter_mut().zip(old_source.values) {
                let target = r.target();
                *r = previous;
                r.set(target, self.frames);
            }
        }
        for (edge, previous) in self.edges.iter_mut().zip(&mut old.edges) {
            let target = edge.level.target();
            let post = edge.post.target();
            std::mem::swap(&mut edge.delay, &mut previous.delay);
            std::mem::swap(&mut edge.pre_delay, &mut previous.pre_delay);
            edge.level = previous.level;
            edge.post = previous.post;
            edge.level.set(target, self.frames);
            edge.post.set(post, self.frames);
        }
    }
    pub fn reset(&mut self) {
        for e in &mut self.edges {
            e.delay.reset();
            e.pre_delay.reset();
            e.level.reset();
            e.post.reset();
        }
        for s in &mut self.sources {
            for r in &mut s.values {
                r.reset();
            }
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        vec_bytes(&self.pre_taps)
            + self
                .pre_taps
                .iter()
                .flatten()
                .map(|t| t.heap_bytes())
                .sum::<u64>()
            + vec_bytes(&self.sources)
            + vec_bytes(&self.edges)
            + vec_bytes(&self.incoming)
            + self.incoming.iter().map(vec_bytes).sum::<u64>()
            + self
                .edges
                .iter()
                .map(|e| e.delay.heap_bytes() + e.pre_delay.heap_bytes())
                .sum::<u64>()
    }
}

pub(super) fn is_bus(kind: TrackKind) -> bool {
    matches!(kind, TrackKind::Group | TrackKind::Return)
}
