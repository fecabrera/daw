//! Prepared stereo sample playback. The host owns loading, stretching, notes, and buffers.
use daw_unit_core::{PortKind, ProcessingContext, SILENT_GAIN_DB, SamplerSettings, UnitDescriptor};
use std::sync::Arc;
mod modulation;

/// Immutable buffers indexed by MIDI note. Missing entries play silence.
/// Keep the complete bank alive outside the callback; note changes only an index.
pub type SampleBank = [Option<Arc<Vec<[f32; 2]>>>; 128];

#[derive(Clone)]
pub struct Sampler {
    bank: Arc<SampleBank>,
    granular: Option<Box<daw_granulizer::Granulizer>>,
    note: u8,
    root_note: u8,
    position: u64,
    elapsed_frames: u64,
    fraction: f64,
    requested_note: u8,
    preview_pitch: bool,
    pitch_ratio: f64,
    pitch_target: f64,
    pitch_remaining: u32,
    previous_sample: [f32; 2],
    transition_from: [f32; 2],
    transition_remaining: u32,
    active: bool,
    gain: f32,
    target: f32,
    remaining: u32,
    fade_frames: u32,
    external: daw_unit_core::ModulationSmoother<1>,
    effective_gain: f32,
}
impl Sampler {
    pub fn new(
        context: ProcessingContext,
        settings: &SamplerSettings,
        samples: Arc<Vec<[f32; 2]>>,
    ) -> Self {
        let mut bank: SampleBank = std::array::from_fn(|_| None);
        bank[usize::from(settings.root_note)] = Some(samples);
        Self::with_bank(context, settings, Arc::new(bank))
    }
    pub fn with_bank(
        context: ProcessingContext,
        settings: &SamplerSettings,
        bank: Arc<SampleBank>,
    ) -> Self {
        let target = gain(settings);
        Self {
            bank,
            granular: settings.granulizer.map(|g| {
                Box::new(daw_granulizer::Granulizer::new(
                    context,
                    g,
                    settings.root_note,
                    settings.trigger == daw_unit_core::SamplerTrigger::OneShot,
                ))
            }),
            note: settings.root_note,
            root_note: settings.root_note,
            position: 0,
            elapsed_frames: 0,
            fraction: 0.0,
            requested_note: settings.root_note,
            preview_pitch: false,
            pitch_ratio: 1.0,
            pitch_target: 1.0,
            pitch_remaining: 0,
            previous_sample: [0.0; 2],
            transition_from: [0.0; 2],
            transition_remaining: 0,
            active: false,
            external: daw_unit_core::ModulationSmoother::new(context),
            effective_gain: target,
            gain: target,
            target,
            remaining: 0,
            fade_frames: (context.sample_rate_hz / 200).max(1),
        }
    }
    pub fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: if self.granular.is_some() {
                "daw.granulizer"
            } else {
                "daw.sampler"
            },
            name: if self.granular.is_some() {
                "Granulizer"
            } else {
                "Sampler"
            },
            is_source: true,
            inputs: &[PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        if let (Some(current), Some(next)) = (&mut self.granular, &next.granular) {
            current.retarget(next);
        }
        if self.target != next.target {
            self.target = next.target;
            self.remaining = self.fade_frames;
        }
    }
    /// Swap prepared owners only during renderer inheritance; the retired source keeps the old bank.
    pub fn inherit_bank(&mut self, next: &mut Self) {
        if !Arc::ptr_eq(&self.bank, &next.bank) {
            let previous_buffer = self.samples().as_ptr();
            std::mem::swap(&mut self.bank, &mut next.bank);
            if self.granular.is_none() {
                let note = if self.bank[usize::from(self.requested_note)].is_some() {
                    self.requested_note
                } else {
                    self.root_note
                };
                self.select_pitch_buffer(note);
                self.active &= self.position < self.samples().len() as u64;
                if previous_buffer != self.samples().as_ptr() {
                    self.transition_from = self.previous_sample;
                    self.transition_remaining = self.fade_frames;
                }
                self.pitch_target = 2_f64
                    .powf(f64::from(i16::from(self.requested_note) - i16::from(self.note)) / 12.0);
                self.pitch_remaining = self.fade_frames;
            }
        }
    }
    /// Bend the current source without resetting position, grains, gain or modulation.
    pub fn set_live_pitch(&mut self, note: u8, preview: bool) {
        let note = note.min(127);
        if self.requested_note == note && self.preview_pitch == preview {
            return;
        }
        self.requested_note = note;
        self.preview_pitch = preview;
        if let Some(g) = &mut self.granular {
            g.set_live_pitch(note);
            return;
        }
        if !preview && self.bank[usize::from(note)].is_some() {
            self.select_pitch_buffer(note);
        }
        self.pitch_target = 2_f64.powf(f64::from(i16::from(note) - i16::from(self.note)) / 12.0);
        self.pitch_remaining = self.fade_frames;
    }
    fn select_pitch_buffer(&mut self, note: u8) {
        if self.note == note {
            return;
        }
        self.pitch_ratio *= 2_f64.powf(f64::from(i16::from(self.note) - i16::from(note)) / 12.0);
        self.note = note;
        self.active &= self.position < self.samples().len() as u64;
        self.transition_from = self.previous_sample;
        self.transition_remaining = self.fade_frames;
    }
    pub fn note_on(&mut self) {
        self.note_on_at(self.root_note);
    }
    pub fn note_on_at(&mut self, note: u8) {
        if !self.active() {
            self.gain = self.target;
            self.remaining = 0;
            self.external.reset();
            self.effective_gain = self.target;
        }
        let length = self.bank[usize::from(self.root_note)]
            .as_ref()
            .map_or(0, |s| s.len());
        if let Some(g) = &mut self.granular {
            g.note_on(note, length);
        }
        self.note = note;
        self.requested_note = note;
        self.preview_pitch = false;
        self.pitch_ratio = 1.0;
        self.pitch_target = 1.0;
        self.pitch_remaining = 0;
        self.fraction = 0.0;
        self.previous_sample = [0.0; 2];
        self.transition_remaining = 0;
        self.position = 0;
        self.elapsed_frames = 0;
        self.active = !self.samples().is_empty();
    }
    fn samples(&self) -> &[[f32; 2]] {
        self.bank
            .get(usize::from(if self.granular.is_some() {
                self.root_note
            } else {
                self.note
            }))
            .and_then(Option::as_deref)
            .map_or(&[], Vec::as_slice)
    }
    pub fn reset(&mut self) {
        if let Some(g) = &mut self.granular {
            g.reset();
        }
        self.position = 0;
        self.elapsed_frames = 0;
        self.fraction = 0.0;
        self.pitch_ratio = 1.0;
        self.pitch_target = 1.0;
        self.pitch_remaining = 0;
        self.preview_pitch = false;
        self.requested_note = self.note;
        self.previous_sample = [0.0; 2];
        self.transition_remaining = 0;
        self.active = false;
        self.gain = self.target;
        self.remaining = 0;
        self.external.reset();
        self.effective_gain = self.target;
    }
    pub fn advance(&mut self, frames: u64) {
        if let Some(g) = &mut self.granular {
            let samples = self.bank[usize::from(self.root_note)]
                .as_deref()
                .map_or(&[][..], Vec::as_slice);
            g.advance(frames, samples.len());
            return;
        }
        if self.active {
            self.elapsed_frames = self.elapsed_frames.saturating_add(frames);
            self.position = self
                .position
                .saturating_add(frames)
                .min(self.samples().len() as u64);
        }
        self.active &= self.position < self.samples().len() as u64;
    }
    pub fn active(&self) -> bool {
        self.granular.as_ref().map_or(self.active, |g| g.active())
    }
    pub fn position(&self) -> Option<u64> {
        self.granular
            .as_ref()
            .map_or_else(|| self.active.then_some(self.position), |g| g.position())
    }
    pub fn granular_snapshot(&self) -> Option<([f32; 8], [f32; 16])> {
        self.granular
            .as_ref()
            .filter(|g| g.active())
            .map(|g| (g.effective(), g.grain_positions()))
    }
    pub fn heap_bytes(&self) -> u64 {
        if self.granular.is_some() {
            std::mem::size_of::<daw_granulizer::Granulizer>() as u64
        } else {
            0
        }
    }
    pub fn next_sample(&mut self) -> [f32; 2] {
        self.process_modulated(0.0)
    }
    fn render_sample(&mut self) -> [f32; 2] {
        if !self.active {
            return [0.0; 2];
        }
        let mut sample = self
            .samples()
            .get(self.position as usize)
            .copied()
            .unwrap_or_default();
        if self.fraction != 0.0 {
            let next = self
                .samples()
                .get(self.position as usize + 1)
                .copied()
                .unwrap_or(sample);
            for (value, next) in sample.iter_mut().zip(next) {
                if value.is_finite() && next.is_finite() {
                    *value = (f64::from(*value)
                        + (f64::from(next) - f64::from(*value)) * self.fraction)
                        as f32;
                }
            }
        }
        // Five-millisecond boundaries keep region edits and one-shot ends free of hard cuts.
        let edge = (self.elapsed_frames as f32 / self.fade_frames as f32)
            .min(
                (self.samples().len() as u64 - self.position - 1) as f32
                    / self.pitch_ratio as f32
                    / self.fade_frames as f32,
            )
            .clamp(0.0, 1.0);
        self.elapsed_frames = self.elapsed_frames.saturating_add(1);
        if self.pitch_remaining > 0 {
            self.pitch_ratio +=
                (self.pitch_target - self.pitch_ratio) / f64::from(self.pitch_remaining);
            self.pitch_remaining -= 1;
        }
        let position = self.position as f64 + self.fraction + self.pitch_ratio;
        self.position = position.floor() as u64;
        self.fraction = position.fract();
        self.active = self.position < self.samples().len() as u64;
        let mut output = sample.map(|v| {
            if v.is_finite() {
                (f64::from(v) * f64::from(self.effective_gain) * f64::from(edge))
                    .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
            } else {
                0.0
            }
        });
        if self.transition_remaining > 0 {
            let mix = 1.0 - self.transition_remaining as f32 / self.fade_frames as f32;
            output = std::array::from_fn(|ch| {
                self.transition_from[ch] + (output[ch] - self.transition_from[ch]) * mix
            });
            self.transition_remaining -= 1;
        }
        self.previous_sample = output;
        output
    }
}
impl daw_unit_core::StereoSourceUnit for Sampler {
    fn descriptor(&self) -> UnitDescriptor {
        Sampler::descriptor(self)
    }
    fn reset(&mut self) {
        Sampler::reset(self);
    }
    fn next_sample(&mut self) -> [f32; 2] {
        Sampler::next_sample(self)
    }
}

fn gain(s: &SamplerSettings) -> f32 {
    if !s.enabled || s.gain_db <= SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(s.gain_db / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_pitch_uses_output_time_for_boundaries_and_crossfades_changed_buffers() {
        let ctx = ProcessingContext {
            sample_rate_hz: 48_000,
            max_block_frames: 512,
        };
        let settings = SamplerSettings::default();
        let mut unit = Sampler::new(ctx, &settings, Arc::new(vec![[1.0; 2]; 4800]));
        unit.note_on();
        unit.set_live_pitch(settings.root_note + 12, true);
        for frame in 0..240 {
            assert_eq!(unit.next_sample(), [frame as f32 / 240.0; 2]);
        }
        assert_eq!(unit.next_sample(), [1.0; 2]);
        // A newly prepared bank can replace the same pitch index, for example after
        // changing the offline pitch engine. Preserve the cursor and fade the PCM change.
        let mut next = Sampler::new(ctx, &settings, Arc::new(vec![[-1.0; 2]; 4800]));
        let position = unit.position;
        unit.inherit_bank(&mut next);
        assert_eq!(unit.position, position);
        assert_eq!(unit.next_sample(), [1.0; 2]);
        for _ in 0..240 {
            assert!(unit.next_sample().iter().all(|v| v.is_finite()));
        }
        assert_eq!(unit.next_sample(), [-1.0; 2]);
        unit.position = 4800 - 240;
        unit.elapsed_frames = 1000;
        unit.pitch_ratio = 2.0;
        unit.pitch_remaining = 0;
        assert!(unit.next_sample()[0].abs() < 0.51);
    }
    #[test]
    fn live_pitch_preserves_position_and_swaps_prepared_owners_without_restarting() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let ctx = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = SamplerSettings::default();
            let root = settings.root_note;
            let high = root + 12;
            let samples = Arc::new(vec![[0.4, -0.2]; rate as usize]);
            let mut unit = Sampler::new(ctx, &settings, samples.clone());
            unit.note_on();
            unit.advance(1000);
            let old_bank = unit.bank.clone();
            unit.set_live_pitch(high, true);
            assert_eq!(unit.position, 1000);
            assert_eq!(unit.note, root);
            for _ in 0..rate / 200 {
                unit.next_sample();
            }
            assert!((unit.pitch_ratio - 2.0).abs() < 1e-12);
            assert!(unit.position > 1000 + u64::from(rate / 200));
            let position = unit.position;
            let mut bank: SampleBank = std::array::from_fn(|_| None);
            bank[usize::from(root)] = Some(samples.clone());
            bank[usize::from(high)] = Some(samples);
            let mut prepared = Sampler::with_bank(ctx, &settings, Arc::new(bank));
            unit.set_live_pitch(high, false);
            unit.inherit_bank(&mut prepared);
            assert!(Arc::ptr_eq(&old_bank, &prepared.bank));
            assert_eq!(unit.position, position);
            assert_eq!(unit.note, high);
            for _ in 0..rate / 200 {
                assert_eq!(unit.next_sample(), [0.4, -0.2]);
            }
            assert!((unit.pitch_ratio - 1.0).abs() < 1e-12);
            unit.set_live_pitch(root, true);
            let position = unit.position;
            assert_eq!(unit.position, position);
            for _ in 0..rate / 200 {
                unit.next_sample();
            }
            assert!((unit.pitch_ratio - 0.5).abs() < 1e-12);
            unit.set_live_pitch(high, false);
            assert_eq!(unit.note, high);
        }
    }
    #[test]
    fn stereo_playback_fades_seeks_and_power_preserve_source_at_every_rate() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let ctx = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let samples = Arc::new(vec![[0.4, -0.2]; rate as usize]);
            let mut s = SamplerSettings::default();
            let mut unit = Sampler::new(ctx, &s, samples.clone());
            assert_eq!(unit.descriptor().outputs, &[PortKind::StereoAudio]);
            assert_eq!(unit.next_sample(), [0.0; 2]);
            unit.note_on();
            assert_eq!(unit.next_sample(), [0.0; 2]);
            unit.advance(u64::from(rate / 100));
            assert_eq!(unit.next_sample(), [0.4, -0.2]);
            s.enabled = false;
            unit.retarget(&Sampler::new(ctx, &s, samples.clone()));
            let mut previous = 0.4;
            for _ in 0..rate / 200 {
                let output = unit.next_sample();
                assert!(output[0] <= previous);
                assert_eq!(output[0], output[1] * -2.0);
                previous = output[0];
            }
            assert_eq!(unit.next_sample(), [0.0; 2]);
            unit.reset();
            unit.note_on();
            unit.advance(u64::from(rate / 100));
            assert_eq!(unit.next_sample(), [0.0; 2]);
            s.enabled = true;
            s.gain_db = SILENT_GAIN_DB;
            unit.retarget(&Sampler::new(ctx, &s, samples.clone()));
            unit.reset();
            unit.note_on();
            assert_eq!(unit.next_sample(), [0.0; 2]);
            unit.advance(u64::from(rate));
            assert!(!unit.active());
            assert_eq!(unit.position(), None);
            assert_eq!(samples[100], [0.4, -0.2]);
        }
    }
}
