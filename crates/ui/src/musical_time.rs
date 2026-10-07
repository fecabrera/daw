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

    pub fn ticks(&self, width: f32) -> impl Iterator<Item = Tick> + '_ {
        // Power-of-two subdivisions keep every tick aligned to the 4/4 bars.
        // Bound tick density in pixels, including at extreme valid tempos.
        let step = 2.0_f64
            .powf((10.0 / self.pixels_per_beat).log2().ceil())
            .max(1.0 / 16.0);
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
}
