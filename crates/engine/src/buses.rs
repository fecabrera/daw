//! Prepared routing and shared post-fader bus processing for playback, export and cache workers.
use super::*;
use daw_core::TrackKind;
use daw_unit_core::memory::vec_bytes;

#[derive(Clone)]
pub(super) struct Buses {
    pub children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    groups: Vec<usize>,
    pub output: Vec<[f32; 2]>,
}
impl Buses {
    pub fn new(project: &Project) -> Self {
        let index: HashMap<_, _> = project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let mut buses = Self {
            children: vec![Vec::new(); project.tracks.len()],
            roots: Vec::new(),
            groups: Vec::new(),
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
        buses
    }
    pub fn heap_bytes(&self) -> u64 {
        vec_bytes(&self.children)
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
    ) {
        for (index, track) in self.plan.project.tracks.iter().enumerate() {
            if track.kind == TrackKind::Group {
                continue;
            }
            let (sample, mono) = track_processing::TrackProcessor {
                audio: &mut self.audio_tracks[index],
                midi: &mut self.midi[index],
                effects: &mut self.effects[index],
                delay: &mut self.alignment.tracks[index],
                clips: &track.clips,
            }
            .sample(frame, self.plan.project.tempo_bpm, region, playing);
            self.buses.output[index] = self.gains[index].apply(sample, mono, live, self.ramp);
        }
    }
    pub(super) fn sample_buses(
        &mut self,
        frame: u64,
        region: Option<(u64, u64)>,
        live: bool,
    ) -> [f32; 2] {
        for &index in &self.buses.groups {
            let mut sum = [0.0; 2];
            for &child in &self.buses.children[index] {
                for (ch, value) in sum.iter_mut().enumerate() {
                    *value += self.buses.output[child][ch];
                }
            }
            let (sample, _) = self.effects[index].sample(
                sum,
                Some(false),
                latency::delayed_frame(frame, self.alignment.input_frames[index], region),
                self.plan.project.tempo_bpm,
                region,
                (&[], None),
            );
            let (sample, _) = self.alignment.tracks[index].tick((sample, false));
            self.buses.output[index] = self.gains[index].apply(sample, false, live, self.ramp);
        }
        let mut sum = [0.0; 2];
        for &root in &self.buses.roots {
            for (ch, value) in sum.iter_mut().enumerate() {
                *value += self.buses.output[root][ch];
            }
        }
        sum
    }
}
