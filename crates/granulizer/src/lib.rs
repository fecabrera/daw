//! Settings adapter for bounded granular synthesis over host-owned PCM.
use daw_dsp_support::granular as dsp;
use daw_unit_core::{GranulizerSettings, ProcessingContext};

pub use dsp::MAX_GRAINS;

#[derive(Clone)]
pub struct Granulizer(dsp::Granulizer);

impl Granulizer {
    pub fn new(
        context: ProcessingContext,
        settings: GranulizerSettings,
        root: u8,
        one_shot: bool,
    ) -> Self {
        Self(dsp::Granulizer::new(
            context.sample_rate_hz,
            dsp::Parameters {
                size_ms: settings.size_ms,
                overlap: settings.overlap,
                position: settings.position,
                speed: settings.speed,
                pitch: settings.pitch,
                position_jitter: settings.position_jitter,
                pitch_jitter: settings.pitch_jitter,
                pan: settings.pan,
                window: settings.window.into(),
                looping: settings.looping,
                seed: settings.seed,
                bounds: GranulizerSettings::bounds(),
            },
            root,
            one_shot,
        ))
    }

    pub fn retarget(&mut self, next: &Self) {
        self.0.retarget(&next.0)
    }

    pub fn reset(&mut self) {
        self.0.reset()
    }

    pub fn set_live_pitch(&mut self, note: u8) {
        self.0.set_live_pitch(note)
    }

    pub fn note_on(&mut self, note: u8, length: usize) {
        self.0.note_on(note, length)
    }

    pub fn advance(&mut self, frames: u64, length: usize) {
        self.0.advance(frames, length)
    }

    pub fn active(&self) -> bool {
        self.0.active()
    }

    pub fn position(&self) -> Option<u64> {
        self.0.position()
    }

    pub fn effective(&self) -> [f32; 8] {
        self.0.effective()
    }

    pub fn grain_positions(&self) -> [f32; 16] {
        self.0.grain_positions()
    }

    pub fn process(&mut self, samples: &[[f32; 2]], offsets: [f32; 8]) -> [f32; 2] {
        self.0.process(samples, offsets)
    }
}
