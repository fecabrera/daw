//! Synthetic cache workloads, shared by regression tests and the release benchmark.
use daw_core::*;
use daw_engine::RenderPlan;

pub fn midi_project(rate: u32, tracks: usize, saturators: usize) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Default::default()
    };
    for index in 0..tracks {
        let id = project.add_midi_track().unwrap();
        let track = project.tracks.last_mut().unwrap();
        track.gain_db = -18.0;
        track.pan = if index % 2 == 0 { -0.2 } else { 0.3 };
        track.instrument = Some(
            OscillatorSettings {
                waveform: Waveform::Saw,
                ..Default::default()
            }
            .into(),
        );
        let instrument = track.instrument.as_mut().unwrap();
        let lfo = LfoInstance::default();
        let source = ModulationSourceId {
            layer: instrument.main.id,
            unit: lfo.id,
        };
        instrument.main.lfos.push(lfo);
        let mut route = Modulation::new(ModulationParameter::EnvelopeAttack);
        route.source = Some(source);
        route.amount = 1.5;
        instrument.layers[0].envelopes[0].modulations.push(route);
        for _ in 0..saturators {
            track.effects.push(EffectInstance::saturator());
        }
        let clip = project.add_midi_pattern(id, 0).unwrap();
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 7680,
                    notes: (0..4)
                        .map(|n| MidiNote {
                            id: Id::new_v4(),
                            start_tick: n * 120,
                            length_ticks: 5760,
                            pitch: 40 + (index % 12) as u8 + n as u8,
                            velocity: 90 + n as u8,
                        })
                        .collect(),
                },
            })
            .unwrap();
    }
    RenderPlan {
        project,
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}

#[allow(dead_code)]
pub fn add_audio(plan: &mut RenderPlan, count: usize, fx: usize, mono: bool) -> Id {
    let rate = plan.project.sample_rate_hz;
    let frames = u64::from(rate) * 2;
    let asset = Id::new_v4();
    let metadata = SourceMetadata {
        sample_rate_hz: rate,
        channels: if mono { 1 } else { 2 },
        sample_format: "float".into(),
        bits_per_sample: 32,
    };
    let samples = (0..frames)
        .map(|i| {
            let signal = (i as f32 * 0.031).sin() * 0.15;
            [signal, if mono { signal } else { -signal * 0.4 }]
        })
        .collect();
    plan.project.assets.push(Asset {
        id: asset,
        name: "synthetic".into(),
        midi_pattern: None,
        source: Source {
            kind: "external".into(),
            path: "synthetic.wav".into(),
            path_kind: "relative".into(),
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: frames,
    });
    plan.audio.insert(
        asset,
        daw_media::AudioData {
            samples: std::sync::Arc::new(samples),
            peaks: Default::default(),
            metadata,
            sample_rate_hz: rate,
        },
    );
    for index in 0..count {
        plan.project.add_track().unwrap();
        let track = plan.project.tracks.last_mut().unwrap();
        track.gain_db = -12.0;
        track.pan = 0.15;
        track.clips.push(Clip {
            id: Id::new_v4(),
            asset_id: asset,
            name: "audio".into(),
            color: None,
            start_frame: index as u64 * 123,
            source_offset_frame: 311,
            length_frames: frames - 1024,
            repeat: Some(ClipLoop {
                length_frames: 10000,
                phase_frame: 155,
            }),
            stretch: None,
        });
        for _ in 0..fx {
            track.effects.push(EffectInstance::saturator());
        }
    }
    asset
}
