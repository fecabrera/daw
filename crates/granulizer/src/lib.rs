//! Bounded stereo granular synthesis over host-owned immutable PCM. No loading or note scheduling.
use daw_unit_core::{GrainWindow, GranulizerSettings, ModulationSmoother, ProcessingContext};

/// A short-grain edit cannot cut existing long grains. If full, skip a new grain instead.
pub const MAX_GRAINS: usize = 64;
#[derive(Clone, Copy, Default)]
struct Grain {
    age: u32,
    frames: u32,
    position: f64,
    step: f64,
    pan: [f32; 2],
    window: GrainWindow,
    looping: bool,
}
#[derive(Clone)]
pub struct Granulizer {
    settings: GranulizerSettings,
    base: [f64; 8],
    base_remaining: u32,
    external: ModulationSmoother<8>,
    rate: u32,
    source_length: usize,
    root: u8,
    note: u8,
    live_ratio: f64,
    live_target: f64,
    live_remaining: u32,
    grains: [Grain; MAX_GRAINS],
    scan: f64,
    last_position: f64,
    until_grain: f64,
    random: u64,
    elapsed: u64,
    one_shot: bool,
    one_shot_target: bool,
    scanning: bool,
    active: bool,
    effective: [f32; 8],
}
impl Granulizer {
    pub fn new(
        context: ProcessingContext,
        settings: GranulizerSettings,
        root: u8,
        one_shot: bool,
    ) -> Self {
        Self {
            settings,
            base: settings.values().map(f64::from),
            base_remaining: 0,
            external: ModulationSmoother::new(context),
            rate: context.sample_rate_hz,
            source_length: 0,
            root,
            note: root,
            live_ratio: 1.0,
            live_target: 1.0,
            live_remaining: 0,
            grains: [Grain::default(); MAX_GRAINS],
            scan: 0.0,
            last_position: f64::from(settings.position),
            until_grain: 0.0,
            random: 1,
            elapsed: 0,
            one_shot,
            one_shot_target: one_shot,
            scanning: false,
            active: false,
            effective: settings.values(),
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        if self.settings.values() != next.settings.values() {
            self.base_remaining = (self.rate / 200).max(1);
        }
        self.settings = next.settings;
        self.one_shot_target = next.one_shot_target;
        // Root changes reconstruct voices in the host. Trigger edits apply at the next note-on.
    }
    pub fn reset(&mut self) {
        self.live_ratio = 1.0;
        self.live_target = 1.0;
        self.live_remaining = 0;
        self.grains.fill(Grain::default());
        self.active = false;
        self.scanning = false;
        self.base = self.settings.values().map(f64::from);
        self.base_remaining = 0;
        self.external.reset();
    }
    /// Preserve grain ages, positions, scan and randomness while retuning the played note.
    pub fn set_live_pitch(&mut self, note: u8) {
        let note = note.min(127);
        let target = 2_f64.powf(f64::from(i16::from(note) - i16::from(self.note)) / 12.0);
        if target != self.live_target {
            self.live_target = target;
            self.live_remaining = (self.rate / 200).max(1);
        }
    }
    pub fn note_on(&mut self, note: u8, length: usize) {
        self.reset();
        self.note = note;
        self.source_length = length;
        self.one_shot = self.one_shot_target;
        self.elapsed = 0;
        self.random = u64::from(self.settings.seed) ^ (u64::from(note) << 32) ^ 0x9e3779b97f4a7c15;
        self.scan = f64::from(self.settings.position) * length.saturating_sub(1) as f64;
        self.last_position = f64::from(self.settings.position);
        self.until_grain = 0.0;
        self.scanning = length > 0;
        self.active = self.scanning;
    }
    /// Reconstruct an unmodulated note in bounded work, including its random sequence.
    /// The host replays modulated processing on workers before publishing transport state.
    pub fn advance(&mut self, frames: u64, length: usize) {
        if !self.active || length == 0 {
            return;
        }
        let elapsed = self.elapsed.saturating_add(frames);
        let note = self.note;
        let one_shot = self.one_shot;
        self.note_on(note, length);
        self.one_shot = one_shot;
        let length = length as f64;
        let origin = f64::from(self.settings.position) * (length - 1.0);
        let speed = f64::from(self.settings.speed);
        let size = (f64::from(self.settings.size_ms) * f64::from(self.rate) / 1000.0)
            .round()
            .max(2.0) as u32;
        let interval = f64::from(size) / f64::from(self.settings.overlap);
        let scan_end = if !self.settings.looping && speed != 0.0 {
            if speed > 0.0 {
                ((length - origin) / speed).ceil() as u64
            } else {
                (origin / -speed).floor() as u64 + 1
            }
        } else {
            u64::MAX
        };
        let end = scan_end.min(if one_shot { length as u64 } else { u64::MAX });
        let scanned = elapsed.min(end);
        let count = (scanned as f64 / interval).ceil() as u64;
        let first = count.saturating_sub(MAX_GRAINS as u64);
        self.random = self
            .random
            .wrapping_add(first.wrapping_mul(3).wrapping_mul(0x9e3779b97f4a7c15));
        for serial in first..count {
            let start = (serial as f64 * interval).ceil() as u64;
            let age = elapsed.saturating_sub(start);
            let position = origin
                + speed * start as f64
                + self.random_signed() * f64::from(self.settings.position_jitter) * (length - 1.0);
            let pitch = f64::from(i16::from(note) - i16::from(self.root))
                + f64::from(self.settings.pitch)
                + self.random_signed() * f64::from(self.settings.pitch_jitter);
            let pan = (self.random_signed() * f64::from(self.settings.pan)) as f32;
            if age >= u64::from(size) {
                continue;
            }
            let step = 2.0_f64.powf(pitch.clamp(-223.0, 223.0) / 12.0);
            let position = if self.settings.looping {
                position.rem_euclid(length)
            } else {
                position.clamp(0.0, length - 1.0)
            };
            let position = position + step * age as f64;
            if let Some(slot) = self.grains.iter_mut().find(|g| g.age >= g.frames) {
                *slot = Grain {
                    age: age as u32,
                    frames: size,
                    position: if self.settings.looping {
                        position.rem_euclid(length)
                    } else {
                        position
                    },
                    step,
                    pan: [(1.0 - pan).sqrt(), (1.0 + pan).sqrt()],
                    window: self.settings.window,
                    looping: self.settings.looping,
                };
            }
        }
        self.elapsed = elapsed;
        self.until_grain = count as f64 * interval - elapsed as f64;
        let scan = origin + speed * scanned as f64;
        self.scan = if self.settings.looping {
            scan.rem_euclid(length)
        } else {
            scan.clamp(0.0, length - 1.0)
        };
        self.scanning = elapsed < end;
        self.active = self.scanning || self.grains.iter().any(|g| g.age < g.frames);
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn position(&self) -> Option<u64> {
        self.active.then_some(self.scan.max(0.0) as u64)
    }
    pub fn effective(&self) -> [f32; 8] {
        self.effective
    }
    /// At most sixteen actual grain positions are published; never synthesize a UI-side clock.
    pub fn grain_positions(&self) -> [f32; 16] {
        let mut out = [f32::NAN; 16];
        for (out, grain) in out
            .iter_mut()
            .zip(self.grains.iter().filter(|g| g.age < g.frames))
        {
            *out = if grain.looping {
                grain.position.rem_euclid(self.source_length as f64) as f32
            } else if (0.0..self.source_length as f64).contains(&grain.position) {
                grain.position as f32
            } else {
                f32::NAN
            };
        }
        out
    }
    fn random_signed(&mut self) -> f64 {
        self.random = self.random.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.random;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        (z >> 11) as f64 * (2.0 / ((1_u64 << 53) as f64)) - 1.0
    }
    pub fn process(&mut self, samples: &[[f32; 2]], offsets: [f32; 8]) -> [f32; 2] {
        if !self.active || samples.is_empty() {
            return [0.0; 2];
        }
        let offsets = self.external.process(offsets);
        let target = self.settings.values();
        if self.base_remaining > 0 {
            for (base, target) in self.base.iter_mut().zip(target) {
                *base += (f64::from(target) - *base) / f64::from(self.base_remaining);
            }
            self.base_remaining -= 1;
        }
        self.effective = std::array::from_fn(|i| {
            let (min, max) = GranulizerSettings::bounds()[i];
            (self.base[i] + offsets[i]).clamp(f64::from(min), f64::from(max)) as f32
        });
        let [
            size,
            overlap,
            position,
            speed,
            pitch,
            jitter,
            pitch_jitter,
            pan,
        ] = self.effective;
        let length = samples.len() as f64;
        let displacement = (f64::from(position) - self.last_position) * (length - 1.0);
        self.scan += displacement;
        self.last_position = f64::from(position);
        if self.settings.looping {
            self.scan = self.scan.rem_euclid(length);
        } else if displacement != 0.0 {
            // Preserve fractional scan positions near the end during ordinary advancement.
            self.scan = self.scan.clamp(0.0, length - 1.0);
        }
        if self.one_shot && self.elapsed >= samples.len() as u64 {
            self.scanning = false;
        }
        if self.scanning && self.until_grain <= 0.0 {
            let frames = (f64::from(size) * f64::from(self.rate) / 1000.0)
                .round()
                .max(2.0) as u32;
            let pos = self.scan + self.random_signed() * f64::from(jitter) * (length - 1.0);
            let semitones = f64::from(i16::from(self.note) - i16::from(self.root))
                + f64::from(pitch)
                + self.random_signed() * f64::from(pitch_jitter);
            let pan = (self.random_signed() * f64::from(pan)) as f32;
            let grain = Grain {
                age: 0,
                frames,
                position: if self.settings.looping {
                    pos.rem_euclid(length)
                } else {
                    pos.clamp(0.0, length - 1.0)
                },
                step: 2.0_f64.powf(semitones.clamp(-223.0, 223.0) / 12.0),
                pan: [(1.0 - pan).sqrt(), (1.0 + pan).sqrt()],
                window: self.settings.window,
                looping: self.settings.looping,
            };
            if let Some(slot) = self.grains.iter_mut().find(|g| g.age >= g.frames) {
                *slot = grain;
            }
            self.until_grain += f64::from(frames) / f64::from(overlap);
        }
        self.until_grain -= 1.0;
        if self.live_remaining > 0 {
            self.live_ratio +=
                (self.live_target - self.live_ratio) / f64::from(self.live_remaining);
            self.live_remaining -= 1;
        }
        let mut output = [0.0_f64; 2];
        let mut weights = 0.0_f64;
        let mut live = false;
        for grain in &mut self.grains {
            if grain.age >= grain.frames {
                continue;
            }
            live = true;
            let weight = grain
                .window
                .amplitude(grain.age as f32 / (grain.frames - 1) as f32);
            weights += f64::from(weight);
            if grain.looping {
                grain.position = grain.position.rem_euclid(length);
            }
            if grain.position >= 0.0 && grain.position < length {
                let i = grain.position as usize;
                let next = if grain.looping {
                    (i + 1) % samples.len()
                } else {
                    (i + 1).min(samples.len() - 1)
                };
                let fraction = grain.position.fract();
                // Non-looping reads can reach the source edge before the grain window ends.
                // Fade in output-frame units, independently of the grain's pitch increment.
                let edge = if grain.looping {
                    1.0
                } else {
                    (grain.position.min(length - 1.0 - grain.position).max(0.0)
                        / (grain.step * self.live_ratio)
                        / f64::from((self.rate / 200).max(1)))
                    .min(1.0)
                };
                for (ch, value) in output.iter_mut().enumerate() {
                    let a = f64::from(samples[i][ch]);
                    let b = f64::from(samples[next][ch]);
                    let sample = a + (b - a) * fraction;
                    if sample.is_finite() {
                        *value += sample * edge * f64::from(weight * grain.pan[ch]);
                    }
                }
            }
            grain.age += 1;
            grain.position += grain.step * self.live_ratio;
        }
        if self.scanning {
            self.scan += f64::from(speed);
            if self.settings.looping {
                self.scan = self.scan.rem_euclid(length);
            } else if !(0.0..length).contains(&self.scan) {
                self.scan = self.scan.clamp(0.0, length - 1.0);
                self.scanning = false;
            }
        }
        self.elapsed = self.elapsed.saturating_add(1);
        self.active = self.scanning || live;
        output
            .map(|v| (v / weights.max(1.0)).clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(rate: u32) -> ProcessingContext {
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        }
    }
    #[test]
    fn live_pitch_preserves_running_grains_scan_randomness_and_modulation() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let samples = vec![[0.25, -0.125]; rate as usize];
            let mut unit = Granulizer::new(context(rate), GranulizerSettings::default(), 60, false);
            unit.note_on(60, samples.len());
            for _ in 0..1000 {
                unit.process(&samples, [0.0; 8]);
            }
            let before = unit.clone();
            unit.set_live_pitch(72);
            assert_eq!(unit.elapsed, before.elapsed);
            assert_eq!(unit.scan, before.scan);
            assert_eq!(unit.random, before.random);
            for (a, b) in unit.grains.iter().zip(&before.grains) {
                assert_eq!(
                    (a.age, a.frames, a.position, a.step),
                    (b.age, b.frames, b.position, b.step)
                );
            }
            let mut reference = before;
            for _ in 0..rate / 200 {
                assert!(
                    unit.process(&samples, [0.0; 8])
                        .iter()
                        .all(|v| v.is_finite())
                );
                reference.process(&samples, [0.0; 8]);
            }
            assert_eq!(unit.elapsed, reference.elapsed);
            assert_eq!(unit.scan, reference.scan);
            assert_eq!(unit.random, reference.random);
            assert!((unit.live_ratio - 2.0).abs() < 1e-12);
        }
    }
    #[test]
    fn repeatable_grains_keep_stereo_gain_and_scan_independent_of_pitch_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let samples = vec![[0.25, -0.125]; rate as usize];
            for window in GrainWindow::ALL {
                let settings = GranulizerSettings {
                    window,
                    position: 0.5,
                    speed: -0.5,
                    ..Default::default()
                };
                let mut unit = Granulizer::new(context(rate), settings, 60, false);
                let mut high = Granulizer::new(context(rate), settings, 60, false);
                unit.note_on(60, samples.len());
                high.note_on(127, samples.len());
                for _ in 0..rate / 5 {
                    let a = unit.process(&samples, [0.0; 8]);
                    let b = high.process(&samples, [0.0; 8]);
                    assert_eq!(a, b);
                    assert_eq!(a[0], -2.0 * a[1]);
                    assert!((0.0..=0.250001).contains(&a[0]));
                }
                let expected = (f64::from(rate) - 1.0) * 0.5 - f64::from(rate / 5) * 0.5;
                assert!((unit.position().unwrap() as f64 - expected).abs() < 1.0);
            }
        }
    }
    #[test]
    fn freeze_negative_scan_windows_and_one_shot_have_bounded_lifetimes() {
        let samples = vec![[0.5; 2]; 4800];
        for looping in [false, true] {
            for speed in [-4.0, 0.0, 4.0] {
                let settings = GranulizerSettings {
                    looping,
                    speed,
                    position: 0.5,
                    size_ms: 10.0,
                    ..Default::default()
                };
                let mut one = Granulizer::new(context(48000), settings, 60, true);
                one.note_on(60, samples.len());
                for _ in 0..6000 {
                    assert!(
                        one.process(&samples, [0.0; 8])
                            .iter()
                            .all(|v| v.is_finite())
                    );
                }
                assert!(!one.active());
                let mut gate = Granulizer::new(context(48000), settings, 60, false);
                gate.note_on(60, samples.len());
                for _ in 0..6000 {
                    gate.process(&samples, [0.0; 8]);
                }
                assert_eq!(gate.active(), looping || speed == 0.0);
                if speed == 0.0 {
                    assert_eq!(gate.position(), Some(2399));
                }
            }
        }
    }
    #[test]
    fn randomness_is_repeatable_and_bounded_seek_keeps_grain_sequence() {
        let samples: Vec<_> = (0..48000)
            .map(|i| {
                let v = (i as f32 * 0.02).sin();
                [v, -v]
            })
            .collect();
        let settings = GranulizerSettings {
            speed: 0.5,
            position_jitter: 0.3,
            pitch_jitter: 12.0,
            pan: 1.0,
            size_ms: 100.0,
            overlap: 4.0,
            ..Default::default()
        };
        let mut unit = Granulizer::new(context(48000), settings, 60, false);
        unit.note_on(72, samples.len());
        let mut clone = unit.clone();
        for _ in 0..72000 {
            assert_eq!(
                unit.process(&samples, [0.0; 8]),
                clone.process(&samples, [0.0; 8])
            );
        }
        let mut seek = Granulizer::new(context(48000), settings, 60, false);
        seek.note_on(72, samples.len());
        seek.advance(72000, samples.len());
        for _ in 0..1000 {
            let a = unit.process(&samples, [0.0; 8]);
            let b = seek.process(&samples, [0.0; 8]);
            assert!(
                (a[0] - b[0]).abs() < 0.0001 && (a[1] - b[1]).abs() < 0.0001,
                "{a:?} {b:?}"
            );
        }
        seek.advance(u64::MAX / 4, samples.len());
        assert!(
            seek.process(&samples, [0.0; 8])
                .iter()
                .all(|v| v.is_finite())
        );
        assert_eq!(samples[0], [0.0, -0.0]);
    }
    #[test]
    fn pitch_changes_grain_read_rate_without_changing_scan_clock() {
        let samples: Vec<_> = (0..48000)
            .map(|i| [(i as f32 * std::f32::consts::TAU * 440.0 / 48000.0).sin(); 2])
            .collect();
        let settings = GranulizerSettings {
            speed: 0.0,
            size_ms: 500.0,
            overlap: 0.25,
            ..Default::default()
        };
        for (note, expected) in [(48, 220.0), (60, 440.0), (72, 880.0)] {
            let mut unit = Granulizer::new(context(48000), settings, 60, false);
            unit.note_on(note, samples.len());
            let mut previous = 0.0;
            let mut crossings = 0;
            for _ in 0..12000 {
                let value = unit.process(&samples, [0.0; 8])[0];
                if previous <= 0.0 && value > 0.0 {
                    crossings += 1;
                }
                previous = value;
            }
            assert!((crossings as f32 * 4.0 - expected).abs() <= 4.0);
            assert_eq!(unit.position(), Some(0));
        }
    }
    #[test]
    fn parameter_edits_finish_old_grains_and_sanitize_external_modulation() {
        let samples = vec![[0.3; 2]; 48000];
        let mut unit = Granulizer::new(context(48000), Default::default(), 60, false);
        unit.note_on(60, samples.len());
        for _ in 0..1000 {
            unit.process(&samples, [0.0; 8]);
        }
        let age = unit.grains[0].age;
        unit.retarget(&Granulizer::new(
            context(48000),
            GranulizerSettings {
                size_ms: 10.0,
                overlap: 8.0,
                window: GrainWindow::Triangle,
                ..Default::default()
            },
            60,
            false,
        ));
        assert_eq!(unit.grains[0].age, age);
        assert_eq!(unit.grains[0].window, GrainWindow::Hann);
        for bad in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            -f32::MAX,
        ] {
            for _ in 0..1000 {
                let a = unit.process(&samples, [bad; 8]);
                assert!(a.iter().all(|v| v.is_finite()));
                assert!(
                    unit.effective()
                        .into_iter()
                        .zip(GranulizerSettings::bounds())
                        .all(|(v, (lo, hi))| (lo..=hi).contains(&v))
                );
                assert!(unit.grains.iter().filter(|g| g.age < g.frames).count() <= MAX_GRAINS);
            }
        }
    }
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    #[test]
    fn bounded_advance_matches_source_edges_freeze_fractional_spacing_and_one_shot_tails() {
        let samples: Vec<_> = (0..12000).map(|i| [(i as f32 * 0.01).sin(); 2]).collect();
        for looping in [false, true] {
            for one_shot in [false, true] {
                for speed in [-4.0, 0.0, 0.7, 4.0] {
                    for at in [0, 1234, 13000, 42000] {
                        let settings = GranulizerSettings {
                            speed,
                            looping,
                            position: 0.2,
                            size_ms: 73.0,
                            overlap: 3.5,
                            position_jitter: 0.1,
                            pitch_jitter: 3.0,
                            ..Default::default()
                        };
                        let context = ProcessingContext {
                            sample_rate_hz: 48000,
                            max_block_frames: 512,
                        };
                        let mut reference = Granulizer::new(context, settings, 60, one_shot);
                        reference.note_on(67, samples.len());
                        for _ in 0..at {
                            reference.process(&samples, [0.0; 8]);
                        }
                        let mut seek = Granulizer::new(context, settings, 60, one_shot);
                        seek.note_on(67, samples.len());
                        seek.advance(at, samples.len());
                        for _ in 0..100 {
                            let a = reference.process(&samples, [0.0; 8]);
                            let b = seek.process(&samples, [0.0; 8]);
                            assert!(
                                (a[0] - b[0]).abs() < 0.0001,
                                "loop {looping} one {one_shot} speed {speed} at {at}: {a:?} {b:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
