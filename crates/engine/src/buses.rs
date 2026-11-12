//! Prepared routing and shared post-fader bus processing for playback, export and cache workers.
use super::*;
use daw_core::TrackKind;
use daw_unit_core::memory::vec_bytes;

#[derive(Clone)]
pub(super) struct Buses {
    pub children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    groups: Vec<usize>,
    pub raw: Vec<latency::Sample>,
    pub sends: super::send_routing::Sends,
    pub output: Vec<[f32; 2]>,
    pub(super) clip_gains: Vec<super::beat_gain::Envelope>,
}
impl Buses {
    pub fn new(plan: &RenderPlan, alignment: &latency::Alignment) -> Self {
        let project = &plan.project;
        let index: HashMap<_, _> = project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let mut buses = Self {
            clip_gains: project
                .tracks
                .iter()
                .map(|t| {
                    super::beat_gain::Envelope::new(
                        plan.beat_tracks
                            .get(&t.id)
                            .map_or(&[], |b| b.clip_gains.as_slice()),
                        project.sample_rate_hz,
                    )
                })
                .collect(),
            children: vec![Vec::new(); project.tracks.len()],
            roots: Vec::new(),
            groups: Vec::new(),
            raw: vec![([0.0; 2], false); project.tracks.len()],
            sends: super::send_routing::Sends::new(plan, alignment),
            output: vec![[0.0; 2]; project.tracks.len()],
        };
        for (i, t) in project.tracks.iter().enumerate() {
            if let Some(parent) = t.group_id.and_then(|id| index.get(&id)) {
                buses.children[*parent].push(i);
            } else {
                buses.roots.push(i);
            }
            if t.kind == TrackKind::Group {
                buses.groups.push(i);
            }
        }
        buses.groups.reverse();
        buses.groups.extend(
            project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.kind == TrackKind::Return)
                .map(|(i, _)| i),
        );
        buses
    }
    pub fn heap_bytes(&self) -> u64 {
        vec_bytes(&self.raw)
            + self.sends.heap_bytes()
            + vec_bytes(&self.clip_gains)
            + self
                .clip_gains
                .iter()
                .map(super::beat_gain::Envelope::heap_bytes)
                .sum::<u64>()
            + vec_bytes(&self.children)
            + self.children.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.roots)
            + vec_bytes(&self.groups)
            + vec_bytes(&self.output)
    }
}
impl TrackGains {
    fn apply(&mut self, source: [f32; 2], mono: bool, live: bool, ramp: f32) -> [f32; 2] {
        if live && self.remaining > 0 {
            for ch in 0..4 {
                self.value[ch] += (self.target[ch] - self.value[ch]) / self.remaining as f32;
            }
            self.remaining -= 1;
        }
        let gains = if live { self.value } else { self.target };
        let base = if mono { 0 } else { 2 };
        let sample = std::array::from_fn(|ch| source[ch] * gains[base + ch]);
        if live {
            for (ch, value) in sample.iter().enumerate() {
                self.peak[ch] = self.peak[ch].max((value * ramp).abs());
            }
        }
        sample
    }
}
impl Renderer {
    pub(super) fn sample_tracks(
        &mut self,
        frame: u64,
        region: Option<(u64, u64)>,
        playing: bool,
        live: bool,
        realtime: bool,
    ) {
        if self.sidechains.active() {
            self.sample_sidechain_graph(frame, region, playing, live, realtime);
            return;
        }
        for (index, track) in self.plan.project.tracks.iter().enumerate() {
            if super::send_routing::is_bus(track.kind) {
                continue;
            }
            let (sample, mono) = track_processing::TrackProcessor {
                sidechains: &[],
                sidechain_sources: None,
                audio: &mut self.audio_tracks[index],
                midi: &mut self.midi[index],
                effects: &mut self.effects[index],
                delay: &mut self.alignment.tracks[index],
                clips: &track.clips,
            }
            .sample_unaligned(
                frame,
                self.plan.project.tempo_bpm,
                region,
                playing,
                realtime,
            );
            self.buses.raw[index] = (sample, mono);
            let (sample, mono) = self.alignment.tracks[index].tick((sample, mono));
            self.buses.output[index] = self.gains[index].apply(sample, mono, live, self.ramp);
        }
    }
    pub(super) fn sample_buses(
        &mut self,
        frame: u64,
        region: Option<(u64, u64)>,
        live: bool,
        realtime: bool,
    ) -> [f32; 2] {
        if !self.sidechains.active() {
            self.buses.sends.advance(live);
            for position in 0..self.buses.groups.len() {
                let index = self.buses.groups[position];
                self.sample_bus(index, frame, region, live, realtime);
            }
        }
        let mut sum = [0.0; 2];
        for &root in &self.buses.roots {
            for (ch, value) in sum.iter_mut().enumerate() {
                *value += self.buses.output[root][ch];
            }
        }
        sum
    }
    fn sample_bus(
        &mut self,
        index: usize,
        frame: u64,
        region: Option<(u64, u64)>,
        live: bool,
        realtime: bool,
    ) {
        let mut sum = if self.plan.project.tracks[index].kind == TrackKind::Return {
            self.buses
                .sends
                .sum(index, &self.buses.raw, live, frame, region, realtime)
        } else {
            [0.0; 2]
        };
        for &child in &self.buses.children[index] {
            for (ch, value) in sum.iter_mut().enumerate() {
                *value += self.buses.output[child][ch];
            }
        }
        if let Some(runtime) = self.midi[index].as_mut() {
            runtime.sample_in_region(
                if self.sidechains.active() {
                    latency::delayed_frame(frame, self.alignment.input_frames[index], region)
                } else {
                    frame
                },
                region,
            );
        }
        let controls = (
            self.midi[index]
                .as_ref()
                .map_or(self.sidechains.values[index].as_slice(), |r| {
                    r.modulation_values()
                }),
            self.midi[index]
                .as_ref()
                .map(|r| r.modulation_sources())
                .or_else(|| {
                    self.sidechains
                        .active()
                        .then_some(&self.sidechains.indices[index])
                }),
        );
        let (sample, _) = self.effects[index].sample(
            sum,
            Some(false),
            latency::delayed_frame(frame, self.alignment.input_frames[index], region),
            self.plan.project.tempo_bpm,
            region,
            controls,
        );
        self.buses.raw[index] = (sample, false);
        let (sample, _) = self.alignment.tracks[index].tick((sample, false));
        let content_frame =
            latency::delayed_frame(frame, self.alignment.output_frames[index], region);
        let gain = self.buses.clip_gains[index].at(content_frame, region);
        self.buses.output[index] =
            self.gains[index].apply(sample.map(|v| v * gain), false, live, self.ramp);
    }
    fn sample_sidechain_graph(
        &mut self,
        frame: u64,
        region: Option<(u64, u64)>,
        playing: bool,
        live: bool,
        realtime: bool,
    ) {
        self.buses.sends.advance(live);
        for position in 0..self.sidechains.order.len() {
            let index = self.sidechains.order[position];
            let clock = latency::delayed_frame(frame, self.alignment.input_frames[index], region);
            let wrapped = self.sidechains.wrapped(index, clock, region);
            self.sidechains.tick(
                index,
                &self.buses.raw,
                &self.sidechain_post,
                frame,
                region,
                realtime,
            );
            if let Some(values) = &self.capture_controls {
                self.sidechains.values[index].copy_from_slice(&values[index]);
            }
            if let Some(runtime) = &mut self.midi[index] {
                if wrapped {
                    runtime.wrap_loop();
                }
                runtime.publish_sidechains(&self.sidechains.values[index]);
            }
            if super::send_routing::is_bus(self.plan.project.tracks[index].kind) {
                self.sample_bus(index, frame, region, live, realtime);
            } else {
                let input = self.alignment.input_frames[index];
                let clock = latency::delayed_frame(frame, input, region);
                let valid = frame >= input || region.is_some_and(|(start, _)| frame >= start);
                let (sample, mono) = if valid {
                    track_processing::TrackProcessor {
                        sidechains: &self.sidechains.values[index],
                        sidechain_sources: Some(&self.sidechains.indices[index]),
                        audio: &mut self.audio_tracks[index],
                        midi: &mut self.midi[index],
                        effects: &mut self.effects[index],
                        delay: &mut self.alignment.tracks[index],
                        clips: &self.plan.project.tracks[index].clips,
                    }
                    .sample_unaligned(
                        clock,
                        self.plan.project.tempo_bpm,
                        region,
                        playing,
                        realtime,
                    )
                } else {
                    ([0.0; 2], false)
                };
                self.buses.raw[index] = (sample, mono);
                let (sample, mono) = self.alignment.tracks[index].tick((sample, mono));
                self.buses.output[index] = self.gains[index].apply(sample, mono, live, self.ramp);
            }
            self.sidechain_post[index] = if live {
                self.gains[index].value
            } else {
                self.gains[index].target
            };
        }
    }
}
