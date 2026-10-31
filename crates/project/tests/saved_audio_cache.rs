//! Separate test process keeps application cache configuration isolated from other suites.
use daw_core::{Edit, Id, MidiNote, MidiPattern, SamplerMode};
use daw_project::Session;
use std::{fs, path::PathBuf};
fn pattern(pitch: u8) -> MidiPattern {
    MidiPattern {
        length_ticks: 960,
        notes: [pitch, 67]
            .into_iter()
            .map(|pitch| MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 960,
                pitch,
                velocity: 100,
            })
            .collect(),
    }
}
#[test]
fn saved_audio_survives_process_restart_and_repeated_pitches_reuse_exact_pcm() {
    if let Some(root) = std::env::var_os("DAW_CACHE_REUSE_CHILD") {
        let root = PathBuf::from(root);
        daw_media::configure_preparation_cache(Some(root.join("cache")));
        let _session = Session::open(&root).unwrap();
        let stats = daw_media::preparation_cache_stats();
        assert_eq!(stats.builds, 0);
        assert_eq!(stats.disk_hits, 3);
        return;
    }
    let root = std::env::temp_dir().join(format!("daw-saved-cache-{}", Id::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.wav");
    let mut writer = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for frame in 0..4096 {
        let value = (frame as f32 * 0.02).sin() * 0.2;
        writer.write_sample(value).unwrap();
        writer.write_sample(-value).unwrap();
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
    let layer = layer.id;
    session = session.with_sampler_file(track, layer, &source).unwrap();
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
    session = session
        .with_edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(64),
        })
        .unwrap();
    let sampler = session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .sampler
        .as_ref()
        .unwrap()
        .id;
    let expected: Vec<_> = session.sampler_audio[&sampler]
        .variants()
        .map(|(pitch, audio)| (pitch, audio.samples.clone()))
        .collect();
    session.save(&root).unwrap();
    drop(session);
    daw_media::configure_preparation_cache(Some(root.join("cache")));
    let mut session = Session::open(&root).unwrap();
    assert_eq!(daw_media::preparation_cache_stats().builds, 3);
    let root_audio = session.sampler_audio[&sampler].root_audio();
    let identity = daw_core::ClipStretch::new(1, 1).unwrap();
    assert!(
        daw_media::PreparedStretch::new_cancellable(
            root_audio,
            identity,
            session.project.stretch_engine,
            &|| true
        )
        .is_err()
    );
    let mut invalid = session.project.stretch_engine;
    invalid.revision = u32::MAX;
    assert!(daw_media::PreparedStretch::new(root_audio, identity, invalid).is_err());

    for ((pitch, samples), (actual, audio)) in expected
        .iter()
        .zip(session.sampler_audio[&sampler].variants())
    {
        assert_eq!(*pitch, actual);
        assert_eq!(samples, &audio.samples);
    }
    session = session
        .with_edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(65),
        })
        .unwrap();
    let before = daw_media::preparation_cache_stats();
    session = session
        .with_edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(64),
        })
        .unwrap();
    assert_eq!(daw_media::preparation_cache_stats().builds, before.builds);
    drop(session);
    daw_media::release_preparation_cache_memory();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "saved_audio_survives_process_restart_and_repeated_pitches_reuse_exact_pcm",
            "--nocapture",
        ])
        .env("DAW_CACHE_REUSE_CHILD", &root)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
