//! Prepared alignment delays. No allocation on seek, loop, retarget, or sample processing.
use daw_core::{EffectInstance, EffectSettings, Id, Project};

pub(super) type Sample = ([f32; 2], bool);
pub(super) const MAX_LIVE_DELAY_COPY: usize = 4096;

#[derive(Clone)]
struct DelaySplice {
    samples: Vec<Sample>,
    skip: usize,
    cursor: usize,
    count: usize,
}

#[derive(Clone)]
pub(super) struct Delay {
    pub(super) id: Id,
    samples: Vec<Sample>,
    index: usize,
    last_input: Sample,
    splice: Option<Box<DelaySplice>>,
}
impl Delay {
    pub fn frames(&self) -> usize {
        self.samples.len()
    }
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.samples)
            + self.splice.as_ref().map_or(0, |splice| {
                size_of::<DelaySplice>() as u64 + daw_unit_core::memory::vec_bytes(&splice.samples)
            })
    }

    pub fn new(id: Id, frames: u64) -> Self {
        Self {
            id,
            samples: vec![([0.0; 2], false); frames as usize],
            index: 0,
            last_input: ([0.0; 2], false),
            splice: None,
        }
    }
    pub fn reset(&mut self) {
        self.samples.fill(([0.0; 2], false));
        self.index = 0;
        self.last_input = ([0.0; 2], false);
        if let Some(splice) = &mut self.splice {
            splice.skip = 0;
            splice.count = 0;
            splice.cursor = 0;
        }
    }

    /// Set the cursor of an empty ring after a linear prefix of this length.
    pub fn skip_reset_prefix(&mut self, frames: u64) {
        if !self.samples.is_empty() {
            self.index = (frames % self.samples.len() as u64) as usize;
        }
    }

    pub fn tick(&mut self, mut sample: Sample) -> Sample {
        if let Some(splice) = &mut self.splice
            && splice.cursor < splice.count
        {
            if splice.skip > 0 {
                splice.skip -= 1;
            } else {
                sample = splice.samples[splice.cursor];
                splice.cursor += 1;
            }
        }
        self.last_input = sample;
        if self.samples.is_empty() {
            return sample;
        }
        let output = self.samples[self.index];
        self.samples[self.index] = sample;
        self.index = (self.index + 1) % self.samples.len();
        output
    }
    pub fn prepare_splice(&mut self, frames: usize) {
        self.splice = Some(Box::new(DelaySplice {
            samples: vec![([0.0; 2], false); frames],
            skip: 0,
            cursor: 0,
            count: 0,
        }));
    }
    fn splicing(&self) -> bool {
        self.splice.as_ref().is_some_and(|s| s.cursor < s.count)
    }
    pub fn live_copy_frames(&self, old: &Self, clock_growth: usize) -> Option<usize> {
        if old.splicing() {
            return None;
        }
        if self.frames() == old.frames() && clock_growth == 0 {
            return Some(0);
        }
        let kept = self.frames().saturating_sub(clock_growth);
        let pending = old.frames().checked_sub(kept)?;
        (pending == 0
            || self
                .splice
                .as_ref()
                .is_some_and(|s| s.samples.len() >= pending))
        .then_some(old.frames())
    }
    pub fn inherit(&mut self, old: &mut Self, clock_growth: usize) {
        self.last_input = old.last_input;
        if self.frames() == old.frames() && clock_growth == 0 {
            std::mem::swap(&mut self.samples, &mut old.samples);
            self.index = old.index;
            if old.splicing() {
                std::mem::swap(&mut self.splice, &mut old.splice);
            }
            return;
        }
        let (first, last) = old.samples.split_at(old.index);
        let mut history = last.iter().chain(first).copied();
        let new_frames = self.frames();
        let old_frames = old.frames();
        // Added leading frames are consumed while the playback clock holds.
        let leading = clock_growth.min(new_frames);
        for (slot, value) in self.samples[leading..].iter_mut().zip(&mut history) {
            *slot = value;
        }
        if old_frames > new_frames - leading {
            // A new effect replaces the delay removed from this line. Use
            // pending old audio until the first fresh effect output arrives.
            let splice = self.splice.as_mut().expect("prepared delay splice");
            splice.skip = clock_growth - leading;
            splice.cursor = 0;
            splice.count = old_frames - (new_frames - leading);
            for (slot, value) in splice.samples.iter_mut().zip(history) {
                *slot = value;
            }
        }
    }

    pub fn removal_copy_frames(&self, old: &Self, clock_drop: usize) -> Option<usize> {
        if old.splicing() {
            return None;
        }
        Some(if self.frames() == old.frames() && clock_drop == 0 {
            0
        } else {
            self.frames()
        })
    }

    pub fn inherit_removal(&mut self, old: &mut Self, clock_drop: usize) {
        if self.frames() == old.frames() && clock_drop == 0 {
            self.inherit(old, 0);
            return;
        }
        self.index = 0;
        self.last_input = old.last_input;
        let (first, last) = old.samples.split_at(old.index);
        // The source cursor stays put. Keep the samples that still fall inside
        // the new delay and bridge any unavailable interval with the last input.
        let history = last.iter().chain(first).skip(clock_drop).copied();
        for (target, frame) in self
            .samples
            .iter_mut()
            .zip(history.chain(std::iter::repeat(self.last_input)))
        {
            *target = frame;
        }
    }
}

#[cfg(test)]
#[test]
fn live_effect_splice_retains_pending_audio_until_the_new_effect_fills() {
    const SATURATOR_DELAY: usize = 32;
    let id = Id::new_v4();
    for (old_frames, growth) in [(16, 16), (32, 0), (49, 0)] {
        let mut old = Delay::new(id, old_frames);
        let mut new = Delay::new(id, old_frames + growth - SATURATOR_DELAY as u64);
        new.prepare_splice(SATURATOR_DELAY);
        // Wrap the old ring before handing it off.
        for i in 0..old_frames * 2 {
            old.tick(([i as f32; 2], true));
        }
        new.inherit(&mut old, growth as usize);
        for tick in 0..96 {
            let fresh = if tick < SATURATOR_DELAY as u64 {
                ([0.0; 2], false)
            } else {
                (
                    [(old_frames * 2 + tick - SATURATOR_DELAY as u64) as f32; 2],
                    true,
                )
            };
            let actual = new.tick(fresh);
            if tick >= growth {
                assert_eq!(actual, ([(old_frames + tick - growth) as f32; 2], true));
            }
        }
    }
}

#[cfg(test)]
#[test]
fn live_removal_retains_the_next_frames_at_the_shorter_clock() {
    let id = Id::new_v4();
    for (new_size, drop, pending) in [(3, 2, [4.0, 5.0, 6.0]), (6, 0, [2.0, 3.0, 4.0])] {
        let mut old = Delay::new(id, 5);
        for index in 0..7 {
            old.tick(([index as f32; 2], true));
        }
        let mut new = Delay::new(id, new_size);
        new.tick(([99.0; 2], true)); // A prepared candidate can have a nonzero ring cursor.
        new.inherit_removal(&mut old, drop);
        for frame in pending {
            assert_eq!(new.tick(([7.0; 2], true)), ([frame; 2], true));
        }
        if new_size > 3 {
            for frame in [5.0, 6.0, 6.0] {
                assert_eq!(new.tick(([7.0; 2], true)), ([frame; 2], true));
            }
        }
    }
}

pub(super) fn track_frames(track: &daw_core::Track, rate: u32) -> u64 {
    chain_frames(&track.effects, rate)
        + track
            .instrument
            .as_ref()
            .map_or(0, |i| super::layer_effects::latency(i, rate))
}
pub(super) fn chain_frames(effects: &[EffectInstance], rate: u32) -> u64 {
    effects
        .iter()
        .map(|e| match &e.unit {
            EffectSettings::Hosted(s) => u64::from(s.latency_frames),
            EffectSettings::Limiter(_) => u64::from(daw_limiter::Limiter::latency_for_rate(rate)),
            EffectSettings::HarmonicLift(_) => {
                u64::from(daw_harmonic_lift::HarmonicLift::LATENCY_FRAMES)
            }
            EffectSettings::Exciter(_) => u64::from(daw_exciter::Exciter::LATENCY_FRAMES),
            EffectSettings::Saturator(_) => u64::from(daw_saturator::Saturator::LATENCY_FRAMES),
            _ => 0,
        })
        .sum()
}
#[derive(Clone)]
pub(super) struct Alignment {
    pub tracks: Vec<Delay>,
    pub input_frames: Vec<u64>,
    pub node_frames: Vec<u64>,
    pub captured_sends: Vec<(usize, usize)>,
    pub send_delays: Vec<(Id, usize, usize, u64)>,
    /// Feed-to-content delay after each track's effects and sibling alignment.
    pub output_frames: Vec<u64>,
    parents: Vec<(Id, Option<Id>, daw_core::TrackKind)>,
    index: std::collections::HashMap<Id, usize>,
    pub track_max: u64,
    pub total: u64,
    pub live: bool,
    pub previous_feed: Option<u64>,
}
impl Alignment {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.node_frames)
            + daw_unit_core::memory::vec_bytes(&self.send_delays)
            + daw_unit_core::memory::vec_bytes(&self.captured_sends)
            + daw_unit_core::memory::vec_bytes(&self.input_frames)
            + daw_unit_core::memory::vec_bytes(&self.output_frames)
            + daw_unit_core::memory::vec_bytes(&self.parents)
            + daw_unit_core::memory::vec_bytes(&self.tracks)
            + daw_unit_core::memory::map_bytes(&self.index)
            + self.tracks.iter().map(Delay::heap_bytes).sum::<u64>()
    }

    pub fn new(project: &Project) -> Self {
        let rate = project.sample_rate_hz;
        let index: std::collections::HashMap<_, _> = project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
        let captured_sends: Vec<_> = project
            .rendered_return_routes()
            .into_iter()
            .map(|(source, destination)| (index[&source], index[&destination]))
            .collect();
        let mut captured_sources = vec![Vec::new(); project.tracks.len()];
        for &(source, destination) in &captured_sends {
            captured_sources[destination].push(source);
        }
        let specifications = super::audio_sidechain::specs(project);
        let pcm = super::audio_sidechain::pcm_specs(project);
        let (subtree, input_frames, track_max) = if specifications.is_empty() && pcm.is_empty() {
            let mut subtree: Vec<_> = project
                .tracks
                .iter()
                .map(|t| track_frames(t, rate))
                .collect();
            let mut input_frames = vec![0; subtree.len()];
            let mut track_max = 0;
            for (i, t) in project.tracks.iter().enumerate().rev() {
                subtree[i] += input_frames[i];
                if let Some(parent) = t.group_id.and_then(|id| index.get(&id)) {
                    input_frames[*parent] = input_frames[*parent].max(subtree[i]);
                } else {
                    track_max = track_max.max(subtree[i]);
                }
            }
            // Return ingress is the longest incoming source/group path. Returns
            // cannot send or parent other tracks, so sources precede them in this DAG.
            for (r, t) in project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.kind == daw_core::TrackKind::Return)
            {
                input_frames[r] = project
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.sends.iter().any(|send| send.return_id == t.id))
                    .map(|(i, _)| subtree[i])
                    .chain(captured_sources[r].iter().map(|&i| subtree[i]))
                    .max()
                    .unwrap_or(0);
                subtree[r] = input_frames[r] + track_frames(t, rate);
            }
            track_max = project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id.is_none())
                .map(|(i, _)| subtree[i])
                .max()
                .unwrap_or(0);
            (subtree, input_frames, track_max)
        } else {
            let mut subtree = vec![0_u64; project.tracks.len()];
            let mut input_frames = vec![0; project.tracks.len()];
            for i in project
                .sidechain_order()
                .expect("validated audio/control graph")
            {
                let audio = project
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        t.group_id == Some(project.tracks[i].id)
                            || t.sends.iter().any(|s| s.return_id == project.tracks[i].id)
                    })
                    .map(|(s, _)| subtree[s])
                    .chain(captured_sources[i].iter().map(|&s| subtree[s]))
                    .max()
                    .unwrap_or(0);
                let control = specifications
                    .iter()
                    .filter(|s| s.destination == i)
                    .filter_map(|spec| {
                        let source = spec.route.sidechain.unwrap().track;
                        index
                            .get(&source)
                            .map(|&s| subtree[s].saturating_sub(spec.stage))
                    })
                    .max()
                    .unwrap_or(0);
                let pcm = pcm
                    .iter()
                    .filter(|s| s.destination == i)
                    .filter_map(|s| {
                        index
                            .get(&s.source)
                            .map(|&source| subtree[source].saturating_sub(s.stage))
                    })
                    .max()
                    .unwrap_or(0);
                input_frames[i] = audio.max(control).max(pcm);
                subtree[i] = input_frames[i] + track_frames(&project.tracks[i], rate);
            }
            let track_max = project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id.is_none())
                .map(|(i, _)| subtree[i])
                .max()
                .unwrap_or(0);
            (subtree, input_frames, track_max)
        };
        let send_delays = project
            .tracks
            .iter()
            .enumerate()
            .flat_map(|(i, t)| {
                t.sends
                    .iter()
                    .map(|s| {
                        let r = index[&s.return_id];
                        (s.id, i, r, input_frames[r] - subtree[i])
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let total = track_max + chain_frames(&project.master.effects, rate);
        let output_frames: Vec<_> = project
            .tracks
            .iter()
            .map(|t| {
                t.group_id
                    .and_then(|id| index.get(&id))
                    .map_or(track_max, |p| input_frames[*p])
            })
            .collect();
        let tracks: Vec<_> = project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| Delay::new(t.id, output_frames[i] - subtree[i]))
            .collect();
        Self {
            node_frames: subtree,
            send_delays,
            captured_sends,
            input_frames,
            output_frames,
            parents: project
                .tracks
                .iter()
                .map(|t| (t.id, t.group_id, t.kind))
                .collect(),
            index: tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect(),
            tracks,
            track_max,
            total,
            live: false,
            previous_feed: None,
        }
    }
    pub fn compatible(&self, old: &Self) -> bool {
        if self.captured_sends != old.captured_sends
            || self.send_delays.len() != old.send_delays.len()
            || !self.send_delays.iter().zip(&old.send_delays).all(|(a, b)| {
                a.0 == b.0
                    && self.parents[a.1].0 == old.parents[b.1].0
                    && self.parents[a.2].0 == old.parents[b.2].0
                    && a.3 == b.3
            })
        {
            return false;
        }
        // A routing change needs exact group history reconstructed on a worker.
        let grouped = |parents: &[(Id, Option<Id>, daw_core::TrackKind)]| {
            parents
                .iter()
                .any(|(_, p, g)| p.is_some() || super::send_routing::is_bus(*g))
        };
        if (grouped(&self.parents) || grouped(&old.parents)) && self.parents != old.parents {
            return false;
        }
        // Adding/removing ordinary tracks must not reset existing zero-latency DSP.
        if self.total == 0 && old.total == 0 {
            return true;
        }
        self.total == old.total
            && self.track_max == old.track_max
            && self.tracks.len() == old.tracks.len()
            && self.tracks.iter().all(|a| {
                old.index.get(&a.id).is_some_and(|&i| {
                    a.samples.len() == old.tracks[i].samples.len()
                        && self.input_frames[self.index[&a.id]] == old.input_frames[i]
                })
            })
    }
    pub fn live_copy_frames(&self, old: &Self, max_delay: u64) -> Option<usize> {
        if self.total.checked_sub(old.total)? > max_delay
            || self.parents != old.parents
            || self.captured_sends != old.captured_sends
            || self.send_delays != old.send_delays
        {
            return None;
        }
        self.tracks.iter().zip(&old.tracks).enumerate().try_fold(
            0,
            |cost, (i, (next, old_delay))| {
                let growth = self.output_frames[i].checked_sub(old.output_frames[i])?;
                Some(cost + next.live_copy_frames(old_delay, growth as usize)?)
            },
        )
    }
    pub fn removal_copy_frames(&self, old: &Self) -> Option<usize> {
        if self.total > old.total
            || self.parents != old.parents
            || self.captured_sends != old.captured_sends
            || self.send_delays != old.send_delays
        {
            return None;
        }
        self.tracks.iter().zip(&old.tracks).enumerate().try_fold(
            0,
            |cost, (i, (next, previous))| {
                let drop = old.output_frames[i].checked_sub(self.output_frames[i])?;
                Some(cost + next.removal_copy_frames(previous, drop as usize)?)
            },
        )
    }
    pub fn inherit(&mut self, old: &mut Self, removal: bool) {
        for (index, a) in self.tracks.iter_mut().enumerate() {
            if let Some(&previous) = old.index.get(&a.id) {
                let b = &mut old.tracks[previous];
                let next_clock = self.output_frames[index];
                let old_clock = old.output_frames[previous];
                if removal {
                    a.inherit_removal(b, (old_clock - next_clock) as usize);
                } else {
                    a.inherit(b, next_clock.saturating_sub(old_clock) as usize);
                }
            }
        }
        self.live = old.live;
        self.previous_feed = old.previous_feed;
    }
    pub fn reset(&mut self) {
        for delay in &mut self.tracks {
            delay.reset();
        }
        self.previous_feed = None;
    }
}
pub(super) fn wrapped_frame(frame: u64, region: Option<(u64, u64)>) -> u64 {
    if let Some((start, end)) = region
        && frame >= end
    {
        start + (frame - start) % (end - start)
    } else {
        frame
    }
}
pub(super) fn delayed_frame(frame: u64, delay: u64, region: Option<(u64, u64)>) -> u64 {
    if let Some((start, end)) = region
        && frame >= start
    {
        let position = (frame - start) % (end - start);
        let delay = delay % (end - start);
        if position >= delay {
            start + (position - delay)
        } else {
            end - (delay - position)
        }
    } else {
        frame.saturating_sub(delay)
    }
}

pub(super) fn advanced_frame(frame: u64, delay: u64, region: Option<(u64, u64)>) -> u64 {
    if let Some((start, end)) = region
        && frame >= start
    {
        start + ((u128::from(frame - start) + u128::from(delay)) % u128::from(end - start)) as u64
    } else {
        frame.saturating_add(delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loop_clock_advance_and_delay_are_inverse_even_near_the_frame_limit() {
        for start in [0, 1000, u64::MAX - 10_000] {
            for length in [1, 17, 4096] {
                let region = Some((start, start + length));
                for offset in [0, length / 2, length - 1] {
                    for delay in [0, 1, 5000, u64::MAX] {
                        let frame = start + offset;
                        let future = advanced_frame(frame, delay, region);
                        assert!((start..start + length).contains(&future));
                        assert_eq!(delayed_frame(future, delay, region), frame);
                    }
                }
            }
        }
    }
}
