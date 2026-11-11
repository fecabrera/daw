//! One track stage for live rendering, offline rendering and cache workers.
use super::{effects::EffectChain, latency, midi::MidiTrackRuntime, schedule::AudioTrack};
use daw_core::Clip;

pub(super) struct TrackProcessor<'a> {
    pub sidechains: &'a [daw_unit_core::ModulationValue],
    pub sidechain_sources: Option<&'a daw_unit_core::ModulationSourceIndex>,
    pub audio: &'a mut AudioTrack,
    pub midi: &'a mut Option<MidiTrackRuntime>,
    pub effects: &'a mut EffectChain,
    pub delay: &'a mut latency::Delay,
    pub clips: &'a [Clip],
}
impl TrackProcessor<'_> {
    pub fn sample_unaligned(
        &mut self,
        frame: u64,
        tempo: f32,
        region: Option<(u64, u64)>,
        advancing: bool,
        realtime: bool,
    ) -> ([f32; 2], bool) {
        let (source, mono) = if let Some(runtime) = self.midi.as_mut() {
            (
                if advancing {
                    runtime.sample_in_region(frame, region)
                } else {
                    runtime.held_sample()
                },
                Some(runtime.output_is_mono()),
            )
        } else {
            self.audio.sample(self.clips, frame, realtime)
        };
        self.effects.sample(
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
                self.midi
                    .as_ref()
                    .map_or(self.sidechains, |r| r.modulation_values()),
                self.midi
                    .as_ref()
                    .map(|r| r.modulation_sources())
                    .or(self.sidechain_sources),
            ),
        )
    }
}

// MIDI channel semantics are prepared, so both delayed metadata banks carry
// the same coefficients. Inheritance then ramps mode changes exactly like Pan,
// including sends and sidechains, without switching banks at a latency boundary.
pub(super) fn track_pan_coefficients(track: &daw_core::Track) -> [[f32; 2]; 2] {
    if matches!(
        track.kind,
        daw_core::TrackKind::Group | daw_core::TrackKind::Return
    ) {
        // Lowered Beat Output groups retain an instrument payload for controls,
        // but their input is the already mixed stereo bus.
        [super::pan_coefficients(track.pan, false); 2]
    } else if let Some(instrument) = &track.instrument {
        let coefficients =
            super::pan_coefficients(track.pan, instrument_is_mono(instrument, &track.effects));
        [coefficients; 2]
    } else {
        [
            super::pan_coefficients(track.pan, true),
            super::pan_coefficients(track.pan, false),
        ]
    }
}

// Resolve channel semantics during preparation, never from instantaneous L/R
// equality (a stereo signal can cross the centre without becoming a mono bus).
pub(super) fn instrument_is_mono(
    instrument: &daw_core::InstrumentSettings,
    track_effects: &[daw_core::EffectInstance],
) -> bool {
    fn preserves_mono(effects: &[daw_core::EffectInstance]) -> bool {
        use daw_core::EffectSettings as E;
        effects.iter().all(|effect| match &effect.unit {
            E::DirectionMixer(_)
            | E::AutoFilter(_)
            | E::StereoSpread(_)
            | E::Chorus(_)
            | E::PlateReverb(_)
            | E::AmbientReverb(_)
            | E::ShimmerReverb(_) => false,
            E::Eq3(_)
            | E::Loudness(_)
            | E::MultibandEq(_)
            | E::Exciter(_)
            | E::MultibandCompressor(_)
            | E::HarmonicLift(_)
            | E::Drive(_)
            | E::DriveV2(_)
            | E::Filter(_)
            | E::Compressor(_)
            | E::Ott(_)
            | E::Limiter(_)
            | E::Saturator(_)
            | E::Bitcrusher(_)
            | E::Missing { .. } => true,
        })
    }
    instrument.main.pan == 0.0
        && preserves_mono(&instrument.main.effects)
        && preserves_mono(track_effects)
        && instrument.layers.iter().all(|layer| {
            layer.pan == 0.0
                && layer.sampler.as_ref().is_none_or(|s| s.input.is_none())
                && layer.noise.is_none()
                && preserves_mono(&layer.effects)
                && layer
                    .oscillators
                    .iter()
                    .all(|osc| osc.unison == daw_core::OscillatorUnison::Off)
        })
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    #[test]
    fn prepared_semantics_keep_legacy_mono_and_preserve_stereo_sources_and_buses() {
        let original = daw_core::InstrumentSettings::from(daw_core::OscillatorSettings::default());
        assert!(instrument_is_mono(&original, &[]));
        for case in 0..6 {
            let mut next = original.clone();
            match case {
                0 => {
                    next.layers[0].oscillators.clear();
                    next.layers[0].sampler = Some(daw_core::SamplerSettings {
                        input: Some(daw_core::SamplerInput {
                            asset_id: daw_core::Id::new_v4(),
                            name: "sample".into(),
                            source_offset_frame: 0,
                            length_frames: 1,
                            repeat: None,
                            stretch: None,
                        }),
                        end_frame: 1,
                        ..Default::default()
                    });
                }
                1 => {
                    next.layers[0].oscillators.clear();
                    next.layers[0].noise = Some(Default::default());
                }
                2 => next.layers[0].oscillators[0].unison = daw_core::OscillatorUnison::Eight,
                3 => next.layers[0].pan = -0.5,
                4 => next.main.pan = 0.5,
                _ => next.layers[0]
                    .effects
                    .push(daw_core::EffectInstance::chorus()),
            }
            assert!(!instrument_is_mono(&next, &[]));
        }
        assert!(!instrument_is_mono(
            &original,
            &[daw_core::EffectInstance::stereo_spread()]
        ));
    }
}
