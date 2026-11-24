use std::f64::consts::TAU;
use timestretch::{Ratio, Request};

const RATE: u32 = 48_000;
const FRAMES: usize = RATE as usize * 10;
const GLIDE: [(f64, f64); 7] = [
    (0.0, 220.0),
    (1.0, 220.0),
    (3.4, 310.0),
    (5.1, 190.0),
    (6.6, 190.0),
    (8.5, 360.0),
    (10.0, 360.0),
];

fn pitch_at(seconds: f64) -> f64 {
    for segment in GLIDE.windows(2) {
        let [(start, from), (end, to)] = [segment[0], segment[1]];
        if seconds <= end {
            let t = (seconds - start) / (end - start);
            return from + (to - from) * t * t * (3.0 - 2.0 * t);
        }
    }
    GLIDE[GLIDE.len() - 1].1
}

#[test]
fn gradual_start_and_confident_short_estimate_preserve_the_glide_contour() {
    let mut phase = 0.0;
    let mut input = Vec::with_capacity(FRAMES);
    for frame in 0..FRAMES {
        let seconds = frame as f64 / f64::from(RATE);
        if frame > 0 {
            phase += TAU * pitch_at((frame as f64 - 0.5) / f64::from(RATE)) / f64::from(RATE);
        }
        let fade = (seconds / 0.08).min((10.0 - seconds) / 0.08).min(1.0);
        let harmonic = 0.78 * phase.sin() + 0.16 * (phase * 2.0).sin() + 0.06 * (phase * 3.0).sin();
        input.push((0.42 * fade * harmonic) as f32);
    }
    let ratio = Ratio::new(8, 1).unwrap();
    let output = Request::new(RATE, 1, ratio).process(&input).unwrap();
    assert_eq!(output.len(), FRAMES * 8);

    // Use crossings in 40 ms output windows. The known source contour provides
    // the target at the time of the measured crossings, independently of DSP.
    let mut errors = Vec::new();
    for left in (0..output.len() - 1920).step_by(960) {
        let center = (left + 960) as f64 / (f64::from(RATE) * 8.0);
        if !GLIDE
            .windows(2)
            .any(|p| p[0].0 < center && center < p[1].0 && p[0].1 != p[1].1)
        {
            continue;
        }
        let mut crossings = output[left..left + 1920]
            .windows(2)
            .enumerate()
            .filter(|(_, p)| p[0] <= 0.0 && p[1] > 0.0)
            .map(|(i, p)| (left + i) as f64 - f64::from(p[0]) / f64::from(p[1] - p[0]));
        let first = crossings.next().unwrap();
        let (n, last) = crossings.enumerate().last().unwrap();
        let measured = f64::from(RATE) * (n + 1) as f64 / (last - first);
        let expected = pitch_at((first + last) / (2.0 * f64::from(RATE) * 8.0));
        errors.push((1200.0 * (measured / expected).log2()).abs());
    }
    assert!(errors.len() > 2_000);
    errors.sort_by(f64::total_cmp);
    let p95 = errors[((errors.len() - 1) as f64 * 0.95).round() as usize];
    assert!(p95 < 4.0, "glide P95 error {p95:.2} cents");
}
