//! Prepared post-effect gain. Gaps retain tails; loop gain follows the content clock.
#[derive(Clone)]
struct Change {
    start_frame: u64,
    end_frame: u64,
    from: f32,
    to: f32,
}

#[derive(Clone)]
pub(super) struct Envelope {
    changes: Vec<Change>,
    ramp_frames: u64,
}

impl Envelope {
    pub fn new(clips: &[(u64, u64, f32)], sample_rate_hz: u32) -> Self {
        let mut clips = clips.to_vec();
        clips.sort_unstable_by_key(|c| c.0);
        let mut result = Self {
            changes: Vec::with_capacity(clips.len()),
            ramp_frames: u64::from((sample_rate_hz / 200).max(1)),
        };
        for (start_frame, end_frame, to) in clips {
            // The first clip has no prior tail. Silence must be exact at note-on.
            let from = if result.changes.is_empty() || to == 0.0 {
                to
            } else {
                result.changes.last().unwrap().to
            };
            result.changes.push(Change {
                start_frame,
                end_frame,
                from,
                to,
            });
        }
        result
    }

    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.changes)
    }

    fn value(
        &self,
        change: &Change,
        frame: u64,
        start_frame: u64,
        from: f32,
        duration_frames: u64,
    ) -> f32 {
        let elapsed_frames = frame.saturating_sub(start_frame);
        if change.to == 0.0 || elapsed_frames >= duration_frames {
            return change.to;
        }
        let amount = elapsed_frames as f32 / duration_frames as f32;
        from + (change.to - from) * amount
    }

    fn raw(&self, frame: u64) -> f32 {
        self.changes
            .partition_point(|c| c.start_frame <= frame)
            .checked_sub(1)
            .map_or(1.0, |index| {
                let c = &self.changes[index];
                let duration_frames = self
                    .changes
                    .get(index + 1)
                    .map_or(self.ramp_frames, |next| {
                        self.ramp_frames.min(next.start_frame - c.start_frame)
                    });
                self.value(c, frame, c.start_frame, c.from, duration_frames)
            })
    }

    pub fn at(&self, frame: u64, region: Option<(u64, u64)>) -> f32 {
        let Some((start_frame, end_frame)) = region.filter(|(start, _)| frame >= *start) else {
            return self.raw(frame);
        };
        let next = self.changes.partition_point(|c| c.start_frame <= frame);
        let previous = next.checked_sub(1).map(|i| &self.changes[i]);
        let tail = self
            .changes
            .partition_point(|c| c.start_frame < end_frame)
            .checked_sub(1)
            .map_or(1.0, |index| self.changes[index].to);
        // A leading loop gap inherits the gain of the preceding loop iteration.
        let Some(c) =
            previous.filter(|c| c.start_frame >= start_frame || c.end_frame > start_frame)
        else {
            return tail;
        };
        let event = c.start_frame.max(start_frame);
        let first = next <= 1
            || (self.changes[next - 2].start_frame < start_frame
                && self.changes[next - 2].end_frame <= start_frame);
        let from = if c.start_frame < start_frame || first {
            tail
        } else {
            c.from
        };
        // Complete short transitions by the next clip or wrap, so each loop is periodic.
        let duration_frames = self
            .changes
            .get(next)
            .map_or(end_frame, |following| following.start_frame.min(end_frame))
            .saturating_sub(event)
            .min(self.ramp_frames);
        self.value(c, frame, event, from, duration_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaps_keep_gain_and_following_clips_transition_without_losing_silence() {
        let envelope = Envelope::new(
            &[(100, 200, 0.25), (1000, 1200, 0.5), (2000, 2200, 0.0)],
            48_000,
        );
        assert_eq!(envelope.at(99, None), 1.0);
        for frame in [100, 199, 200, 999] {
            assert_eq!(envelope.at(frame, None), 0.25);
        }
        assert_eq!(envelope.at(1120, None), 0.375);
        assert_eq!(envelope.at(1240, None), 0.5);
        for frame in [2000, 2100, 9000] {
            assert_eq!(envelope.at(frame, None), 0.0);
        }
    }
    #[test]
    fn settled_gain_retains_small_positive_values_exactly() {
        let envelope = Envelope::new(&[(0, 100, 1.0), (100, 200, 1e-10)], 48000);
        assert_eq!(envelope.at(340, None), 1e-10);
        assert_eq!(envelope.at(5000, None), 1e-10);
    }
    #[test]
    fn leading_loop_gap_retains_last_gain_and_lookup_is_seek_independent() {
        let envelope = Envelope::new(&[(500, 600, 0.25), (1000, 1200, 0.0)], 48_000);
        for frame in [100, 300, 499, 1000, 1500] {
            assert_eq!(envelope.at(frame, Some((100, 1700))), 0.0);
        }
        assert_eq!(envelope.at(740, Some((100, 1700))), 0.25);
        let short = Envelope::new(&[(0, 50, 0.25), (80, 85, 0.5), (90, 95, 0.75)], 48_000);
        let region = Some((70, 100));
        let cut = Envelope::new(&[(0, 85, 0.25), (90, 100, 0.75)], 48_000);
        assert_eq!(cut.at(70, region), 0.75);
        assert_eq!(cut.at(90, region), 0.25);
        assert_eq!(short.at(70, region), 0.75);
        assert_eq!(short.at(80, region), 0.75);
        assert_eq!(short.at(90, region), 0.5);
        assert!((short.at(99, region) - short.at(70, region)).abs() < 0.026);
        assert_eq!(short.at(95, region), 0.625);
        assert_eq!(short.at(85, region), 0.625);
        assert_eq!(short.at(75, region), 0.75);
    }
}
