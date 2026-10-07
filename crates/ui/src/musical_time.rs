use daw_core::seconds;

pub const BEATS_PER_BAR: u64 = 4;

pub fn position(frame: u64, tempo_bpm: f32) -> (u64, u64) {
    let beat = (seconds(frame) * (f64::from(tempo_bpm) / 60.0)).floor() as u64;
    (beat / BEATS_PER_BAR + 1, beat % BEATS_PER_BAR + 1)
}

pub fn monitor(frame: u64, tempo_bpm: f32) -> String {
    let (bar, beat) = position(frame, tempo_bpm);
    format!("{bar:04}.{beat:02}")
}

pub fn shift(frame: u64, tempo_bpm: f32, beats: i64) -> u64 {
    if beats == 0 {
        return frame;
    }
    let frames_per_beat = f64::from(daw_core::SAMPLE_RATE) * 60.0 / f64::from(tempo_bpm);
    // The next sample must not fall just before a fractional beat boundary.
    (frame as f64 + beats as f64 * frames_per_beat)
        .max(0.0)
        .ceil() as u64
}

pub fn size_boundaries(frame: u64, anchor: u64, length: u64) -> impl Iterator<Item = u64> {
    let length = i128::from(length.max(1));
    let index = (i128::from(frame) - i128::from(anchor)).div_euclid(length);
    [index, index + 1]
        .into_iter()
        .filter_map(move |index| u64::try_from(i128::from(anchor) + index * length).ok())
}

pub struct Timeline {
    beats_per_second: f64,
    pixels_per_beat: f64,
    scroll_beats: f64,
}

pub struct Tick {
    pub x: f32,
    pub beat: f64,
    pub bar: bool,
    pub whole_beat: bool,
    pub label: Option<String>,
}

impl Timeline {
    pub fn new(tempo_bpm: f32, pixels_per_second: f32, scroll_seconds: f64) -> Self {
        let beats_per_second = f64::from(tempo_bpm) / 60.0;
        Self {
            beats_per_second,
            pixels_per_beat: f64::from(pixels_per_second) / beats_per_second,
            scroll_beats: scroll_seconds * beats_per_second,
        }
    }

    pub fn beats(&self, frame: u64) -> f64 {
        seconds(frame) * self.beats_per_second
    }

    fn visible_step(&self) -> f64 {
        2.0_f64
            .powf((10.0 / self.pixels_per_beat).log2().ceil())
            .max(1.0 / 16.0)
    }

    fn snap_radius(&self) -> f64 {
        6.0 / self.pixels_per_beat / self.beats_per_second * f64::from(daw_core::SAMPLE_RATE)
    }

    pub fn snap_anchor(
        &self,
        frame: u64,
        limits: std::ops::RangeInclusive<u64>,
        anchors: impl IntoIterator<Item = u64>,
    ) -> Option<u64> {
        anchors
            .into_iter()
            .filter(|candidate| {
                limits.contains(candidate) && candidate.abs_diff(frame) as f64 <= self.snap_radius()
            })
            .min_by_key(|candidate| candidate.abs_diff(frame))
    }

    /// Nearby visible musical lines take precedence over source/clip boundaries.
    pub fn snap(
        &self,
        frame: u64,
        limits: std::ops::RangeInclusive<u64>,
        anchors: impl IntoIterator<Item = u64>,
    ) -> u64 {
        let radius = self.snap_radius();
        let near = |candidate: u64| {
            limits.contains(&candidate) && candidate.abs_diff(frame) as f64 <= radius
        };
        let step = self.visible_step();
        for spacing in [step.max(BEATS_PER_BAR as f64), step.max(1.0), step] {
            let index = (self.beats(frame) / spacing).floor();
            let grid = [index, index + 1.0]
                .into_iter()
                .filter(|index| *index >= 0.0)
                // Ceil prevents fractional beat boundaries landing in the previous beat.
                .map(|index| {
                    (index * spacing / self.beats_per_second * f64::from(daw_core::SAMPLE_RATE))
                        .ceil() as u64
                })
                .filter(|candidate| near(*candidate))
                .min_by_key(|candidate| candidate.abs_diff(frame));
            if let Some(candidate) = grid {
                return candidate;
            }
        }
        self.snap_anchor(frame, limits, anchors).unwrap_or(frame)
    }

    pub fn ticks(&self, width: f32) -> impl Iterator<Item = Tick> + '_ {
        // Power-of-two subdivisions keep every tick aligned to the 4/4 bars.
        // Bound tick density in pixels, including at extreme valid tempos.
        let step = self.visible_step();
        let label_step = 2.0_f64
            .powf((80.0 / self.pixels_per_beat).log2().ceil())
            .max(1.0);
        let first = (self.scroll_beats / step).floor();
        let count = (f64::from(width) / (step * self.pixels_per_beat)).ceil() as usize + 2;
        (0..count).filter_map(move |index| {
            let beat = (first + index as f64) * step;
            let x = (beat - self.scroll_beats) * self.pixels_per_beat;
            if x < 0.0 || x > f64::from(width) {
                return None;
            }
            let whole_beat = beat.fract() == 0.0;
            let bar = whole_beat && beat % BEATS_PER_BAR as f64 == 0.0;
            let label = if beat % label_step == 0.0 {
                let whole = beat as u64;
                let bar_number = whole / BEATS_PER_BAR + 1;
                Some(if label_step >= BEATS_PER_BAR as f64 {
                    bar_number.to_string()
                } else {
                    format!("{bar_number}.{}", whole % BEATS_PER_BAR + 1)
                })
            } else {
                None
            };
            Some(Tick {
                x: x as f32,
                beat,
                bar,
                whole_beat,
                label,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::frames;

    #[test]
    fn monitor_counts_beats_and_bars_from_one_at_the_project_tempo() {
        assert_eq!(monitor(0, 120.0), "0001.01");
        assert_eq!(monitor(23_999, 120.0), "0001.01");
        assert_eq!(monitor(24_000, 120.0), "0001.02");
        assert_eq!(monitor(95_999, 120.0), "0001.04");
        assert_eq!(monitor(96_000, 120.0), "0002.01");
        assert_eq!(monitor(frames(2.0), 60.0), "0001.03");
        assert_eq!(monitor(frames(8.0), 90.0), "0004.01");
        let boundary = (48_000.0 * 60.0 / 123.5_f64).ceil() as u64;
        assert_eq!(monitor(boundary - 1, 123.5), "0001.01");
        assert_eq!(monitor(boundary, 123.5), "0001.02");
    }

    #[test]
    fn shifting_whole_beats_preserves_phase_and_crosses_fractional_boundaries() {
        for tempo in [60.0, 123.5, 127.0] {
            assert_eq!(monitor(shift(0, tempo, 1), tempo), "0001.02");
            assert_eq!(monitor(shift(0, tempo, 4), tempo), "0002.01");
            let fourth_beat = shift(0, tempo, 3);
            assert_eq!(monitor(shift(fourth_beat, tempo, 1), tempo), "0002.01");
            assert_eq!(
                monitor(shift(shift(0, tempo, 4), tempo, -1), tempo),
                "0001.04"
            );
            let phase = fourth_beat + frames(0.1);
            let advanced = shift(phase, tempo, 4);
            let returned = shift(advanced, tempo, -4);
            assert!(returned.abs_diff(phase) <= 1);
            assert_eq!(shift(phase, tempo, 0), phase);
            assert_eq!(shift(phase, tempo, -100), 0);
        }
    }
    #[test]
    fn ruler_and_grid_ticks_follow_tempo_scroll_and_zoom() {
        let timeline = Timeline::new(120.0, 70.0, 0.0);
        let ticks = timeline.ticks(500.0).collect::<Vec<_>>();
        let bar_two = ticks
            .iter()
            .find(|tick| tick.label.as_deref() == Some("2"))
            .unwrap();
        assert_eq!(bar_two.x, 140.0);
        assert!(bar_two.bar);
        assert_eq!(timeline.beats(frames(2.0)), bar_two.beat);
        let slower = Timeline::new(60.0, 70.0, 0.0);
        assert_eq!(
            slower.ticks(500.0).find(|tick| tick.beat == 4.0).unwrap().x,
            280.0
        );
        let scrolled = Timeline::new(120.0, 70.0, 1.5);
        assert_eq!(
            scrolled
                .ticks(500.0)
                .find(|tick| tick.beat == 4.0)
                .unwrap()
                .x,
            35.0
        );
        let close = Timeline::new(120.0, 300.0, 0.0);
        assert_eq!(
            close
                .ticks(500.0)
                .find(|tick| tick.beat == 1.0)
                .unwrap()
                .label
                .as_deref(),
            Some("1.2")
        );
        for tempo in [f32::MIN_POSITIVE, 60.0, 120.0, 999.0, f32::MAX] {
            for zoom in [1.0, 70.0, 3000.0] {
                let timeline = Timeline::new(tempo, zoom, 0.0);
                let ticks = timeline.ticks(1000.0).collect::<Vec<_>>();
                assert!(ticks.len() <= 102);
                assert!(ticks.iter().all(|tick| tick.x.is_finite()));
            }
        }
    }

    #[test]
    fn snapping_prioritizes_bars_beats_and_visible_subdivisions_before_anchors() {
        let timeline = Timeline::new(120.0, 70.0, 0.0);
        for (raw, expected) in [(2.03, 2.0), (2.53, 2.5), (2.28, 2.25)] {
            assert_eq!(
                timeline.snap(frames(raw), 0..=u64::MAX, [frames(raw)]),
                frames(expected)
            );
        }
        assert_eq!(
            timeline.snap(frames(2.13), 0..=u64::MAX, [frames(2.14)]),
            frames(2.14)
        );
        assert_eq!(timeline.snap(frames(2.13), 0..=u64::MAX, []), frames(2.13));
        let close = Timeline::new(120.0, 300.0, 1.5);
        assert_eq!(close.snap(frames(2.07), 0..=u64::MAX, []), frames(2.0625));
        let far = Timeline::new(120.0, 10.0, 0.0);
        assert_eq!(far.snap(frames(2.25), 0..=u64::MAX, []), frames(2.0));
        // The bar wins even when a visible subdivision is closer to the pointer.
        let dense = Timeline::new(120.0, 80.0, 0.0);
        assert_eq!(dense.snap(frames(2.075), 0..=u64::MAX, []), frames(2.0));
    }

    #[test]
    fn snap_candidates_respect_bounds_fractional_tempo_and_extreme_positions() {
        let timeline = Timeline::new(120.0, 70.0, 0.0);
        assert_eq!(
            timeline.snap(frames(2.03), frames(2.01)..=frames(2.04), [frames(2.02)]),
            frames(2.02)
        );
        let fractional = Timeline::new(123.5, 70.0, 0.0);
        let boundary = shift(0, 123.5, 4);
        assert_eq!(fractional.snap(boundary + 100, 0..=u64::MAX, []), boundary);
        assert_eq!(position(boundary, 123.5), (2, 1));
        for tempo in [f32::MIN_POSITIVE, 60.0, 120.0, 999.0, f32::MAX] {
            let timeline = Timeline::new(tempo, 70.0, 0.0);
            for frame in [0, 1, frames(3.0), u64::MAX - 1, u64::MAX] {
                assert_eq!(timeline.snap(frame, frame..=frame, []), frame);
            }
        }
    }

    #[test]
    fn size_boundaries_use_the_trimmed_base_and_remain_in_the_frame_range() {
        assert_eq!(size_boundaries(26, 3, 10).collect::<Vec<_>>(), vec![23, 33]);
        assert_eq!(size_boundaries(0, 23, 10).collect::<Vec<_>>(), vec![3]);
        assert_eq!(
            size_boundaries(u64::MAX, u64::MAX - 20, 15).collect::<Vec<_>>(),
            vec![u64::MAX - 5]
        );
        let timeline = Timeline::new(120.0, 70.0, 0.0);
        assert_eq!(
            timeline.snap_anchor(
                frames(4.251),
                frames(3.13)..=u64::MAX,
                size_boundaries(frames(4.251), frames(2.0), frames(1.13)),
            ),
            Some(frames(4.26))
        );
    }
}
