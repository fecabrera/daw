//! One track stage for live rendering, offline rendering and cache workers.
use super::{effects::EffectChain, latency, midi::MidiTrackRuntime, schedule::AudioTrack};
use daw_core::Clip;

pub(super) struct TrackProcessor<'a> {
    pub audio: &'a mut AudioTrack,
    pub midi: &'a mut Option<MidiTrackRuntime>,
    pub effects: &'a mut EffectChain,
    pub delay: &'a mut latency::Delay,
    pub clips: &'a [Clip],
}
impl TrackProcessor<'_> {
    pub fn sample(
        &mut self,
        frame: u64,
        tempo: f32,
        region: Option<(u64, u64)>,
        advancing: bool,
    ) -> ([f32; 2], bool) {
        let (source, mono) = if let Some(runtime) = self.midi.as_mut() {
            (
                if advancing {
                    runtime.sample_in_region(frame, region)
                } else {
                    runtime.held_sample()
                },
                Some(true),
            )
        } else {
            self.audio.sample(self.clips, frame)
        };
        let source = self.effects.sample(
            source,
            mono,
            latency::delayed_frame(
                frame,
                self.midi.as_ref().map_or(0, |r| r.effect_latency()),
                region,
            ),
            tempo,
            region,
            (
                self.midi.as_ref().map_or(&[], |r| r.modulation_values()),
                self.midi.as_ref().map(|r| r.modulation_sources()),
            ),
        );
        self.delay.tick(source)
    }
}
