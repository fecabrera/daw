//! Prepared stereo sample playback. The host owns loading, stretching, notes, and buffers.
use daw_unit_core::{PortKind, ProcessingContext, SILENT_GAIN_DB, SamplerSettings, UnitDescriptor};
use std::sync::Arc;
mod modulation;

pub struct Sampler {
    samples: Arc<Vec<[f32; 2]>>,
    position: u64,
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
        let target = gain(settings);
        Self {
            samples,
            position: 0,
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
            type_id: "daw.sampler",
            name: "Sampler",
            is_source: true,
            inputs: &[PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        if self.target != next.target {
            self.target = next.target;
            self.remaining = self.fade_frames;
        }
    }
    pub fn note_on(&mut self) {
        if !self.active {
            self.gain = self.target;
            self.remaining = 0;
            self.external.reset();
            self.effective_gain = self.target;
        }
        self.position = 0;
        self.active = !self.samples.is_empty();
    }
    pub fn reset(&mut self) {
        self.position = 0;
        self.active = false;
        self.gain = self.target;
        self.remaining = 0;
        self.external.reset();
        self.effective_gain = self.target;
    }
    pub fn advance(&mut self, frames: u64) {
        if self.active {
            self.position = self
                .position
                .saturating_add(frames)
                .min(self.samples.len() as u64);
        }
        self.active &= self.position < self.samples.len() as u64;
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn position(&self) -> Option<u64> {
        self.active.then_some(self.position)
    }
    pub fn next_sample(&mut self) -> [f32; 2] {
        self.process_modulated(0.0)
    }
    fn render_sample(&mut self) -> [f32; 2] {
        if !self.active {
            return [0.0; 2];
        }
        let sample = self
            .samples
            .get(self.position as usize)
            .copied()
            .unwrap_or_default();
        // Five-millisecond boundaries keep region edits and one-shot ends free of hard cuts.
        let edge = (self.position as f32 / self.fade_frames as f32)
            .min((self.samples.len() as u64 - self.position - 1) as f32 / self.fade_frames as f32)
            .clamp(0.0, 1.0);
        self.position += 1;
        self.active = self.position < self.samples.len() as u64;
        sample.map(|v| {
            if v.is_finite() {
                (f64::from(v) * f64::from(self.effective_gain) * f64::from(edge))
                    .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
            } else {
                0.0
            }
        })
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
