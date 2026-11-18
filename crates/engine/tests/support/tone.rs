use daw_core::*;
use daw_engine::RenderPlan;
use daw_media::AudioData;
use std::{collections::HashMap, sync::Arc};

pub fn plan(rate: u32, mono: bool) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Project::default()
    };
    project.add_track().unwrap();
    let id = Id::new_v4();
    let metadata = SourceMetadata {
        sample_rate_hz: rate,
        channels: if mono { 1 } else { 2 },
        sample_format: "ieee_float".into(),
        bits_per_sample: 32,
    };
    project.assets.push(Asset {
        id,
        name: "tone".into(),
        midi_pattern: None,
        rendered_audio: None,
        compound_audio: None,
        beat_pattern: None,
        source: Source {
            kind: "external".into(),
            path: "/tone.wav".into(),
            storage: None,
            path_kind: "absolute".into(),
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: 8192,
    });
    project.tracks[0].clips.push(Clip {
        channels: Default::default(),
        fades: Default::default(),
        muted: false,
        id: Id::new_v4(),
        asset_id: id,
        name: "tone".into(),
        color: None,
        start_frame: 0,
        source_offset_frame: 0,
        length_frames: 8192,
        repeat: None,
        stretch: None,
        gain_db: 0.0,
        transpose: 0,
    });
    let samples = (0..8192)
        .map(|frame| {
            let sample = (frame as f32 * 1000.0 * std::f32::consts::TAU / rate as f32).sin() * 0.1;
            [sample, if mono { sample } else { -sample * 0.5 }]
        })
        .collect();
    RenderPlan {
        external: Default::default(),
        beat_tracks: Default::default(),
        project,
        audio: HashMap::from([(
            id,
            AudioData {
                disk: None,
                sample_rate_hz: rate,
                metadata,
                samples: Arc::new(samples),
                peaks: Arc::new(vec![]),
            },
        )]),
        sampler_audio: Default::default(),
        stretched_audio: HashMap::new(),
    }
}
