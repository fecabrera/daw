use daw_core::{
    Edit, Id, InstrumentComponent, InstrumentLayerSettings, MidiNote, MidiPattern, Modulation,
    ModulationParameter as P, SamplerSettings,
};
use daw_engine::Renderer;
use daw_project::Session;
use std::{fs, sync::Arc};

#[test]
fn granular_sample_preparation_storage_copy_rate_export_and_failures_share_session_transactions() {
    let folder = std::env::temp_dir().join(format!("daw-granulizer-{}", Id::new_v4()));
    fs::create_dir(&folder).unwrap();
    let path = folder.join("input.wav");
    let mut wav = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for frame in 0..12000 {
        let v = (frame as f32 * 0.03).sin() * 0.2;
        wav.write_sample(v).unwrap();
        wav.write_sample(-v).unwrap();
    }
    wav.finalize().unwrap();
    let original = fs::read(&path).unwrap();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let layer = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    let owner = layer.id;
    layer.sampler = Some(SamplerSettings::granular());
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 1920,
                notes: [0, 60, 127]
                    .into_iter()
                    .map(|pitch| MidiNote {
                        id: Id::new_v4(),
                        pitch,
                        velocity: 64,
                        start_tick: 0,
                        length_ticks: 360,
                    })
                    .collect(),
            },
        })
        .unwrap();
    session = session.with_sampler_file(track, owner, &path).unwrap();
    let id = session.project.samplers().next().unwrap().id;
    assert_eq!(session.sampler_audio[&id].variants().count(), 1);
    assert!(!session.sampler_preparation_needed(&session.project));
    let root = session.audio[&id].samples.clone();
    let mut instrument = session.project.tracks[0].instrument.clone().unwrap();
    let s = instrument.layers[0].sampler.as_mut().unwrap();
    s.granulizer.as_mut().unwrap().position_jitter = 0.2;
    s.granulizer.as_mut().unwrap().pan = 0.5;
    s.modulations.push(Modulation::new(P::GrainPitch));
    session = session
        .with_edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    assert!(Arc::ptr_eq(&root, &session.audio[&id].samples));
    let mut expected = vec![[0.0; 2]; 24000];
    Renderer::new(session.plan()).render_offline(&mut expected);
    assert!(expected.iter().any(|s| s[0].abs() > 0.001));
    let export = folder.join("export.wav");
    session
        .export_with_settings(
            &export,
            false,
            daw_media::ExportSettings {
                wav_codec: daw_media::WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let audio = daw_media::decode_wav(&export).unwrap();
    assert_eq!(&audio.samples[..expected.len()], &expected);
    let project_folder = folder.join("project");
    session.save(&project_folder).unwrap();
    let reopened = Session::open(&project_folder).unwrap();
    let mut saved = expected.clone();
    Renderer::new(reopened.plan()).render_offline(&mut saved);
    assert_eq!(saved, expected);
    let mut range = vec![[0.0; 2]; 512];
    session.plan().render(12000, &mut range);
    assert_eq!(range, expected[12000..12512]);
    for rate in daw_core::SAMPLE_RATES {
        let rated = session.with_audio_settings(rate, 256).unwrap();
        assert_eq!(rated.audio[&id].samples.len(), rate as usize / 4);
        assert_eq!(rated.sampler_audio[&id].variants().count(), 1);
        assert!(
            rated
                .project
                .samplers()
                .next()
                .unwrap()
                .granulizer
                .is_some()
        );
    }
    let before = serde_json::to_value(&session.project).unwrap();
    assert!(
        session
            .with_sampler_file(track, owner, &folder.join("missing.wav"))
            .is_err()
    );
    assert!(
        session
            .with_sampler_clip_cancellable(
                track,
                owner,
                &daw_core::Clip::from(
                    session
                        .project
                        .samplers()
                        .next()
                        .unwrap()
                        .input
                        .as_ref()
                        .unwrap()
                ),
                &|| true
            )
            .is_err()
    );
    assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    let mut copied = session.project.clone();
    let instrument = copied.tracks[0].instrument.as_mut().unwrap();
    instrument.layers.push(InstrumentLayerSettings::default());
    let destination = instrument.layers[1].id;
    copied
        .edit(Edit::TransferInstrumentComponent {
            track_id: track,
            source: owner,
            destination,
            component: InstrumentComponent::Sampler,
            destination_index: None,
            duplicate: true,
        })
        .unwrap();
    let instrument = copied.tracks[0].instrument.as_ref().unwrap();
    let source = instrument.layers[0].sampler.as_ref().unwrap();
    let copy = instrument.layers[1].sampler.as_ref().unwrap();
    assert_ne!(source.id, copy.id);
    assert_ne!(source.modulations[0].id, copy.modulations[0].id);
    assert_eq!(source.granulizer, copy.granulizer);
    assert!(
        copied
            .edit(Edit::TransferInstrumentComponent {
                track_id: track,
                source: owner,
                destination: instrument.main.id,
                component: InstrumentComponent::Sampler,
                destination_index: None,
                duplicate: false
            })
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn switching_to_granular_drops_unused_offline_pitches_and_reuses_the_root_buffer() {
    let source = daw_media::AudioData {
        sample_rate_hz: 48000,
        samples: Arc::new(vec![[0.1, -0.1]; 4800]),
        peaks: Default::default(),
        metadata: daw_core::SourceMetadata {
            sample_rate_hz: 48000,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        },
    };
    let mut settings = SamplerSettings {
        input: Some(daw_core::SamplerInput {
            asset_id: Id::new_v4(),
            name: "sample".into(),
            source_offset_frame: 0,
            length_frames: 4800,
            repeat: None,
            stretch: None,
        }),
        end_frame: 4800,
        ..Default::default()
    };
    let engine = daw_core::StretchEngine::RUBBER_BAND;
    let old = daw_media::PreparedSampler::new_cancellable(
        &source,
        &settings,
        4800,
        engine,
        &[60, 72],
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(old.variants().count(), 2);
    settings.granulizer = Some(Default::default());
    settings.version = 2;
    assert!(!old.matches(&source, &settings, 4800, engine, &[60, 72]));
    let granular = daw_media::PreparedSampler::new_cancellable(
        &source,
        &settings,
        4800,
        engine,
        &[60, 72],
        Some(&old),
        &|| false,
    )
    .unwrap();
    assert_eq!(granular.variants().count(), 1);
    assert!(Arc::ptr_eq(
        &old.root_audio().samples,
        &granular.root_audio().samples
    ));
    assert!(granular.matches(&source, &settings, 4800, engine, &[0, 60, 72, 127]));
}
