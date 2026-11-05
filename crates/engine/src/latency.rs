//! Prepared alignment delays. No allocation on seek, loop, retarget, or sample processing.
use daw_core::{EffectInstance, EffectSettings, Id, Project};

pub(super) type Sample = ([f32; 2], bool);
#[derive(Clone)]
pub(super) struct Delay {
    id: Id,
    samples: Vec<Sample>,
    index: usize,
}
impl Delay {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.samples)
    }

    pub fn new(id: Id, frames: u64) -> Self {
        Self {
            id,
            samples: vec![([0.0; 2], false); frames as usize],
            index: 0,
        }
    }
    pub fn reset(&mut self) {
        self.samples.fill(([0.0; 2], false));
        self.index = 0;
    }

    pub fn tick(&mut self, sample: Sample) -> Sample {
        if self.samples.is_empty() {
            return sample;
        }
        let output = self.samples[self.index];
        self.samples[self.index] = sample;
        self.index = (self.index + 1) % self.samples.len();
        output
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
        .map(|e| match e.unit {
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
        if self.send_delays.len() != old.send_delays.len()
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
    pub fn inherit(&mut self, old: &mut Self) {
        for a in &mut self.tracks {
            if let Some(&index) = old.index.get(&a.id) {
                let b = &mut old.tracks[index];
                std::mem::swap(&mut a.samples, &mut b.samples);
                a.index = b.index;
            }
        }
        self.live = old.live;
        self.previous_feed = old.previous_feed;
    }
    pub fn reset(&mut self) {
        for delay in &mut self.tracks {
            delay.samples.fill(([0.0; 2], false));
            delay.index = 0;
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
