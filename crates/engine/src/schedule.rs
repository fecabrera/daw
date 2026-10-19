//! Prepared non-overlapping ranges. Saved clip order is independent of playback order.
use super::{RenderPlan, sample_fade};
use daw_core::Track;
use daw_media::AudioData;

#[derive(Clone)]
pub(super) struct Ranges {
    sorted: Vec<(u64, u64, usize)>,
    window: (u64, u64, Option<usize>),
}
impl Ranges {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.sorted)
    }

    pub fn new(ranges: impl Iterator<Item = (u64, u64)>) -> Self {
        let mut sorted: Vec<_> = ranges.enumerate().map(|(i, (a, b))| (a, b, i)).collect();
        sorted.sort_unstable_by_key(|r| r.0);
        Self {
            sorted,
            window: (0, 0, None),
        }
    }
    pub fn at(&mut self, frame: u64) -> Option<usize> {
        if frame >= self.window.0 && frame < self.window.1 {
            return self.window.2;
        }
        let next = self.sorted.partition_point(|r| r.0 <= frame);
        let previous = next.checked_sub(1).map(|i| self.sorted[i]);
        self.window = match previous {
            Some((start, end, index)) if frame < end => (start, end, Some(index)),
            _ => (
                previous.map_or(0, |r| r.1),
                self.sorted.get(next).map_or(u64::MAX, |r| r.0),
                None,
            ),
        };
        self.window.2
    }
}

#[derive(Clone)]
pub(super) struct AudioTrack {
    ranges: Ranges,
    audio: Vec<Option<AudioData>>,
    rate: u32,
}
impl AudioTrack {
    pub fn heap_bytes(&self) -> u64 {
        self.ranges.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.audio)
            + self
                .audio
                .iter()
                .flatten()
                .map(|a| a.metadata.sample_format.capacity() as u64)
                .sum::<u64>()
    }

    pub fn new(plan: &RenderPlan, track: &Track) -> Self {
        Self {
            ranges: Ranges::new(track.clips.iter().map(|c| (c.start_frame, c.end()))),
            audio: track
                .clips
                .iter()
                .map(|c| plan.audio_for_clip(c).cloned())
                .collect(),
            rate: plan.project.sample_rate_hz,
        }
    }
    pub fn sample(&mut self, track: &Track, frame: u64) -> ([f32; 2], Option<bool>) {
        let Some(index) = self.ranges.at(frame) else {
            return ([0.0; 2], None);
        };
        let Some(audio) = &self.audio[index] else {
            return ([0.0; 2], None);
        };
        let clip = &track.clips[index];
        let local = frame - clip.start_frame;
        let position = clip.source_frame(local);
        let fade = sample_fade(clip, local, position, self.rate);
        (
            audio
                .samples
                .get(position as usize)
                .copied()
                .unwrap_or_default()
                .map(|v| v * fade),
            Some(audio.metadata.channels == 1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsorted_ranges_support_gaps_exclusive_ends_and_arbitrary_seeks() {
        let ranges = [(30, 40), (10, 20), (20, 25)];
        let mut index = Ranges::new(ranges.into_iter());
        for frame in (0..45)
            .chain((0..45).rev())
            .chain([40, 20, 10, u64::MAX, 25, 0])
        {
            assert_eq!(
                index.at(frame),
                ranges.iter().position(|(a, b)| *a <= frame && frame < *b)
            );
        }
    }
}
