use daw_core::{Edit, Id, MidiNote, MidiPattern, NoiseSettings, NoiseType};
use daw_engine::Renderer;
use daw_project::Session;
#[test]
fn noise_save_reopen_and_float_export_keep_the_same_signal_without_media_assets() {
    let folder = std::env::temp_dir().join(format!("daw-noise-{}", Id::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let mut instrument = session.project.tracks[0].instrument.clone().unwrap();
    instrument.layers[0].noise = Some(NoiseSettings {
        kind: NoiseType::Pink,
        color: 0.25,
        width: 0.75,
        ..Default::default()
    });
    session = session
        .with_edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session = session
        .with_edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    pitch: 60,
                    velocity: 100,
                    start_tick: 0,
                    length_ticks: 480,
                }],
            },
        })
        .unwrap();
    let mut expected = vec![[0.0; 2]; 12000];
    Renderer::new(session.plan()).render_offline(&mut expected);
    assert!(expected.iter().any(|s| s[0].abs() > 0.001));
    let project = folder.join("project");
    session.save(&project).unwrap();
    let reopened = Session::open(&project).unwrap();
    assert_eq!(
        reopened.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    assert!(reopened.audio.is_empty());
    let mut actual = expected.clone();
    Renderer::new(reopened.plan()).render_offline(&mut actual);
    assert_eq!(actual, expected);
    let output = folder.join("noise.wav");
    session
        .export_with_settings(
            &output,
            false,
            daw_media::ExportSettings {
                wav_codec: daw_media::WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let rendered = daw_media::decode_wav(&output).unwrap();
    assert_eq!(&rendered.samples[..expected.len()], &expected);
    std::fs::remove_dir_all(folder).unwrap();
}
