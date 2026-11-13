//! Fixed, peak-preserving log cells. Preparation validates every scan bound.
use daw_unit_core::{SPECTRUM_BINS, spectrum_frequency};
#[derive(Clone, Copy, Default)]
pub(super) struct Cell {
    pub first: u16,
    pub count: u8,
    pub interpolate: bool,
    pub fraction: f32,
}
pub(super) fn prepare(
    n: usize,
    rate: f32,
    maximum_hz: f32,
    per_cell: usize,
    total: usize,
) -> ([Cell; SPECTRUM_BINS], usize) {
    // Adjacent cells use exactly the same geometric midpoint. Compute these
    // from the published centers so rounding cannot leave a coefficient gap.
    let edge =
        |i: usize| (f64::from(spectrum_frequency(i)) * f64::from(spectrum_frequency(i + 1))).sqrt();
    let mut cells = [Cell::default(); SPECTRUM_BINS];
    let mut contributions = 0;
    let mut length = 0;
    for (i, cell) in cells.iter_mut().enumerate() {
        let hz = spectrum_frequency(i) as f64;
        if hz > f64::from(maximum_hz) {
            break;
        }
        let center = hz * n as f64 / f64::from(rate);
        let lower = if i == 0 {
            hz * hz / edge(0)
        } else {
            edge(i - 1)
        };
        let upper = if i == SPECTRUM_BINS - 1 {
            hz * hz / edge(i - 1)
        } else {
            edge(i)
        };
        let first = (lower * n as f64 / f64::from(rate)).ceil().max(1.0) as usize;
        let end = (upper * n as f64 / f64::from(rate))
            .ceil()
            .min((n / 2) as f64) as usize;
        assert!(
            end.saturating_sub(first) <= per_cell,
            "prepared spectrum cell bound"
        );
        *cell = if end > first {
            Cell {
                first: first as u16,
                count: (end - first) as u8,
                ..Cell::default()
            }
        } else {
            let first = (center.floor() as usize).clamp(1, n / 2 - 2);
            Cell {
                first: first as u16,
                count: 2,
                interpolate: true,
                fraction: (center - first as f64).clamp(0.0, 1.0) as f32,
            }
        };
        assert!(
            usize::from(cell.count) <= per_cell,
            "prepared spectrum cell bound"
        );
        contributions += usize::from(cell.count);
        length += 1;
    }
    assert!(contributions <= total, "prepared spectrum map budget");
    (cells, length)
}
