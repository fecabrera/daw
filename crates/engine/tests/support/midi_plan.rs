use daw_core::*;
use daw_engine::RenderPlan;
use std::collections::HashMap;

pub fn plan(waveform: Waveform, rate: u32) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Project::default()
    };
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument = Some(OscillatorSettings::default().into());
    project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].waveform = waveform;
    // Existing oscillator tests isolate source/mixer behavior using the legacy gate.
    project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
        EnvelopeSettings::legacy();
    let clip = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![
                    MidiNote {
                        id: Id::from_u128(1),
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 69,
                        velocity: 127,
                    },
                    MidiNote {
                        id: Id::from_u128(2),
                        start_tick: 480,
                        length_ticks: 1440,
                        pitch: 72,
                        velocity: 100,
                    },
                ],
            },
        })
        .unwrap();
    project.tracks[0].clips[0].length_frames = project.midi_frame(3840);
    RenderPlan {
        external: Default::default(),
        beat_tracks: Default::default(),
        project,
        audio: HashMap::new(),
        sampler_audio: Default::default(),
        stretched_audio: HashMap::new(),
    }
}
