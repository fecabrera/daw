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
            EffectSettings::Saturator(_) => u64::from(daw_saturator::Saturator::LATENCY_FRAMES),
            _ => 0,
        })
        .sum()
}
#[derive(Clone)]
pub(super) struct Alignment {
    pub tracks: Vec<Delay>,
    index: std::collections::HashMap<Id, usize>,
    pub track_max: u64,
    pub total: u64,
    pub live: bool,
    pub previous_feed: Option<u64>,
}
impl Alignment {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.tracks)
            + daw_unit_core::memory::map_bytes(&self.index)
            + self.tracks.iter().map(Delay::heap_bytes).sum::<u64>()
    }

    pub fn new(project: &Project) -> Self {
        let rate = project.sample_rate_hz;
        let track_max = project
            .tracks
            .iter()
            .map(|t| track_frames(t, rate))
            .max()
            .unwrap_or(0);
        let total = track_max + chain_frames(&project.master.effects, rate);
        let tracks: Vec<_> = project
            .tracks
            .iter()
            .map(|t| Delay {
                id: t.id,
                samples: vec![([0.0; 2], false); (track_max - track_frames(t, rate)) as usize],
                index: 0,
            })
            .collect();
        Self {
            index: tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect(),
            tracks,
            track_max,
            total,
            live: false,
            previous_feed: None,
        }
    }
    pub fn compatible(&self, old: &Self) -> bool {
        // Adding/removing ordinary tracks must not reset existing zero-latency DSP.
        if self.total == 0 && old.total == 0 {
            return true;
        }
        self.total == old.total
            && self.track_max == old.track_max
            && self.tracks.len() == old.tracks.len()
            && self.tracks.iter().all(|a| {
                old.index
                    .get(&a.id)
                    .is_some_and(|&i| a.samples.len() == old.tracks[i].samples.len())
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
