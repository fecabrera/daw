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
    staged: Option<(usize, u64, [f32; 2])>,
    loop_pin: Option<(u64, Option<(usize, u64)>)>,
    readers: Vec<daw_media::FrameReader>,
    reader_indices: Vec<Option<usize>>,
}
impl AudioTrack {
    pub fn heap_bytes(&self) -> u64 {
        self.ranges.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.audio)
            + daw_unit_core::memory::vec_bytes(&self.readers)
            + daw_unit_core::memory::vec_bytes(&self.reader_indices)
            + self
                .readers
                .iter()
                .map(daw_media::FrameReader::heap_bytes)
                .sum::<u64>()
            + self
                .audio
                .iter()
                .flatten()
                .map(|a| a.metadata.sample_format.capacity() as u64)
                .sum::<u64>()
    }

    pub fn new(plan: &RenderPlan, track: &Track) -> Self {
        let mut sources = Vec::new();
        let mut readers = Vec::new();
        let reader_indices = track
            .clips
            .iter()
            .map(|clip| {
                if clip.muted {
                    return None;
                }
                let audio = plan.audio_for_clip(clip)?;
                let source = audio.disk.as_ref()?;
                let pointer = std::sync::Arc::as_ptr(source);
                if let Some(index) = sources.iter().position(|s| *s == pointer) {
                    return Some(index);
                }
                sources.push(pointer);
                readers.push(daw_media::FrameReader::new(audio));
                Some(readers.len() - 1)
            })
            .collect();
        Self {
            readers,
            reader_indices,
            staged: None,
            loop_pin: None,
            ranges: Ranges::new(track.clips.iter().map(|c| (c.start_frame, c.end()))),
            audio: track
                .clips
                .iter()
                .map(|c| {
                    if c.muted {
                        None
                    } else {
                        plan.audio_for_clip(c).cloned()
                    }
                })
                .collect(),
            rate: plan.project.sample_rate_hz,
        }
    }
    pub fn source_error(&self) -> Option<String> {
        self.readers.iter().find_map(|reader| reader.error())
    }
    pub fn source_failed(&self) -> bool {
        self.readers.iter().any(daw_media::FrameReader::failed)
    }
    pub fn prepare_streaming(&mut self) -> daw_media::Result<()> {
        for reader in &mut self.readers {
            reader.prepare_live()?;
        }
        Ok(())
    }
    pub fn ready(&mut self, clips: &[daw_core::Clip], frame: u64, loop_start: Option<u64>) -> bool {
        if let Some(start) = loop_start {
            if self.loop_pin.as_ref().is_none_or(|(old, _)| *old != start) {
                let pin = self.ranges.at(start).and_then(|i| {
                    self.reader_indices[i]
                        .map(|reader| (reader, clips[i].source_frame(start - clips[i].start_frame)))
                });
                self.loop_pin = Some((start, pin));
            }
        } else {
            self.loop_pin = None;
        }
        let Some(index) = self.ranges.at(frame) else {
            return true;
        };
        if self.audio[index].is_none() {
            return true;
        }
        let Some(reader) = self.reader_indices[index] else {
            return true;
        };
        let clip = &clips[index];
        let pin = self.loop_pin.and_then(|(_, pin)| pin);
        if let Some((other, position)) = pin
            && other != reader
        {
            self.readers[other].ready(position, Some(position));
        }
        let pinned = pin
            .filter(|(other, _)| *other == reader)
            .map(|(_, frame)| frame);
        let position = clip.source_frame(frame - clip.start_frame);
        let ready = self.readers[reader].ready(position, pinned);
        self.staged = self.readers[reader]
            .try_live_sample(position)
            .map(|value| (index, position, value));
        ready && self.staged.is_some()
    }
    pub fn sample(
        &mut self,
        clips: &[daw_core::Clip],
        frame: u64,
        realtime: bool,
    ) -> ([f32; 2], Option<bool>) {
        let Some(index) = self.ranges.at(frame) else {
            return ([0.0; 2], None);
        };
        let Some(audio) = &self.audio[index] else {
            return ([0.0; 2], None);
        };
        let clip = &clips[index];
        let local = frame - clip.start_frame;
        let position = clip.source_frame(local);
        let fade = sample_fade(clip, local, position, self.rate);
        let value = if let Some(reader) = self.reader_indices[index] {
            if realtime {
                self.staged
                    .filter(|(i, p, _)| *i == index && *p == position)
                    .map_or_else(
                        || self.readers[reader].live_sample(position),
                        |(_, _, value)| value,
                    )
            } else {
                self.readers[reader].sample(position).unwrap_or_default()
            }
        } else {
            audio
                .samples
                .get(position as usize)
                .copied()
                .unwrap_or_default()
        };
        (
            value.map(|v| v * fade * daw_core::linear_gain(clip.gain_db)),
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
