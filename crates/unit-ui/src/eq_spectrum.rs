//! Display interpolation for the shared EQ spectrum; never processes audio.
use daw_unit_core::{SPECTRUM_BINS, SpectrumReading};
use egui::{Id, Pos2, Rect, Ui};

const FLOOR_DB: f32 = -90.0;
const FOLLOW_SECONDS: f64 = 0.035;
#[derive(Clone)]
struct Display {
    db: [f32; SPECTRUM_BINS],
    rate: u32,
    time: f64,
    frame: u64,
    generation: u64,
}

impl Display {
    #[cfg(test)]
    fn follow(&mut self, target: [f32; SPECTRUM_BINS], rate: u32, time: f64, frame: u64) {
        self.follow_frame(target, rate, self.generation, time, frame);
    }
    fn follow_frame(
        &mut self,
        target: [f32; SPECTRUM_BINS],
        rate: u32,
        generation: u64,
        time: f64,
        frame: u64,
    ) {
        if self.rate != rate || self.generation != generation || frame > self.frame + 1 {
            self.db = target;
        } else {
            let amount = (1.0 - (-(time - self.time).max(0.0) / FOLLOW_SECONDS).exp()) as f32;
            for (current, target) in self.db.iter_mut().zip(target) {
                *current += (target - *current) * amount;
            }
        }
        self.rate = rate;
        self.time = time;
        self.frame = frame;
        self.generation = generation;
    }
}

pub(super) fn clear(ui: &Ui, id: Id) {
    ui.data_mut(|data| data.remove::<Display>(id));
}

pub(super) fn points(
    ui: &Ui,
    id: Id,
    plot: Rect,
    reading: Option<&SpectrumReading>,
    enabled: bool,
) -> Vec<Pos2> {
    let Some(reading) = reading
        .filter(|r| enabled && r.valid_branches != 0 && r.magnitudes.iter().any(|v| *v > 0.0))
    else {
        clear(ui, id);
        return Vec::new();
    };
    let target = reading.magnitudes.map(|value| {
        if value.is_finite() && value > 0.0 {
            (20.0 * value.max(1e-8).log10()).clamp(FLOOR_DB, 0.0)
        } else {
            FLOOR_DB
        }
    });
    let time = ui.input(|input| input.time);
    let frame = ui.ctx().cumulative_frame_nr();
    let mut display = ui
        .data_mut(|data| data.get_temp::<Display>(id))
        .unwrap_or(Display {
            db: target,
            rate: reading.sample_rate_hz,
            time,
            frame,
            generation: reading.generation,
        });
    display.follow_frame(
        target,
        reading.sample_rate_hz,
        reading.generation,
        time,
        frame,
    );
    let points = curve(plot, &display.db);
    if display.db.iter().any(|db| *db > FLOOR_DB + 0.05) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
    ui.data_mut(|data| data.insert_temp(id, display));
    points
}

fn curve(plot: Rect, db: &[f32; SPECTRUM_BINS]) -> Vec<Pos2> {
    // Uniform log-frequency bins permit shape-preserving cubic interpolation.
    // Zero slopes at extrema and bounded segments avoid invented spectral peaks.
    let differences: [f32; SPECTRUM_BINS - 1] = std::array::from_fn(|i| db[i + 1] - db[i]);
    let slopes: [f32; SPECTRUM_BINS] = std::array::from_fn(|i| match i {
        0 => differences[0],
        i if i == SPECTRUM_BINS - 1 => differences[i - 1],
        i => {
            let (a, b) = (differences[i - 1], differences[i]);
            if a * b <= 0.0 {
                0.0
            } else {
                2.0 * a * b / (a + b)
            }
        }
    });
    // Include every measured center exactly, plus cubic midpoints. Reduce only
    // after this step so a narrow peak cannot fall between drawing samples.
    let segments = 2 * (SPECTRUM_BINS - 1);
    let dense: Vec<_> = (0..=segments)
        .map(|index| {
            let fraction = index as f32 / segments as f32;
            let bin = fraction * (SPECTRUM_BINS - 1) as f32;
            let i = (bin.floor() as usize).min(SPECTRUM_BINS - 2);
            let t = bin - i as f32;
            let t2 = t * t;
            let value = ((2.0 * t - 3.0) * t2 + 1.0) * db[i]
                + ((t - 2.0) * t2 + t) * slopes[i]
                + (-2.0 * t + 3.0) * t2 * db[i + 1]
                + (t - 1.0) * t2 * slopes[i + 1];
            let value = value.clamp(db[i].min(db[i + 1]), db[i].max(db[i + 1]));
            egui::pos2(
                plot.left() + fraction * plot.width(),
                plot.bottom() - plot.height() * (value - FLOOR_DB) / -FLOOR_DB,
            )
        })
        .collect();
    reduce(plot, &dense)
}

fn reduce(plot: Rect, dense: &[Pos2]) -> Vec<Pos2> {
    // Retain the min and max of each logical column in their original order.
    // Two extrema per column preserve both peaks and valleys at pixel limits.
    let columns = plot.width().ceil().clamp(64.0, 511.0) as usize;
    let mut result = Vec::with_capacity(2 * columns + 2);
    let mut push = |index: usize| {
        if result.last() != Some(&dense[index]) {
            result.push(dense[index]);
        }
    };
    push(0);
    let mut start = 0;
    while start < dense.len() {
        let column = ((dense[start].x - plot.left()) / plot.width() * columns as f32) as usize;
        let mut end = start + 1;
        let mut minimum = start;
        let mut maximum = start;
        while end < dense.len()
            && ((dense[end].x - plot.left()) / plot.width() * columns as f32) as usize == column
        {
            if dense[end].y < dense[minimum].y {
                minimum = end;
            }
            if dense[end].y > dense[maximum].y {
                maximum = end;
            }
            end += 1;
        }
        push(minimum.min(maximum));
        push(minimum.max(maximum));
        start = end;
    }
    push(dense.len() - 1);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_is_time_based_and_resets_after_rate_or_visibility_changes() {
        let initial = Display {
            db: [-60.0; SPECTRUM_BINS],
            rate: 48_000,
            time: 0.0,
            frame: 0,
            generation: 1,
        };
        let mut one = initial.clone();
        one.follow([-12.0; SPECTRUM_BINS], 48_000, 0.03, 1);
        assert!(one.db[0] > -60.0 && one.db[0] < -12.0);
        let mut three = initial;
        for frame in 1..=3 {
            three.follow([-12.0; SPECTRUM_BINS], 48_000, frame as f64 * 0.01, frame);
        }
        assert!((one.db[0] - three.db[0]).abs() < 0.0001);
        let previous = one.db;
        one.follow([0.0; SPECTRUM_BINS], 48_000, 0.03, 1);
        assert_eq!(
            one.db, previous,
            "a second layout pass must not advance animation"
        );
        one.follow([-30.0; SPECTRUM_BINS], 96_000, 0.04, 2);
        assert_eq!(one.db, [-30.0; SPECTRUM_BINS]);
        one.follow([-70.0; SPECTRUM_BINS], 96_000, 0.05, 5);
        assert_eq!(one.db, [-70.0; SPECTRUM_BINS]);
    }

    #[test]
    fn dense_curve_stays_inside_each_measured_segment_without_overshoot() {
        let plot = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(240.0, 150.0));
        let db = std::array::from_fn(|i| if i % 3 == 0 { -5.0 } else { -80.0 });
        let points = curve(plot, &db);
        assert!((65..=1025).contains(&points.len()));
        assert_eq!(points.first().unwrap().x, plot.left());
        assert_eq!(points.last().unwrap().x, plot.right());
        for pair in points.windows(2) {
            assert!(pair[0].x < pair[1].x);
        }
        for point in &points {
            assert!(plot.contains(*point));
            let bin = ((point.x - plot.left()) / plot.width() * (SPECTRUM_BINS - 1) as f32).floor()
                as usize;
            let bin = bin.min(SPECTRUM_BINS - 2);
            let value = FLOOR_DB + (plot.bottom() - point.y) / plot.height() * -FLOOR_DB;
            assert!(value >= db[bin].min(db[bin + 1]) - 0.0001);
            assert!(value <= db[bin].max(db[bin + 1]) + 0.0001);
        }
    }

    #[test]
    fn missing_silent_and_disabled_readings_clear_cached_motion_immediately() {
        let ctx = egui::Context::default();
        let id = Id::new("spectrum-test");
        let reading = SpectrumReading {
            magnitudes: [0.1; SPECTRUM_BINS],
            valid_branches: 3,
            ..Default::default()
        };
        let empty = SpectrumReading::default();
        for (reading, enabled) in [
            (Some(&reading), true),
            (None, true),
            (Some(&reading), true),
            (Some(&empty), true),
            (Some(&reading), true),
            (Some(&reading), false),
        ] {
            let active = enabled && reading.is_some_and(|r| r.magnitudes.iter().any(|v| *v > 0.0));
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let points = points(
                    ui,
                    id,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0)),
                    reading,
                    enabled,
                );
                assert_eq!(!points.is_empty(), active);
            });
            output.textures_delta.clear();
            assert_eq!(
                ctx.data(|data| data.get_temp::<Display>(id).is_some()),
                active
            );
        }
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn pixel_limited_reduction_keeps_measured_peaks_and_valleys() {
        for width in [90.0, 240.0, 800.0] {
            let plot = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 100.0));
            let mut db = [-90.0; SPECTRUM_BINS];
            db[100] = -6.0;
            db[125] = -12.0;
            let points = curve(plot, &db);
            for index in [100, 125] {
                let x = index as f32 / 511.0 * width;
                let y = 100.0 - 100.0 * (db[index] - FLOOR_DB) / -FLOOR_DB;
                assert!(
                    points
                        .iter()
                        .any(|p| (p.x - x).abs() < 0.001 && (p.y - y).abs() < 0.001)
                );
            }
            assert!(points.iter().any(|p| p.x > 100.0 / 511.0 * width
                && p.x < 125.0 / 511.0 * width
                && p.y == plot.bottom()));
        }
    }
}
