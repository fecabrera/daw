//! Live monitor click, driven by the renderer's audible project frame. No export
//! renderer, project data, clocks, buffers or owners are created on the callback.
use std::f64::consts::TAU;

pub(super) struct Metronome {
    rate: u32,
    enabled: bool,
    gain: f32,
    previous: f32,
    previous_frame: Option<u64>,
    tempo: f64,
    transition: u32,
    transition_from: f32,
}
impl Metronome {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            enabled: false,
            gain: 0.0,
            previous: 0.0,
            previous_frame: None,
            tempo: 0.0,
            transition: 0,
            transition_from: 0.0,
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn next(&mut self, frame: Option<u64>, tempo: f64) -> f32 {
        let fade = (self.rate / 200).max(1);
        let target = if self.enabled && frame.is_some() {
            1.0
        } else {
            0.0
        };
        self.gain += (target - self.gain).clamp(-1.0 / fade as f32, 1.0 / fade as f32);
        if self.gain == 0.0 {
            self.previous = 0.0;
            self.previous_frame = frame;
            self.tempo = tempo;
            self.transition = 0;
            return 0.0;
        }
        let mut tone = if let Some(frame) = frame {
            if self.previous_frame.and_then(|n| n.checked_add(1)) != Some(frame)
                || self.tempo != tempo
            {
                self.transition = fade;
                self.transition_from = self.previous;
            }
            click(frame, tempo, self.rate)
        } else {
            self.previous
        };
        if self.transition > 0 {
            let progress = 1.0 - self.transition as f32 / fade as f32;
            tone = self.transition_from * (1.0 - progress) + tone * progress;
            self.transition -= 1;
        }
        self.previous = tone;
        self.previous_frame = frame;
        self.tempo = tempo;
        tone * self.gain
    }
}

fn click(frame: u64, tempo: f64, rate: u32) -> f32 {
    if rate == 0 || !tempo.is_finite() || tempo <= 0.0 {
        return 0.0;
    }
    // Derive every beat from the absolute musical clock; rounding each boundary
    // independently avoids drift at fractional frame periods. Limit to one beat
    // per sample for otherwise valid but impractically high project tempos.
    let period = (f64::from(rate) * 60.0 / tempo).clamp(1.0, f64::MAX);
    let beat = ((frame as f64 + 0.5) / period).floor();
    let start = (beat * period).round();
    let age = (frame as f64 - start).max(0.0);
    let duration = f64::from(rate) * 0.020;
    if age >= duration {
        return 0.0;
    }
    let accented = beat.rem_euclid(4.0) == 0.0;
    let frequency = if accented { 2400.0 } else { 1600.0 };
    // A 10 dB amplitude increase is 10^(10/20).
    let amplitude = if accented { 0.40 } else { 0.28 } * 3.162_277_660_168_379_5;
    let attack = (age / (f64::from(rate) * 0.005)).min(1.0);
    let decay = (1.0 - age / duration).powi(3);
    (amplitude * attack * decay * (TAU * frequency * age / f64::from(rate)).sin()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metronome_clicks_follow_rounded_beats_without_accumulated_drift_at_all_rates() {
        for rate in daw_core::SAMPLE_RATES {
            for tempo in [89.3, 120.0, 163.7] {
                for beat in [1_u64, 4, 57, 1_000, 100_000] {
                    let frame = (beat as f64 * 60.0 * f64::from(rate) / tempo).round() as u64;
                    assert_eq!(
                        click(frame - 1, tempo, rate),
                        0.0,
                        "early beat {beat}, {rate}, {tempo}"
                    );
                    assert!(
                        click(frame + 1, tempo, rate) > 0.0,
                        "missing beat {beat}, {rate}, {tempo}"
                    );
                    assert_eq!(click(frame + u64::from(rate) / 10, tempo, rate), 0.0);
                }
            }
        }
    }

    #[test]
    fn metronome_accents_bar_starts_and_is_silent_outside_short_clicks() {
        let peak = |beat: u64| {
            (0..960_u64)
                .map(|age| click(beat * 24_000 + age, 120.0, 48_000).abs())
                .fold(0.0_f32, f32::max)
        };
        assert!((0.52..0.55).contains(&peak(0)), "bar click level");
        assert!((0.36..0.39).contains(&peak(1)), "beat click level");
        assert!(peak(0) > peak(1) * 1.25);
        assert_eq!(peak(0), peak(4));
        assert_eq!(peak(1), peak(5));
        for frame in 960..24_000 {
            assert_eq!(click(frame, 120.0, 48_000), 0.0);
        }
    }

    #[test]
    fn metronome_power_pause_and_tempo_changes_remain_bounded_and_fade_to_silence() {
        for rate in daw_core::SAMPLE_RATES {
            let mut click = Metronome::new(rate);
            assert_eq!(click.next(Some(0), 120.0), 0.0);
            click.set_enabled(true);
            for frame in 0..u64::from(rate) / 100 {
                let value = click.next(Some(frame), 120.0);
                assert!(value.is_finite() && value.abs() <= 0.6);
            }
            // Pause part-way through a click, then restart/seek with another tempo.
            for _ in 0..rate / 200 + 2 {
                click.next(None, 120.0);
            }
            assert_eq!(click.next(None, 120.0), 0.0);
            for frame in 0..u64::from(rate) / 100 {
                click.next(Some(frame), 89.3);
            }
            click.set_enabled(false);
            for frame in 0..u64::from(rate) / 200 + 2 {
                click.next(Some(frame), 89.3);
            }
            assert_eq!(click.next(Some(1), 89.3), 0.0);
        }
    }
}
