//! Synthetic release probe. Writes only an explicitly supplied scratch fixture folder.
use daw_core::{Edit, Id, MidiNote, MidiPattern, SamplerMode};
use daw_project::Session;
use std::{path::Path, time::Instant};
fn pattern(pitches: &[u8], velocity: u8) -> MidiPattern {
    MidiPattern {
        length_ticks: 3840,
        notes: pitches
            .iter()
            .map(|&pitch| MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 3840,
                pitch,
                velocity,
            })
            .collect(),
    }
}
fn initialize(root: &Path, seconds: u32) {
    std::fs::create_dir_all(root).unwrap();
    let path = root.join("source.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for index in 0..44100u64 * u64::from(seconds) {
        let value = ((index as f64 * 440.0 * std::f64::consts::TAU / 44100.0).sin()
            * 0.2
            * 8388607.0) as i32;
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let layer = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.oscillators.clear();
    layer.sampler = Some(Default::default());
    let owner = layer.id;
    session = session.with_sampler_file(track, owner, &path).unwrap();
    session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .sampler
        .as_mut()
        .unwrap()
        .mode = SamplerMode::Off;
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(&[60, 64, 67], 100),
        })
        .unwrap();
    session.save(root).unwrap();
}
fn timed<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let before = daw_media::preparation_cache_stats();
    let clock = Instant::now();
    let result = work();
    let elapsed = clock.elapsed().as_secs_f64() * 1000.0;
    let after = daw_media::preparation_cache_stats();
    println!(
        "{}",
        serde_json::json!({"case":name,"milliseconds":elapsed,"memory_hits":after.memory_hits-before.memory_hits,"disk_hits":after.disk_hits-before.disk_hits,"builds":after.builds-before.builds,"shared_audio_bytes_at_return":daw_media::audio_memory_bytes()})
    );
    result
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        args.len() >= 3,
        "audio_preparation init|uncached|cached <scratch-project> [seconds|cache-directory]"
    );
    let root = Path::new(&args[2]);
    if args[1] == "init" {
        initialize(root, args.get(3).map_or(58, |n| n.parse().unwrap()));
        return;
    }
    if args[1] == "cached" {
        daw_media::configure_preparation_cache(Some(args.get(3).expect("cache directory").into()));
    } else {
        assert_eq!(args[1], "uncached");
    }
    let document = timed("editor_model", || Session::open_document(root).unwrap());
    let mut session = document.into_editor_session();
    timed("required_audio", || {
        session
            .prepare_project_audio(&|| false, &|_, _| {})
            .unwrap()
    });
    timed("renderer", || daw_engine::Renderer::new(session.plan()));
    let clip = session.project.tracks[0].clips[0].id;
    session = timed("velocity_edit", || {
        session
            .with_edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(&[60, 64, 67], 90),
            })
            .unwrap()
    });
    session = timed("new_pitch", || {
        session
            .with_edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(&[61, 64, 67], 90),
            })
            .unwrap()
    });
    session = session
        .with_edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(&[60, 64, 67], 90),
        })
        .unwrap();
    let _session = timed("repeated_pitch", || {
        session
            .with_edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(&[61, 64, 67], 90),
            })
            .unwrap()
    });
}
