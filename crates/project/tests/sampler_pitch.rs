use daw_core::{Edit, Id, MidiNote, MidiPattern, SamplerMode, SamplerSettings, StretchEngine};
use daw_engine::Renderer;
use daw_project::Session;
use std::{fs, path::PathBuf, sync::Arc};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-sampler-pitch-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        let mut wav = hound::WavWriter::create(
            path.join("tone.wav"),
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for frame in 0..24000 {
            let value = (frame as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.25;
            wav.write_sample(value).unwrap();
            wav.write_sample(-value).unwrap();
        }
        wav.finalize().unwrap();
        Self(path)
    }
    fn session(&self, engine: StretchEngine, notes: &[u8]) -> Session {
        let mut session = Session::default();
        session.project.stretch_engine = engine;
        let track = session.project.add_midi_track().unwrap();
        let layer = &mut session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.sampler = Some(SamplerSettings::default());
        let owner = layer.id;
        let clip = session.project.add_midi_pattern(track, 0).unwrap();
        session
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: pattern(notes),
            })
            .unwrap();
        session.project.tracks[0].clips[0].length_frames = session.project.midi_frame(11520);
        session
            .with_sampler_file(track, owner, &self.0.join("tone.wav"))
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pattern(notes: &[u8]) -> MidiPattern {
    MidiPattern {
        length_ticks: 11520,
        notes: notes
            .iter()
            .enumerate()
            .map(|(i, &pitch)| MidiNote {
                id: Id::new_v4(),
                start_tick: i as u32 * 1920,
                length_ticks: 240,
                pitch,
                velocity: 127,
            })
            .collect(),
    }
}
fn settings(session: &Session) -> &SamplerSettings {
    session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .sampler
        .as_ref()
        .unwrap()
}
fn frequency(samples: &[[f32; 2]], rate: u32) -> f64 {
    let crossings: Vec<_> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0][0] <= 0.0 && pair[1][0] > 0.0)
        .map(|(i, pair)| i as f64 - f64::from(pair[0][0]) / f64::from(pair[1][0] - pair[0][0]))
        .collect();
    assert!(crossings.len() > 10);
    f64::from(rate) * (crossings.len() - 1) as f64 / (crossings.last().unwrap() - crossings[0])
}
fn assert_pitch(samples: &[[f32; 2]], rate: u32, offset: i8) {
    let middle = &samples[samples.len() / 4..samples.len() * 3 / 4];
    let expected = 440.0 * (f64::from(offset) / 12.0).exp2();
    let cents = (frequency(middle, rate) / expected).log2().abs() * 1200.0;
    assert!(cents < 15.0, "{offset} semitones at {rate}: {cents} cents");
    assert!(middle.iter().all(|s| (s[0] + s[1]).abs() < 0.0001));
}

#[test]
fn sampler_pitch_uses_selected_engine_and_root_with_live_export_seek_and_storage_parity() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.0.join("tone.wav")).unwrap();
    for engine in [StretchEngine::INTEGRATED, StretchEngine::RUBBER_BAND] {
        let mut session = fixture.session(engine, &[48, 60, 72, 35, 85]);
        let id = settings(&session).id;
        assert_eq!(session.sampler_audio[&id].variants().count(), 5);
        assert_eq!(
            session.audio[&id].samples,
            session.audio[&settings(&session).input.as_ref().unwrap().asset_id].samples
        );
        assert!(!session.sampler_preparation_needed(&session.project));
        for (offset, audio) in session.sampler_audio[&id].variants() {
            assert_eq!(audio.samples.len(), 24000);
            assert_pitch(&audio.samples, 48000, offset);
        }
        let mut offline = vec![[0.0; 2]; 240000];
        Renderer::new(session.plan()).render_offline(&mut offline);
        for (i, offset) in [-12, 0, 12, -25, 25].into_iter().enumerate() {
            assert_pitch(&offline[i * 48000..i * 48000 + 24000], 48000, offset);
        }
        let mut live = Renderer::new(session.plan());
        live.play();
        for (i, &expected) in offline.iter().enumerate() {
            let actual = live.next_sample();
            if i > 240 {
                assert_eq!(actual, expected);
            }
        }
        let mut range = vec![[0.0; 2]; 512];
        session.plan().render(100000, &mut range);
        assert_eq!(range, offline[100000..100512]);

        let export = fixture.0.join(format!("{}.wav", engine.label()));
        session
            .export_with_settings(
                &export,
                false,
                daw_media::ExportSettings {
                    format: daw_media::ExportFormat::Wav,
                    wav_codec: daw_media::WavCodec::Float32,
                    ..Default::default()
                },
            )
            .unwrap();
        let decoded = daw_media::decode_wav(&export).unwrap();
        assert_eq!(&decoded.samples[..offline.len()], &offline);

        let old = session.audio[&id].samples.clone();
        let mut instrument = session.project.tracks[0].instrument.clone().unwrap();
        instrument.layers[0].sampler.as_mut().unwrap().gain_db = -3.0;
        instrument.layers[0].sampler.as_mut().unwrap().collapsed = true;
        let edit = Edit::SetInstrument {
            track_id: session.project.tracks[0].id,
            instrument: Some(instrument),
        };
        assert!(
            !session.sampler_preparation_needed(&session.project.edited(edit.clone()).unwrap())
        );
        session = session.with_edit(edit).unwrap();
        assert!(Arc::ptr_eq(&old, &session.audio[&id].samples));

        let folder = fixture.0.join(engine.label());
        session.save(&folder).unwrap();
        let reopened = Session::open(&folder).unwrap();
        let mut saved = vec![[0.0; 2]; 24000];
        let mut expected = saved.clone();
        Renderer::new(session.plan()).render_offline(&mut expected);
        Renderer::new(reopened.plan()).render_offline(&mut saved);
        assert_eq!(saved, expected);
        assert_eq!(reopened.project.stretch_engine, engine);
    }
    assert_eq!(fs::read(fixture.0.join("tone.wav")).unwrap(), original);
}

#[test]
fn sampler_pitch_rebuilds_for_notes_root_duration_engine_rate_and_rolls_back_cancellation() {
    let fixture = Fixture::new();
    let session = fixture.session(StretchEngine::INTEGRATED, &[60]);
    let id = settings(&session).id;
    let clip = session.project.tracks[0].clips[0].id;
    let mut changed = session.clone();
    changed
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: pattern(&[36, 84, 35, 85]),
        })
        .unwrap();
    assert!(session.sampler_preparation_needed(&changed.project));
    let calls = std::cell::Cell::new(0);
    assert!(
        changed
            .prepare_stretches_cancellable(&|| {
                calls.set(calls.get() + 1);
                calls.get() > 12
            })
            .is_err()
    );
    assert_eq!(changed.sampler_audio[&id].variants().count(), 1);
    assert!(Arc::ptr_eq(
        &changed.audio[&id].samples,
        &session.audio[&id].samples
    ));
    changed.prepare_stretches().unwrap();
    assert_eq!(changed.sampler_audio[&id].variants().count(), 5);
    for (offset, audio) in changed.sampler_audio[&id].variants() {
        assert_pitch(&audio.samples, 48000, offset);
    }

    let mut rooted = changed.clone();
    rooted.project.tracks[0].instrument.as_mut().unwrap().layers[0]
        .sampler
        .as_mut()
        .unwrap()
        .root_note = 61;
    assert!(changed.sampler_preparation_needed(&rooted.project));
    rooted.prepare_stretches().unwrap();
    assert_eq!(
        rooted.sampler_audio[&id]
            .variants()
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>(),
        [-26, -25, 0, 23, 24]
    );
    for engine in [StretchEngine::INTEGRATED, StretchEngine::RUBBER_BAND] {
        let base = fixture.session(engine, &[72]);
        for mode in [SamplerMode::Off, SamplerMode::Time, SamplerMode::Bars] {
            let mut timed = base.clone();
            let s = timed.project.tracks[0].instrument.as_mut().unwrap().layers[0]
                .sampler
                .as_mut()
                .unwrap();
            s.mode = mode;
            s.time_ms = 750.0;
            s.bars = 1;
            timed.prepare_stretches().unwrap();
            let frames = settings(&timed).output_frames(48000, 120.0).unwrap();
            for (offset, audio) in timed.sampler_audio[&settings(&timed).id].variants() {
                assert_eq!(audio.samples.len() as u64, frames);
                assert_pitch(&audio.samples, 48000, offset);
            }
        }
        for rate in daw_core::SAMPLE_RATES {
            let rated = base.with_audio_settings(rate, 256).unwrap();
            for (offset, audio) in rated.sampler_audio[&settings(&rated).id].variants() {
                assert_eq!(audio.samples.len(), rate as usize / 2);
                assert_pitch(&audio.samples, rate, offset);
            }
        }
    }
    let switched = changed
        .with_processing_settings(48000, 512, StretchEngine::RUBBER_BAND)
        .unwrap();
    assert_eq!(switched.project.stretch_engine, StretchEngine::RUBBER_BAND);
    assert!(!switched.sampler_preparation_needed(&switched.project));
    for (offset, audio) in switched.sampler_audio[&id].variants() {
        assert_pitch(&audio.samples, 48000, offset);
        if offset != 0 {
            let old = changed.sampler_audio[&id]
                .variants()
                .find(|(o, _)| *o == offset)
                .unwrap()
                .1;
            assert!(!Arc::ptr_eq(&audio.samples, &old.samples));
        }
    }
    assert!(
        session
            .with_processing_settings(
                48000,
                512,
                StretchEngine {
                    revision: 99,
                    ..StretchEngine::INTEGRATED
                }
            )
            .is_err()
    );
}

#[test]
fn sampler_full_midi_endpoints_prepare_with_each_engine_without_clamping() {
    let fixture = Fixture::new();
    for engine in [StretchEngine::INTEGRATED, StretchEngine::RUBBER_BAND] {
        for (root, note, expected_offset) in
            [(0, 127, 127), (127, 0, -127), (60, 0, -60), (60, 127, 67)]
        {
            let mut session = fixture.session(engine, &[]);
            let id = settings(&session).id;
            let clip = session.project.tracks[0].clips[0].id;
            session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0]
                .sampler
                .as_mut()
                .unwrap()
                .root_note = root;
            session
                .project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: clip,
                    pattern: pattern(&[note]),
                })
                .unwrap();
            session.prepare_stretches().unwrap();
            assert_eq!(settings(&session).pitch_offset(note), Some(expected_offset));
            let variants = &session.sampler_audio[&id];
            let audio = variants
                .variants()
                .find(|(offset, _)| *offset == expected_offset)
                .unwrap()
                .1;
            assert_eq!(audio.samples.len(), 24000);
            assert!(
                audio
                    .samples
                    .iter()
                    .all(|s| s.iter().all(|v| v.is_finite()))
            );
            assert!(!Arc::ptr_eq(&audio.samples, &variants.root_audio().samples));
            assert!(!session.sampler_preparation_needed(&session.project));
            let mut rendered = vec![[0.0; 2]; 24000];
            Renderer::new(session.plan()).render_offline(&mut rendered);
            assert!(rendered.iter().all(|s| s.iter().all(|v| v.is_finite())));
        }
    }
}

#[test]
fn parallel_project_preparation_matches_serial_banks_and_cancellation_keeps_the_source() {
    for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
        let fixture = Fixture::new();
        let serial = fixture.session(engine, &[60, 64, 67]);
        let mut parallel = serial.clone();
        parallel.sampler_audio.clear();
        parallel
            .prepare_project_audio(&|| false, &|_, _| {})
            .unwrap();
        for (id, bank) in &serial.sampler_audio {
            let expected: Vec<_> = bank.variants().collect();
            let actual: Vec<_> = parallel.sampler_audio[id].variants().collect();
            assert_eq!(expected.len(), actual.len());
            for ((pitch, audio), (other, prepared)) in expected.into_iter().zip(actual) {
                assert_eq!(pitch, other);
                assert_eq!(audio.samples, prepared.samples);
            }
        }
        let before = parallel.project.clone();
        assert!(
            parallel
                .prepare_project_audio(&|| true, &|_, _| {})
                .is_err()
        );
        assert_eq!(parallel.project, before);
    }
}
